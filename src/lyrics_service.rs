use crate::{lyrics::*, native, services::Track, storage};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
const BASE: &str = "https://lyrics.api.dacubeking.com";
#[derive(Default, Serialize, Deserialize)]
pub struct JwtCache {
    token: String,
    exp: u64,
}
impl JwtCache {
    pub fn token(&self) -> Option<String> {
        self.usable(crate::services::now_ms() / 1000)
            .then(|| self.token.clone())
    }
    pub fn usable(&self, now: u64) -> bool {
        !self.token.is_empty() && self.exp > now
    }
    pub fn should_renew(&self, now: u64) -> bool {
        self.exp <= now.saturating_add(3600)
    }
    pub fn accept(&mut self, token: String) -> Result<(), String> {
        let token = normalize_token(&token);
        let exp = jwt_exp(&token).ok_or("Некоректний JWT")?;
        if exp <= crate::services::now_ms() / 1000 {
            return Err("JWT прострочено".into());
        }
        self.token = token;
        self.exp = exp;
        Ok(())
    }
    pub fn load() -> Self {
        let mut cached = std::fs::read(storage::data_dir().join("lyrics-auth.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Self>(&b).ok())
            .unwrap_or_default();
        // Derive expiry from the actual token rather than trusting a stale side field.
        cached.token = normalize_token(&cached.token);
        cached.exp = jwt_exp(&cached.token).unwrap_or(0);
        cached
    }
    fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(storage::data_dir()).map_err(|e| e.to_string())?;
        std::fs::write(
            storage::data_dir().join("lyrics-auth.json"),
            serde_json::to_vec(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    pub fn clear(&mut self) {
        self.token.clear();
        self.exp = 0;
        let _ = std::fs::remove_file(storage::data_dir().join("lyrics-auth.json"));
    }
    pub async fn exchange(&mut self, token: String) -> Result<(), String> {
        let token = normalize_token(&token);
        if token.is_empty() {
            return Err("Порожній Turnstile token".into());
        }
        // The original app also accepts an already-issued JWT without exchanging it.
        let jwt = if jwt_exp(&token).is_some() {
            token
        } else {
            native::verify_turnstile(token, Some(BASE.into())).await?
        };
        self.accept(jwt)?;
        self.save()
    }
}

pub(crate) fn normalize_token(raw: &str) -> String {
    let mut raw = raw.trim().to_owned();
    for _ in 0..8 {
        if raw
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("bearer "))
        {
            raw = raw[7..].trim().to_owned();
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            break;
        };
        let next = if let Some(text) = value.as_str() {
            Some(text.to_owned())
        } else {
            [
                "jwt",
                "token",
                "access_token",
                "accessToken",
                "data",
                "result",
            ]
            .iter()
            .filter_map(|key| value.get(key))
            .find(|v| v.is_string() || v.is_object())
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            })
        };
        let Some(next) = next else {
            break;
        };
        raw = next.trim().to_owned();
    }
    raw
}

fn jwt_exp(token: &str) -> Option<u64> {
    let segments = token.split('.').collect::<Vec<_>>();
    if segments.len() != 3 || segments.iter().any(|s| s.is_empty()) {
        return None;
    }
    let payload = segments[1];
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()?
        .get("exp")?
        .as_u64()
}

// Concurrent lyric requests share one renewal. A valid near-expiry JWT remains usable.
#[derive(Default)]
struct RefreshResult {
    finished: Option<std::time::Instant>,
    result: Option<Result<String, String>>,
}
impl RefreshResult {
    fn recent(&self, requested: std::time::Instant) -> Option<Result<String, String>> {
        let finished = self.finished?;
        // All waiters share both success and failure. A failed challenge gets a
        // short cooldown instead of opening one helper per queued song request.
        (finished >= requested
            || self.result.as_ref().is_some_and(Result::is_err)
                && finished.elapsed() < std::time::Duration::from_secs(30))
        .then(|| self.result.clone())
        .flatten()
    }
}
static AUTH_REFRESH: tokio::sync::Mutex<RefreshResult> =
    tokio::sync::Mutex::const_new(RefreshResult {
        finished: None,
        result: None,
    });
async fn renew_jwt(rejected: Option<String>) -> Result<String, String> {
    // Track cancellation must not release the shared renewal lock while a
    // browser challenge is still running. Other tracks reuse that one renewal.
    tokio::spawn(renew_jwt_shared(rejected))
        .await
        .map_err(|_| "Не вдалося перевірити доступ до слів".to_owned())?
}
async fn renew_jwt_shared(rejected: Option<String>) -> Result<String, String> {
    let requested = std::time::Instant::now();
    let mut refresh = AUTH_REFRESH.lock().await;
    let mut cached = JwtCache::load();
    if let Some(token) = cached.token() {
        if rejected.as_ref().is_some_and(|old| old != &token)
            || (rejected.is_none() && !cached.should_renew(crate::services::now_ms() / 1000))
        {
            return Ok(token);
        }
    }
    if let Some(result) = refresh.recent(requested) {
        return result;
    }
    eprintln!("[lyrics-auth] refreshing automatically");
    let result = async {
        let challenge = crate::challenge::acquire_silent().await?;
        cached.exchange(challenge).await?;
        cached.token().ok_or("JWT недоступний".into())
    }
    .await;
    eprintln!(
        "[lyrics-auth] {}",
        if result.is_ok() {
            "JWT cached"
        } else {
            "automatic refresh failed"
        }
    );
    refresh.finished = Some(std::time::Instant::now());
    refresh.result = Some(result.clone());
    result
}
async fn ensure_jwt() -> Result<String, String> {
    let cached = JwtCache::load();
    if let Some(token) = cached.token() {
        if cached.should_renew(crate::services::now_ms() / 1000) {
            // An unsuccessful renewal must not discard the still-valid credential.
            schedule_renewal();
        }
        return Ok(token);
    }
    renew_jwt(None).await
}

/// Startup warmup, matching the WebStorm app's prewarmAuthenticationToken.
/// Returns metadata only to callers; credentials stay in the auth cache.
pub async fn prewarm_authentication() -> Result<JwtCache, String> {
    ensure_jwt().await?;
    Ok(JwtCache::load())
}

fn invalidate_jwt(rejected: &str) {
    let mut cached = JwtCache::load();
    if cached.token == rejected {
        cached.clear();
    }
}

fn schedule_renewal() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        tokio::spawn(async {
            let _ = renew_jwt(None).await;
            RUNNING.store(false, Ordering::SeqCst);
        });
    }
}

fn valid_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}

/// Exact URLSearchParams fields used by the WebStorm app's buildV2LyricsBody.
pub fn v2_lyrics_form(track: &Track, video_id: &str, token: &str) -> HashMap<String, String> {
    let album = regex::Regex::new(r"(?i)\s*-\s*(Single|EP|Album|Deluxe|Remix)\b.*$")
        .unwrap()
        .replace(&track.album, "")
        .trim()
        .to_string();
    let duration = if track.duration_secs.is_finite() {
        track.duration_secs.max(0.).round() as u64
    } else {
        0
    };
    HashMap::from([
        ("videoId".into(), video_id.into()),
        ("song".into(), track.title.trim().into()),
        ("artist".into(), track.artist.trim().into()),
        ("album".into(), album),
        ("duration".into(), duration.to_string()),
        ("alwaysFetchMetadata".into(), "false".into()),
        ("token".into(), token.into()),
    ])
}

/// v2 can answer SSE, direct lyrics, or a JSON-wrapped lyric payload.
pub fn parse_lyrics_response(raw: &str) -> Vec<LyricsAlternative> {
    parse_lyrics_response_depth(raw, 0)
}
fn parse_lyrics_response_depth(raw: &str, depth: usize) -> Vec<LyricsAlternative> {
    if depth > 8 {
        return vec![];
    }
    let raw = raw.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if raw.starts_with("```") || raw.starts_with("~~~") {
        if let Some(body) = crate::lyrics::code_fence_body(raw) {
            return parse_lyrics_response_depth(body, depth + 1);
        }
        return crate::lyrics::recover_ttml(raw)
            .map(|document| parse_lyrics_response_depth(&document, depth + 1))
            .unwrap_or_default();
    }
    if let Some(document) = crate::lyrics::recover_ttml(raw) {
        return parse_lyrics_response_depth(&document, depth + 1);
    }
    if raw.starts_with('"') {
        return serde_json::from_str::<String>(raw)
            .map(|body| parse_lyrics_response_depth(&body, depth + 1))
            .unwrap_or_default();
    }
    if matches!(raw, "" | "null" | "{}" | "[DONE]") {
        return vec![];
    }
    let is_sse = raw
        .lines()
        .any(|l| l.starts_with("data:") || l.starts_with("event:"));
    let mut alternatives = parse_provider_sse(raw);
    if is_sse {
        return alternatives;
    }
    if alternatives.is_empty() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
            if value.get("results").is_some() {
                alternatives = parse_provider_sse(&format!("data: {raw}\n\n"));
            }
            // API gateways may wrap the same provider records in data/result arrays.
            // Keep every provider alternative instead of treating the envelope as text.
            if alternatives.is_empty() {
                if let Some(records) = value.as_array() {
                    if records
                        .iter()
                        .any(|v| v.get("results").is_some() || v.get("provider").is_some())
                    {
                        for record in records {
                            alternatives.extend(parse_lyrics_response_depth(
                                &record.to_string(),
                                depth + 1,
                            ));
                        }
                    }
                } else {
                    for field in ["data", "result", "response", "payload"] {
                        if let Some(inner) =
                            value.get(field).filter(|v| v.is_object() || v.is_array())
                        {
                            alternatives
                                .extend(parse_lyrics_response_depth(&inner.to_string(), depth + 1));
                            if !alternatives.is_empty() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    if alternatives.is_empty() {
        if let Some(parsed) = alternative("dacubeking", raw) {
            alternatives.push(parsed);
        }
    }
    alternatives
}
fn cache_path(track: &Track) -> PathBuf {
    use sha1::{Digest, Sha1};
    storage::data_dir()
        .join("lyrics")
        .join(format!("{:x}.json", Sha1::digest(track.path.as_bytes())))
}
#[derive(Serialize, Deserialize)]
struct Cached {
    #[serde(default)]
    version: u32,
    saved: u64,
    alternatives: Vec<LyricsAlternative>,
}
pub fn save_cache(track: &Track, alternatives: &[LyricsAlternative]) -> Result<(), String> {
    let path = cache_path(track);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        path,
        serde_json::to_vec(&Cached {
            version: 4,
            saved: crate::services::now_ms(),
            alternatives: alternatives.to_vec(),
        })
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub fn load_cache(track: &Track) -> Option<Vec<LyricsAlternative>> {
    let c: Cached = serde_json::from_slice(&std::fs::read(cache_path(track)).ok()?).ok()?;
    (c.version == 4
        && crate::services::now_ms().saturating_sub(c.saved) < 30 * 24 * 3600 * 1000
        && !c.alternatives.is_empty())
    .then_some(c.alternatives)
}
pub fn parse(raw: &str) -> ParsedLyrics {
    parse_payload(raw)
        .map(|(lyrics, _, _)| lyrics)
        .unwrap_or_else(|| parse_plain(""))
}
fn alternative(provider: &str, raw: &str) -> Option<LyricsAlternative> {
    let (lyrics, format, sync_type) = parse_payload(raw)?;
    Some(LyricsAlternative {
        provider: provider.into(),
        sync_type,
        format: format.into(),
        lyrics,
    })
}
pub fn migrated_cache(
    store: &crate::services::Store,
    track: &Track,
) -> Option<Vec<LyricsAlternative>> {
    let id = track.video_id.as_deref()?;
    let entry = store.legacy.get(&format!("pvf:lyrics:video:{id}"))?;
    let saved = entry.get("savedAt").and_then(|v| v.as_u64()).unwrap_or(0);
    if entry.get("version").and_then(|v| v.as_u64()) != Some(3)
        || crate::services::now_ms().saturating_sub(saved) > 30 * 24 * 3600 * 1000
    {
        return None;
    }
    if let Some(sse) = entry.get("sseRaw").and_then(|v| v.as_str()) {
        let alts = parse_provider_sse(sse);
        if !alts.is_empty() {
            return Some(alts);
        }
    }
    let raw = entry.get("result")?.get("raw")?.as_str()?;
    alternative("Перенесений кеш", raw).map(|a| vec![a])
}
pub async fn fetch(
    track: &Track,
    provider: &str,
    refresh: bool,
    jwt: Option<String>,
) -> Result<Vec<LyricsAlternative>, String> {
    fetch_with_progress(track, provider, refresh, jwt, |_| {}).await
}

/// Publish usable results as providers finish; authentication cannot hold back
/// public lyrics, and the final quality order still includes every provider.
pub async fn fetch_with_progress(
    track: &Track,
    provider: &str,
    refresh: bool,
    jwt: Option<String>,
    progress: impl FnMut(Vec<LyricsAlternative>) + Send,
) -> Result<Vec<LyricsAlternative>, String> {
    if !refresh {
        if let Some(c) = load_cache(track) {
            return Ok(c);
        }
    }
    if !track.path.starts_with("yt://") {
        for ext in ["ttml", "lrc", "qrc", "srt", "vtt", "txt"] {
            let path = std::path::Path::new(&track.path).with_extension(ext);
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Some(a) = alternative("Локальний файл", &raw) {
                    return Ok(vec![a]);
                }
            }
        }
    }
    static PUBLIC_CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> =
        std::sync::OnceLock::new();
    let client = PUBLIC_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(18))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let primary = async {
        if provider == "auto" || provider == "dacubeking" {
            fetch_primary(track, jwt).await
        } else {
            (vec![], vec![])
        }
    };
    let unison = async {
        if provider == "auto" || provider == "unison" {
            fetch_unison(track, &client).await
        } else {
            (vec![], vec![])
        }
    };
    let lrclib = async {
        if provider == "auto" || provider == "lrclib" {
            fetch_lrclib(track, &client).await
        } else {
            (vec![], vec![])
        }
    };
    let (mut found, mut errors) =
        collect_provider_results(primary, unison, lrclib, track.duration_secs, progress).await;
    if found.is_empty() && (provider == "auto" || provider == "youtube") {
        let (alternatives, failures) = fetch_captions(track).await;
        found.extend(alternatives);
        errors.extend(failures);
    }
    prepare_alternatives(&mut found, track.duration_secs);
    if found.is_empty() {
        return Err(if errors.is_empty() {
            "Слова не знайдено. Можна вставити власні.".into()
        } else {
            errors.join(" · ")
        });
    }
    // A full/read-only cache must not discard successfully downloaded lyrics.
    let _ = save_cache(track, &found);
    Ok(found)
}

type ProviderResult = (Vec<LyricsAlternative>, Vec<String>);

async fn collect_provider_results(
    primary: impl std::future::Future<Output = ProviderResult>,
    unison: impl std::future::Future<Output = ProviderResult>,
    lrclib: impl std::future::Future<Output = ProviderResult>,
    duration_secs: f64,
    mut progress: impl FnMut(Vec<LyricsAlternative>),
) -> ProviderResult {
    let mut found = Vec::new();
    let mut errors = Vec::new();
    tokio::pin!(primary, unison, lrclib);
    let mut done = [false; 3];
    while done.iter().any(|finished| !finished) {
        let (index, (alternatives, failures)) = tokio::select! {
            value = &mut primary, if !done[0] => (0, value),
            value = &mut unison, if !done[1] => (1, value),
            value = &mut lrclib, if !done[2] => (2, value),
        };
        done[index] = true;
        errors.extend(failures);
        if !alternatives.is_empty() {
            found.extend(alternatives);
            prepare_alternatives(&mut found, duration_secs);
            progress(found.clone());
        }
    }
    (found, errors)
}

fn prepare_alternatives(found: &mut Vec<LyricsAlternative>, duration_secs: f64) {
    for alt in found.iter_mut() {
        alt.sync_type = crate::braccato::effective_sync_type(alt);
        if alt.sync_type != LyricsSyncType::Plain {
            complete_timing(&mut alt.lyrics, (duration_secs.max(0.) * 1000.) as u64);
        }
    }
    crate::braccato::providers::normalize_alternatives(found);
}

async fn fetch_primary(
    track: &Track,
    jwt: Option<String>,
) -> (Vec<LyricsAlternative>, Vec<String>) {
    let mut found = vec![];
    let mut errors = vec![];

    let direct = track
        .video_id
        .as_deref()
        .or_else(|| track.path.strip_prefix("yt://"));
    let id = if let Some(id) = direct.filter(|id| valid_video_id(id)) {
        Some(id.to_string())
    } else {
        native::resolve_youtube_video_id(
            track.title.clone(),
            track.artist.clone(),
            Some(track.duration_secs),
        )
        .await
        .unwrap_or(None)
    };
    if let Some(id) = id {
        let authentication = match jwt
            .filter(|t| jwt_exp(t).is_some_and(|exp| exp > crate::services::now_ms() / 1000))
        {
            Some(jwt) => {
                if jwt_exp(&jwt).is_some_and(|exp| exp <= crate::services::now_ms() / 1000 + 3600) {
                    schedule_renewal();
                }
                Ok(jwt)
            }
            None => ensure_jwt().await,
        };
        match authentication {
            Ok(token) => {
                let params = v2_lyrics_form(track, &id, &token);
                let mut response = native::fetch_lyrics_get(params, Some(BASE.into())).await;
                if response.as_ref().is_err_and(|e| e == "v2/lyrics HTTP 401") {
                    invalidate_jwt(&token);
                    response = match renew_jwt(Some(token)).await {
                        Ok(fresh) => {
                            let retry = native::fetch_lyrics_get(
                                v2_lyrics_form(track, &id, &fresh),
                                Some(BASE.into()),
                            )
                            .await;
                            if retry.as_ref().is_err_and(|e| e == "v2/lyrics HTTP 401") {
                                invalidate_jwt(&fresh);
                            }
                            retry
                        }
                        Err(_) => Err("Потрібна повторна перевірка доступу до слів".into()),
                    };
                }
                match response {
                    Ok(Some(raw)) => found.extend(parse_lyrics_response(&raw)),
                    Err(e) => errors.push(e),
                    _ => {}
                }
            }
            Err(e) => errors.push(format!("Автоматична перевірка слів: {e}")),
        }
    }

    (found, errors)
}

async fn fetch_unison(
    track: &Track,
    client: &reqwest::Client,
) -> (Vec<LyricsAlternative>, Vec<String>) {
    let mut found = vec![];
    let mut errors = vec![];

    let mut variants = vec![vec![
        ("song", track.title.as_str()),
        ("artist", track.artist.as_str()),
    ]];
    if let Some(id) = &track.video_id {
        variants.insert(0, vec![("v", id.as_str())]);
    }
    for query in variants {
        match client
            .get("https://unison.boidu.dev/lyrics")
            .query(&query)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                if let Ok(v) = r.json::<serde_json::Value>().await {
                    if let Some(raw) = v["data"]["lyrics"].as_str() {
                        if let Some(a) = alternative("Unison", raw) {
                            found.push(a);
                            break;
                        }
                    }
                }
            }
            Ok(r) if r.status().as_u16() >= 429 => {
                errors.push(format!("Unison HTTP {}", r.status()))
            }
            Err(_) => errors.push("Unison: помилка з’єднання".into()),
            _ => {}
        }
    }

    (found, errors)
}

async fn fetch_lrclib(
    track: &Track,
    client: &reqwest::Client,
) -> (Vec<LyricsAlternative>, Vec<String>) {
    let mut found = vec![];
    let mut errors = vec![];
    {
        match client
            .get("https://lrclib.net/api/get")
            .header("User-Agent", "PlayerVF/0.1")
            .query(&[
                ("track_name", &track.title),
                ("artist_name", &track.artist),
                ("album_name", &track.album),
                ("duration", &track.duration_secs.round().to_string()),
            ])
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                if let Ok(v) = r.json::<serde_json::Value>().await {
                    for key in ["syncedLyrics", "plainLyrics"] {
                        if let Some(raw) = v[key].as_str() {
                            if let Some(a) = alternative("LRCLib", raw) {
                                found.push(a);
                            }
                        }
                    }
                }
            }
            Err(_) => errors.push("LRCLib: помилка з’єднання".into()),
            _ => {}
        }
    }
    // Search is necessary for streams whose browse row did not include a duration,
    // and for local files whose album spelling differs from the provider record.
    if !found.iter().any(|a| a.provider == "LRCLib") {
        if let Ok(response) = client
            .get("https://lrclib.net/api/search")
            .header("User-Agent", "PlayerVF/0.1")
            .query(&[("track_name", &track.title), ("artist_name", &track.artist)])
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(records) = response.json::<Vec<serde_json::Value>>().await {
                    let norm = |s: &str| {
                        s.chars()
                            .filter(|c| c.is_alphanumeric())
                            .flat_map(char::to_lowercase)
                            .collect::<String>()
                    };
                    if let Some(value) = records.iter().find(|v| {
                        v["trackName"]
                            .as_str()
                            .is_some_and(|s| norm(s) == norm(&track.title))
                            && v["artistName"]
                                .as_str()
                                .is_some_and(|s| norm(s) == norm(&track.artist))
                            && (track.duration_secs <= 0.
                                || v["duration"]
                                    .as_f64()
                                    .is_some_and(|d| (d - track.duration_secs).abs() <= 5.))
                    }) {
                        for key in ["syncedLyrics", "plainLyrics"] {
                            if let Some(raw) = value[key].as_str() {
                                if let Some(a) = alternative("LRCLib", raw) {
                                    found.push(a);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (found, errors)
}

async fn fetch_captions(track: &Track) -> (Vec<LyricsAlternative>, Vec<String>) {
    let mut found = vec![];
    let mut errors = vec![];

    if let Some(id) = &track.video_id {
        match native::fetch_youtube_captions(id.clone(), Some("uk".into())).await {
            Ok(Some(c)) => {
                let raw = c
                    .lines
                    .iter()
                    .map(|l| {
                        format!(
                            "[{:02}:{:02}.{:03}]{}",
                            l.start_ms / 60000,
                            l.start_ms / 1000 % 60,
                            l.start_ms % 1000,
                            l.text
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if let Some(a) = alternative("YouTube", &raw) {
                    found.push(a);
                }
            }
            Err(e) => errors.push(e),
            _ => {}
        }
    }

    (found, errors)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn public_lyrics_arrive_before_auth_and_upgrade_when_ready() {
        let (release, gate) = tokio::sync::oneshot::channel();
        let mut release = Some(release);
        let primary = async {
            gate.await.unwrap();
            (
                vec![alternative("primary", "[00:01.00]Timed line").unwrap()],
                vec![],
            )
        };
        let public = async { (vec![alternative("public", "Plain line").unwrap()], vec![]) };
        let unavailable = async { (vec![], vec!["unavailable".into()]) };
        let mut modes = Vec::new();
        let (found, errors) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            collect_provider_results(primary, public, unavailable, 10., |alts| {
                modes.push(alts[0].sync_type);
                // Authentication can only finish after the public result is shown.
                if let Some(release) = release.take() {
                    release.send(()).unwrap();
                }
            }),
        )
        .await
        .expect("public lyrics must not wait for authentication");
        assert_eq!(modes, [LyricsSyncType::Plain, LyricsSyncType::Synced]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].provider, "primary");
        assert_eq!(found[0].lyrics.lines[0].duration_ms, 9000);
        assert_eq!(errors, ["unavailable"]);
    }

    #[test]
    fn refresh_result_shares_waiters_and_cools_down_failures() {
        let started = std::time::Instant::now();
        let completed = std::time::Instant::now();
        let later = completed + std::time::Duration::from_secs(1);
        let mut refresh = RefreshResult {
            finished: Some(completed),
            result: Some(Ok("jwt".into())),
        };
        assert_eq!(refresh.recent(started), Some(Ok("jwt".into())));
        assert_eq!(refresh.recent(later), None);
        refresh.result = Some(Err("timeout".into()));
        assert_eq!(refresh.recent(later), Some(Err("timeout".into())));
        refresh.finished = Some(completed - std::time::Duration::from_secs(31));
        assert_eq!(refresh.recent(later), None);
    }
    #[test]
    fn jwt_renewal_boundary_and_malformed() {
        let c = JwtCache {
            token: "x".into(),
            exp: 10000,
        };
        assert!(c.usable(6399));
        assert!(c.usable(6400));
        assert!(c.usable(9999));
        assert!(!c.usable(10000));
        assert!(!c.should_renew(6399));
        assert!(c.should_renew(6400));
        let mut c = JwtCache::default();
        assert!(c.accept("bad".into()).is_err());
    }
    fn fixture_jwt(exp: u64) -> String {
        format!(
            "e30.{}.fixture",
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::to_vec(&serde_json::json!({"exp":exp})).unwrap())
        )
    }

    #[test]
    fn jwt_exchange_envelopes_and_quoted_bearer_tokens_are_normalized() {
        let valid = fixture_jwt(crate::services::now_ms() / 1000 + 7200);
        for raw in [
            serde_json::json!({"data":{"access_token":valid}}).to_string(),
            serde_json::json!(format!("Bearer {valid}")).to_string(),
            serde_json::json!({"result":{"jwt":format!("Bearer {valid}")}}).to_string(),
        ] {
            let mut cache = JwtCache::default();
            cache.accept(raw).unwrap();
            assert_eq!(cache.token().as_deref(), Some(valid.as_str()));
        }
        assert!(JwtCache::default()
            .accept("Bearer not.a.jwt".into())
            .is_err());
    }

    #[test]
    fn provider_envelopes_preserve_alternatives_and_source_timing() {
        let response = serde_json::json!({"data":[
            {"provider":"first","results":{"synced":"[00:00.00]First\n[00:02.25]Second"}},
            {"provider":"second","results":{"plain":"Untimed words"}}
        ]});
        let parsed = parse_lyrics_response(&response.to_string());
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].provider, "first");
        assert_eq!(parsed[0].sync_type, LyricsSyncType::Synced);
        assert_eq!(parsed[0].lyrics.lines[1].start_ms, 2250);
        assert_eq!(parsed[1].sync_type, LyricsSyncType::Plain);
        assert!(!crate::lyrics::has_word_level_timing(&parsed[0].lyrics));
        assert!(parse_lyrics_response(r#"{"error":"unauthorized"}"#).is_empty());
    }
    #[test]
    fn jwt_inputs_are_trimmed_decoded_and_expiry_is_validated() {
        let valid = fixture_jwt(crate::services::now_ms() / 1000 + 1200);
        let mut cache = JwtCache::default();
        cache.accept(format!("  Bearer {valid} ")).unwrap();
        assert_eq!(cache.token(), Some(valid.clone()));
        cache
            .accept(serde_json::json!({"jwt":valid}).to_string())
            .unwrap();
        assert!(cache.accept(fixture_jwt(1)).is_err());
        for malformed in [
            "header.payload",
            "a.!.b",
            "a..c",
            "a.e30.c",
            "a.e30.c.extra",
        ] {
            assert!(cache.accept(malformed.into()).is_err());
        }
        // Invalid input cannot destroy an existing valid token.
        assert!(cache.token().is_some());
    }
    #[test]
    fn matches_original_v2_request_fields() {
        let track = Track {
            title: " Song ".into(),
            artist: " Artist ".into(),
            album: "Record - Single".into(),
            duration_secs: 132.4,
            ..Default::default()
        };
        let form = v2_lyrics_form(&track, "abcdefghijk", "fixture");
        assert_eq!(form.len(), 7);
        assert_eq!(form["alwaysFetchMetadata"], "false");
        assert_eq!(form["videoId"], "abcdefghijk");
        assert_eq!(form["song"], "Song");
        assert_eq!(form["artist"], "Artist");
        assert_eq!(form["duration"], "132");
        assert_eq!(form["album"], "Record");
        assert_eq!(form["token"], "fixture");
    }
    #[test]
    fn response_decoding_handles_sse_json_and_direct_formats() {
        assert_eq!(
            parse_lyrics_response("[00:01.00]Hello")[0].lyrics.lines[0].text,
            "Hello"
        );
        assert_eq!(
            parse_lyrics_response(r#"{"lyrics":"[00:01.00]Hello"}"#)[0]
                .lyrics
                .lines[0]
                .text,
            "Hello"
        );
        let event = r#"{"provider":"lrclib","results":{"synced":"[00:01.00]Hello"}}"#;
        assert_eq!(parse_lyrics_response(event)[0].provider, "lrclib");
        assert_eq!(
            parse_lyrics_response(&format!("data: {event}\n\n"))[0].provider,
            "lrclib"
        );
        assert!(parse_lyrics_response("event: provider\ndata: broken\n\n").is_empty());
        assert!(parse_lyrics_response("null").is_empty());
        let fenced = format!("```json\n{event}\n```");
        assert_eq!(parse_lyrics_response(&fenced)[0].provider, "lrclib");
        let encoded = serde_json::to_string(&fenced).unwrap();
        assert_eq!(parse_lyrics_response(&encoded)[0].provider, "lrclib");
        let xml = "```xml\n<tt><body><p begin='1s' end='3s'>Fixture</p></body></tt>\n```";
        let event = serde_json::json!({"provider":"fixture", "results":{"xml":xml, "plainLyrics":"Fallback"}});
        let found = parse_lyrics_response(&format!("```json\n{event}\n```"));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].provider, "fixture");
        assert_eq!(found[0].format, "ttml");
        assert_eq!(found[0].sync_type, LyricsSyncType::Synced);
        assert_eq!(found[1].format, "txt");
        assert!(parse_lyrics_response("```xml\n<tt invalid\n```").is_empty());
        let messy = "````sql\nxml ``` <tt><body><p begin='1s' end='2s'>First&amp;#x20;line</p></body></tt> ```\n````";
        let parsed = parse_lyrics_response(messy);
        assert_eq!(parsed[0].format, "ttml");
        assert_eq!(parsed[0].lyrics.lines[0].text, "First line");
    }
    #[test]
    fn parses_plain_and_timed_paste() {
        assert_eq!(parse("Hello\nWorld").lines.len(), 2);
        assert_eq!(parse("[00:02.00]Hello").lines[0].start_ms, 2000);
    }
}
