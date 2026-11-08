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

const MANUAL_AUTH_REQUIRED: &str = "Dacubeking: потрібна ручна перевірка Turnstile у налаштуваннях слів";

// There is no silent Turnstile flow: use a valid saved token, or ask for the
// existing user-initiated browser verification. Other sources run independently.
async fn ensure_jwt() -> Result<String, String> {
    saved_token_or_manual(&JwtCache::load())
}

fn saved_token_or_manual(cache: &JwtCache) -> Result<String, String> {
    cache.token().ok_or_else(|| MANUAL_AUTH_REQUIRED.into())
}

/// Check the locally saved credential; verification is user initiated.
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
    fetch_with_progress_inner(track, provider, refresh, jwt, false, progress).await
}

/// The native player calls this after the user explicitly selects Dacubeking.
/// That selection may open the provider's real Turnstile page; automatic
/// discovery and server requests never open a browser as a side effect.
pub(crate) async fn fetch_with_interactive_auth(
    track: &Track,
    provider: &str,
    refresh: bool,
    jwt: Option<String>,
    progress: impl FnMut(Vec<LyricsAlternative>) + Send,
) -> Result<Vec<LyricsAlternative>, String> {
    let allow_interactive_auth = provider == "dacubeking";
    fetch_with_progress_inner(track, provider, refresh, jwt, allow_interactive_auth, progress).await
}

async fn fetch_with_progress_inner(
    track: &Track,
    provider: &str,
    refresh: bool,
    jwt: Option<String>,
    allow_interactive_auth: bool,
    progress: impl FnMut(Vec<LyricsAlternative>) + Send,
) -> Result<Vec<LyricsAlternative>, String> {
    // On a partial automatic refresh, preserve previously cached providers
    // before the new cache file is written.
    let previous = (refresh && matches!(provider, "auto" | "kugou")).then(|| load_cache(track)).flatten();
    if !refresh && !matches!(provider, "betterlyrics" | "kugou") {
        if let Some(c) = load_cache(track) {
            return Ok(c);
        }
    }
    let mut local = Vec::new();
    if !matches!(provider, "betterlyrics" | "kugou") && !track.path.starts_with("yt://") {
        for ext in ["ttml", "lrc", "qrc", "srt", "vtt", "txt"] {
            let path = std::path::Path::new(&track.path).with_extension(ext);
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Some(a) = alternative("Локальний файл", &raw) {
                    local.push(a);
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
            fetch_primary(track, jwt, allow_interactive_auth).await
        } else {
            (vec![], vec![])
        }
    };
    let betterlyrics = async {
        if provider == "auto" || provider == "betterlyrics" {
            fetch_betterlyrics_chain(track, &client, "https://api.betterlyrics.org").await
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
    let kugou = async {
        if provider == "auto" || provider == "kugou" {
            match tokio::time::timeout(std::time::Duration::from_secs(12), fetch_kugou(track, &client, "https://mobileservice.kugou.com", "https://lyrics.kugou.com")).await {
                Ok(result) => result,
                Err(_) => (vec![], vec!["KuGou: час очікування вичерпано".into()]),
            }
        } else {
            (vec![], vec![])
        }
    };
    let (mut found, mut errors) =
        collect_provider_results(primary, betterlyrics, unison, lrclib, kugou, track.duration_secs, progress).await;
    found.extend(local);
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
    if let Some(previous) = previous {
        found = crate::braccato::providers::merge_refreshed_alternatives(found, &previous);
    }
    // A full/read-only cache must not discard successfully downloaded lyrics.
    let _ = save_cache(track, &found);
    Ok(found)
}

type ProviderResult = (Vec<LyricsAlternative>, Vec<String>);

async fn collect_provider_results(
    primary: impl std::future::Future<Output = ProviderResult>,
    betterlyrics: impl std::future::Future<Output = ProviderResult>,
    unison: impl std::future::Future<Output = ProviderResult>,
    lrclib: impl std::future::Future<Output = ProviderResult>,
    kugou: impl std::future::Future<Output = ProviderResult>,
    duration_secs: f64,
    mut progress: impl FnMut(Vec<LyricsAlternative>),
) -> ProviderResult {
    let mut found = Vec::new();
    let mut errors = Vec::new();
    tokio::pin!(primary, betterlyrics, unison, lrclib, kugou);
    let mut done = [false; 5];
    while done.iter().any(|finished| !finished) {
        let (index, (alternatives, failures)) = tokio::select! {
            value = &mut primary, if !done[0] => (0, value),
            value = &mut betterlyrics, if !done[1] => (1, value),
            value = &mut unison, if !done[2] => (2, value),
            value = &mut lrclib, if !done[3] => (3, value),
            value = &mut kugou, if !done[4] => (4, value),
        };
        done[index] = true;
        #[cfg(debug_assertions)]
        eprintln!("[lyrics-debug] {}: {} result(s), {} error(s)", ["Dacubeking", "Better Lyrics", "Unison", "LRCLib", "KuGou"][index], alternatives.len(), failures.len());
        errors.extend(failures);
        if !alternatives.is_empty() {
            found.extend(alternatives);
            prepare_alternatives(&mut found, duration_secs);
            progress(found.clone());
        }
    }
    (found, errors)
}

fn betterlyrics_query(track: &Track, include_video_id: bool) -> Vec<(&'static str, String)> {
    let duration = if track.duration_secs.is_finite() && track.duration_secs > 0.0 {
        track.duration_secs.round() as u64
    } else {
        0
    };
    let mut query = vec![
        ("s", track.title.trim().to_string()),
        ("a", track.artist.trim().to_string()),
        ("al", track.album.trim().to_string()),
        ("d", duration.to_string()),
    ];
    if include_video_id {
        if let Some(id) = crate::services::track_video_id(track) {
            query.push(("videoId", id.to_string()));
        }
    }
    query
}

/// Better Lyrics' current cache-first API uses an API key, not the legacy JWT.
async fn fetch_betterlyrics(track: &Track, client: &reqwest::Client, endpoint: &str) -> ProviderResult {
    if track.title.trim().is_empty() && track.artist.trim().is_empty() {
        return (vec![], vec![]);
    }
    let query = betterlyrics_query(track, true);
    let response = client.get(endpoint).query(&query).send().await;
    #[cfg(debug_assertions)]
    match &response {
        Ok(response) => eprintln!("[lyrics-debug] Better Lyrics TTML: HTTP {}", response.status().as_u16()),
        Err(_) => eprintln!("[lyrics-debug] Better Lyrics TTML: connection error"),
    }
    match response {
        Ok(response) if response.status().is_success() => {
            let parsed = response.json::<serde_json::Value>().await.ok()
                .and_then(|value| value.get("ttml").and_then(|v| v.as_str()).and_then(|raw| alternative("Better Lyrics", raw)));
            match parsed {
                Some(lyrics) => (vec![lyrics], vec![]),
                None => (vec![], vec!["Better Lyrics: некоректна відповідь зі словами".into()]),
            }
        }
        Ok(response) if response.status().as_u16() == 401 => (vec![], vec![
            "Better Lyrics: слова не в кеші сервісу; потрібен API-ключ".into(),
        ]),
        Ok(response) if response.status().as_u16() == 404 => (vec![], vec![]),
        Ok(response) => (vec![], vec![format!("Better Lyrics HTTP {}", response.status().as_u16())]),
        Err(_) => (vec![], vec!["Better Lyrics: помилка з’єднання".into()]),
    }
}

async fn fetch_betterlyrics_fallback(
    track: &Track, client: &reqwest::Client, endpoint: &str, name: &str,
) -> ProviderResult {
    let response = client.get(endpoint).query(&betterlyrics_query(track, false)).send().await;
    #[cfg(debug_assertions)]
    match &response {
        Ok(response) => eprintln!("[lyrics-debug] {name}: HTTP {}", response.status().as_u16()),
        Err(_) => eprintln!("[lyrics-debug] {name}: connection error"),
    }
    match response {
        Ok(response) if response.status().is_success() => {
            let parsed = response.json::<serde_json::Value>().await.ok()
                .and_then(|value| value.get("lyrics").and_then(|v| v.as_str()).and_then(|raw| alternative(name, raw)));
            match parsed {
                Some(lyrics) => (vec![lyrics], vec![]),
                None => (vec![], vec![format!("{name}: некоректна відповідь зі словами")]),
            }
        }
        Ok(response) if response.status().as_u16() == 404 => (vec![], vec![]),
        Ok(response) if response.status().as_u16() == 401 => (vec![], vec!["Better Lyrics: слова не в кеші сервісу; потрібен API-ключ".into()]),
        Ok(response) => (vec![], vec![format!("{name} HTTP {}", response.status().as_u16())]),
        Err(_) => (vec![], vec![format!("{name}: помилка з’єднання")]),
    }
}

async fn fetch_betterlyrics_chain(track: &Track, client: &reqwest::Client, base: &str) -> ProviderResult {
    let (found, mut errors) = fetch_betterlyrics(track, client, &format!("{base}/getLyrics")).await;
    if !found.is_empty() || track.title.trim().is_empty() && track.artist.trim().is_empty() {
        return (found, errors);
    }
    // A missing cached TTML entry need not mean the provider-specific caches
    // are empty. Avoid multiplying requests on connection/rate-limit errors.
    if !errors.is_empty() && !errors.iter().any(|error| error.contains("потрібен API-ключ")) {
        return (found, errors);
    }
    let qq_url = format!("{base}/qq/getLyrics");
    let kugou_url = format!("{base}/kugou/getLyrics");
    let (qq, kugou) = tokio::join!(
        fetch_betterlyrics_fallback(track, client, &qq_url, "Better Lyrics · QQ"),
        fetch_betterlyrics_fallback(track, client, &kugou_url, "Better Lyrics · Kugou"),
    );
    let mut alternatives = qq.0;
    alternatives.extend(kugou.0);
    for error in qq.1.into_iter().chain(kugou.1) {
        if !errors.contains(&error) { errors.push(error); }
    }
    (alternatives, errors)
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
    allow_interactive_auth: bool,
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
            Some(jwt) => Ok(jwt),
            None if allow_interactive_auth => acquire_interactive_jwt().await,
            None => ensure_jwt().await,
        };
        match authentication {
            Ok(token) => {
                let params = v2_lyrics_form(track, &id, &token);
                let mut response = native::fetch_lyrics_get(params, Some(BASE.into())).await;
                if response.as_ref().is_err_and(|e| e == "v2/lyrics HTTP 401") {
                    invalidate_jwt(&token);
                    response = if allow_interactive_auth {
                        match acquire_interactive_jwt().await {
                            Ok(fresh) => native::fetch_lyrics_get(
                                v2_lyrics_form(track, &id, &fresh),
                                Some(BASE.into()),
                            )
                            .await,
                            Err(error) => Err(error),
                        }
                    } else {
                        match JwtCache::load().token().filter(|fresh| fresh != &token) {
                            Some(fresh) => {
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
                            None => Err(MANUAL_AUTH_REQUIRED.into()),
                        }
                    };
                }
                match response {
                    Ok(Some(raw)) => found.extend(parse_lyrics_response(&raw)),
                    Err(e) => errors.push(e),
                    _ => {}
                }
            }
            Err(e) => errors.push(e),
        }
    }

    (found, errors)
}

async fn acquire_interactive_jwt() -> Result<String, String> {
    let turnstile_token = crate::challenge::acquire().await?;
    let mut cache = JwtCache::load();
    cache.exchange(turnstile_token).await?;
    cache
        .token()
        .ok_or_else(|| "Dacubeking: перевірка не повернула чинний JWT".into())
}

async fn kugou_json(mut response: reqwest::Response) -> Result<serde_json::Value, ()> {
    const MAX_BYTES: usize = 2_000_000;
    if response.content_length().is_some_and(|size| size > MAX_BYTES as u64) {
        return Err(());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
            return Err(());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| ())
}

/// KuGou's public song and lyric lookup, using exact metadata and duration
/// matching so a cover or a different recording is not silently substituted.
async fn fetch_kugou(track: &Track, client: &reqwest::Client, songs_base: &str, lyrics_base: &str) -> ProviderResult {
    let normalize = |text: &str| -> String {
        text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
    };
    let title = normalize(&track.title);
    let artist = normalize(&track.artist);
    if title.is_empty() || artist.is_empty() {
        return (vec![], vec![]);
    }
    let keyword = format!("{} - {}", track.title.trim(), track.artist.trim());
    let response = match client.get(format!("{songs_base}/api/v3/search/song"))
        .query(&[("version", "9108"), ("plat", "0"), ("pagesize", "8"), ("showtype", "0"), ("keyword", keyword.as_str())])
        .send().await {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => return (vec![], vec![format!("KuGou HTTP {}", response.status().as_u16())]),
        Err(_) => return (vec![], vec!["KuGou: помилка з’єднання".into()]),
    };
    let songs = match kugou_json(response).await {
        Ok(value) => value,
        Err(_) => return (vec![], vec!["KuGou: некоректна відповідь пошуку".into()]),
    };
    let song = songs.pointer("/data/info").and_then(|value| value.as_array()).and_then(|songs| {
        songs.iter().take(8).find(|song| {
            let same_title = song.get("songname").and_then(|v| v.as_str()).is_some_and(|v| normalize(v) == title);
            let same_artist = song.get("singername").and_then(|v| v.as_str()).is_some_and(|v| normalize(v) == artist);
            let same_duration = !track.duration_secs.is_finite() || track.duration_secs <= 0.0
                || song.get("duration").and_then(|v| v.as_f64()).is_some_and(|v| (v - track.duration_secs).abs() <= 8.0);
            same_title && same_artist && same_duration
        })
    });
    let Some(hash) = song.and_then(|song| song.get("hash")).and_then(|v| v.as_str())
        .filter(|hash| hash.len() == 32 && hash.bytes().all(|c| c.is_ascii_hexdigit())) else {
        return (vec![], vec![]);
    };
    let response = match client.get(format!("{lyrics_base}/search"))
        .query(&[("ver", "1"), ("man", "yes"), ("client", "pc"), ("hash", hash)])
        .send().await {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => return (vec![], vec![format!("KuGou HTTP {}", response.status().as_u16())]),
        Err(_) => return (vec![], vec!["KuGou: помилка з’єднання".into()]),
    };
    let candidates = match kugou_json(response).await {
        Ok(value) => value,
        Err(_) => return (vec![], vec!["KuGou: некоректна відповідь пошуку слів".into()]),
    };
    let Some(candidate) = candidates.get("candidates").and_then(|v| v.as_array()).and_then(|v| v.first()) else {
        return (vec![], vec![]);
    };
    let id = candidate.get("id").and_then(|v| v.as_u64().map(|n| n.to_string()).or_else(|| v.as_str().map(str::to_string)));
    let key = candidate.get("accesskey").and_then(|v| v.as_str());
    let (Some(id), Some(key)) = (id.filter(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit())), key.filter(|key| !key.is_empty() && key.len() <= 512)) else {
        return (vec![], vec![]);
    };
    let response = match client.get(format!("{lyrics_base}/download"))
        .query(&[("fmt", "lrc"), ("charset", "utf8"), ("client", "pc"), ("ver", "1"), ("id", id.as_str()), ("accesskey", key)])
        .send().await {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => return (vec![], vec![format!("KuGou HTTP {}", response.status().as_u16())]),
        Err(_) => return (vec![], vec!["KuGou: помилка з’єднання".into()]),
    };
    let decoded = kugou_json(response).await.ok()
        .and_then(|value| value.get("content").and_then(|v| v.as_str()).filter(|text| text.len() <= 1_500_000).map(str::to_string))
        .and_then(|text| base64::engine::general_purpose::STANDARD.decode(text).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| alternative("KuGou", &text))
        .filter(|alt| alt.sync_type != LyricsSyncType::Plain);
    match decoded {
        Some(lyrics) => (vec![lyrics], vec![]),
        None => (vec![], vec!["KuGou: немає придатних синхронізованих слів".into()]),
    }
}

async fn fetch_unison(
    track: &Track,
    client: &reqwest::Client,
) -> (Vec<LyricsAlternative>, Vec<String>) {
    let mut found = vec![];
    let mut errors = vec![];

    if let Some(id) = crate::services::track_video_id(track) {
        match client.get(format!("https://unison.boidu.dev/lyrics/variants/{id}"))
            .query(&[("limit", "50")]).send().await {
            Ok(response) if response.status().is_success() => {
                if let Ok(body) = response.json::<serde_json::Value>().await {
                    found.extend(parse_unison_versions(&body, id));
                }
            }
            Ok(response) if response.status().as_u16() >= 429 => errors.push(format!("Unison HTTP {}", response.status())),
            Err(_) => errors.push("Unison: помилка з’єднання".into()),
            _ => {}
        }
        if !found.is_empty() { return (found, errors); }
    }

    let mut variants = vec![vec![
        ("song", track.title.as_str()),
        ("artist", track.artist.as_str()),
    ]];
    if let Some(id) = crate::services::track_video_id(track) {
        variants.insert(0, vec![("v", id)]);
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
                    if let Some(id) = v["data"]["videoId"].as_str().filter(|id| crate::services::video_id(id)) {
                        if let Ok(response) = client.get(format!("https://unison.boidu.dev/lyrics/variants/{id}"))
                            .query(&[("limit", "50")]).send().await {
                            if response.status().is_success() {
                                if let Ok(body) = response.json::<serde_json::Value>().await {
                                    found.extend(parse_unison_versions(&body, id));
                                }
                            }
                        }
                        if !found.is_empty() { break; }
                    }
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

fn parse_unison_versions(body: &serde_json::Value, video_id: &str) -> Vec<LyricsAlternative> {
    if body["success"].as_bool() != Some(true) { return Vec::new(); }
    body["data"].as_array().into_iter().flatten().take(50)
        .filter(|item| item["videoId"].as_str() == Some(video_id) && item["hidden"].as_bool() != Some(true))
        .filter_map(|item| item["lyrics"].as_str().and_then(|raw| alternative("Unison", raw)))
        .collect()
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

    #[test]
    fn cached_kugou_supplement_respects_local_and_richer_lyrics() {
        let line = alternative("LRCLib", "[00:01.00]Line").unwrap();
        assert!(crate::braccato::providers::should_supplement_kugou(&[line]));
        let local = alternative("Власні слова", "[00:01.00]Mine").unwrap();
        assert!(!crate::braccato::providers::should_supplement_kugou(&[local]));
        let kugou = alternative("KuGou", "[00:01.00]Line").unwrap();
        assert!(!crate::braccato::providers::should_supplement_kugou(&[kugou]));
        let rich = alternative("qq", "[1000,2000]Hello(1000,1000)World(2000,1000)").unwrap();
        assert!(!crate::braccato::providers::should_supplement_kugou(&[rich]));
    }

    #[tokio::test]
    async fn kugou_uses_matching_artist_title_duration_and_downloads_lrc() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut bytes = [0; 4096];
                let size = stream.read(&mut bytes).unwrap();
                let request = String::from_utf8_lossy(&bytes[..size]).into_owned();
                let body = if request.starts_with("GET /api/v3/search/song?") {
                    serde_json::json!({"data":{"info":[
                        {"songname":"Song", "singername":"Different", "duration":123, "hash":"11111111111111111111111111111111"},
                        {"songname":"Song", "singername":"Artist", "duration":300, "hash":"22222222222222222222222222222222"},
                        {"songname":"Song", "singername":"Artist", "duration":124, "hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
                    ]}}).to_string()
                } else if request.starts_with("GET /search?") {
                    serde_json::json!({"candidates":[{"id":123,"accesskey":"fixture"}]}).to_string()
                } else {
                    serde_json::json!({"content":base64::engine::general_purpose::STANDARD.encode("[00:01.00]Hello")}).to_string()
                };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                requests.push(request);
            }
            requests
        });
        let track = Track { title:"Song".into(), artist:"Artist".into(), duration_secs:123.0, ..Default::default() };
        let (found, errors) = fetch_kugou(&track, &reqwest::Client::new(), &base, &base).await;
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].provider, "KuGou");
        assert_eq!(found[0].lyrics.lines[0].text, "Hello");
        assert_ne!(found[0].sync_type, LyricsSyncType::Plain);
        let requests = server.join().unwrap();
        assert!(requests[1].contains("hash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(requests[2].starts_with("GET /download?"));
        assert!(requests[2].contains("fmt=lrc"));
    }

    #[tokio::test]
    #[ignore = "requires the public KuGou service and this song to remain available"]
    async fn kugou_live_sample_fetches_synchronized_lyrics() {
        let track = Track { title:"Hacking to the Gate".into(), artist:"いとうかなこ".into(), duration_secs:257.0, ..Default::default() };
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(12)).build().unwrap();
        let (found, errors) = fetch_kugou(&track, &client, "https://mobileservice.kugou.com", "https://lyrics.kugou.com").await;
        assert!(!found.is_empty(), "KuGou returned no parsable lyrics: {errors:?}");
        assert_eq!(found[0].provider, "KuGou");
        assert_ne!(found[0].sync_type, LyricsSyncType::Plain);
        assert!(found[0].lyrics.lines.len() > 5);
    }

    #[tokio::test]
    #[ignore = "requires the public KuGou service and an isolated PLAYERVF_DATA_DIR"]
    async fn kugou_live_cached_refresh_preserves_previous_source() {
        assert!(std::env::var_os("PLAYERVF_DATA_DIR").is_some(), "set an isolated data directory");
        let track = Track { path:"fixture://kugou-live-sample".into(), title:"Hacking to the Gate".into(), artist:"いとうかなこ".into(), duration_secs:257.0, ..Default::default() };
        let previous = alternative("LRCLib", "[00:01.00]Fixture cached line").unwrap();
        save_cache(&track, &[previous]).unwrap();
        let found = fetch(&track, "kugou", true, None).await.expect("live KuGou request");
        assert!(found.iter().any(|alt| alt.provider == "KuGou" && alt.sync_type == LyricsSyncType::Synced));
        assert!(found.iter().any(|alt| alt.provider == "LRCLib"));
        let saved = load_cache(&track).unwrap();
        assert!(saved.iter().any(|alt| alt.provider == "KuGou"));
        assert!(saved.iter().any(|alt| alt.provider == "LRCLib"));
    }

    #[test]
    fn partial_refresh_preserves_cached_sources_and_prefers_fresh_updates() {
        let old = alternative("qq", "[00:01.00]Original").unwrap();
        let lrclib = alternative("LRCLib", "[00:02.00]Fallback").unwrap();
        let merged = crate::braccato::providers::merge_refreshed_alternatives(vec![lrclib], &[old]);
        assert!(merged.iter().any(|alt| alt.provider == "qq" && alt.lyrics.lines[0].text == "Original"));
        assert!(merged.iter().any(|alt| alt.provider == "LRCLib"));
        let updated = alternative("qq", "[00:01.00]Updated").unwrap();
        let merged = crate::braccato::providers::merge_refreshed_alternatives(vec![updated], &merged);
        let qq: Vec<_> = merged.iter().filter(|alt| alt.provider == "qq").collect();
        assert_eq!(qq.len(), 2);
        assert_eq!(qq[0].lyrics.lines[0].text, "Updated");
        assert_eq!(qq[1].lyrics.lines[0].text, "Original");
    }

    #[test]
    fn unison_collects_all_visible_versions_for_the_video() {
        let body = serde_json::json!({"success":true,"data":[
            {"videoId":"abcdefghijk","lyrics":"[00:01.00]First"},
            {"videoId":"abcdefghijk","lyrics":"Second"},
            {"videoId":"abcdefghijk","lyrics":"Hidden","hidden":true},
            {"videoId":"other","lyrics":"Other"}
        ]});
        let versions = parse_unison_versions(&body, "abcdefghijk");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].lyrics.lines[0].text, "First");
        assert_eq!(versions[1].lyrics.lines[0].text, "Second");
    }

    #[tokio::test]
    async fn betterlyrics_falls_back_to_qq_when_ttml_requires_key() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut bytes = [0; 4096];
                let size = stream.read(&mut bytes).unwrap();
                let request = String::from_utf8_lossy(&bytes[..size]).into_owned();
                let (status, body) = if request.starts_with("GET /qq/getLyrics?") {
                    (200, serde_json::json!({"lyrics":"<QrcInfos><LyricInfo><Lyric_1 LyricContent=\"[1000,1000]Hello(1000,1000)\"/></LyricInfo></QrcInfos>","provider":"qq"}).to_string())
                } else if request.starts_with("GET /kugou/getLyrics?") {
                    (404, "{}".to_string())
                } else {
                    (401, "{}".to_string())
                };
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                requests.push(request);
            }
            requests
        });
        let track = Track { path: "yt://abcdefghijk".into(), title: "Song".into(), artist: "Artist".into(), duration_secs: 123.0, ..Default::default() };
        let (found, _) = fetch_betterlyrics_chain(&track, &reqwest::Client::new(), &base).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].provider, "Better Lyrics · QQ");
        assert_eq!(found[0].lyrics.lines[0].text, "Hello");
        let requests = server.join().unwrap();
        assert!(requests.iter().any(|request| request.starts_with("GET /getLyrics?") && request.contains("videoId=abcdefghijk")));
        assert!(requests.iter().filter(|request| request.starts_with("GET /qq/") || request.starts_with("GET /kugou/")).all(|request| !request.contains("videoId=")));
    }

    #[tokio::test]
    #[ignore = "requires the public Better Lyrics service and its documented example to be available"]
    async fn betterlyrics_live_documented_example_parses_ttml_and_qq() {
        let track = Track { title: "E85".into(), artist: "Don Toliver".into(), album: "OCTANE".into(), duration_secs: 154.0, ..Default::default() };
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().unwrap();
        let (found, errors) = fetch_betterlyrics(&track, &client, "https://api.betterlyrics.org/getLyrics").await;
        assert!(!found.is_empty(), "Better Lyrics returned no parsable lyrics: {errors:?}");
        assert_eq!(found[0].format, "ttml");
        assert!(!found[0].lyrics.lines.is_empty());
        let (qq, errors) = fetch_betterlyrics_fallback(&track, &client, "https://api.betterlyrics.org/qq/getLyrics", "Better Lyrics · QQ").await;
        assert!(!qq.is_empty(), "Better Lyrics QQ returned no parsable lyrics: {errors:?}");
        assert_eq!(qq[0].format, "qrc");
        assert!(!qq[0].lyrics.lines.is_empty());
    }

    #[tokio::test]
    async fn betterlyrics_uses_public_contract_and_reports_cache_miss() {
        use std::io::{Read, Write};
        for status in [200, 401, 404] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}/getLyrics", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut request = [0; 4096];
                let size = stream.read(&mut request).unwrap();
                let body = if status == 200 {
                    serde_json::json!({"ttml":"<tt><body><div><p begin='1s' end='3s'>Hello</p></div></body></tt>"}).to_string()
                } else { "{}".to_string() };
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                String::from_utf8_lossy(&request[..size]).into_owned()
            });
            let track = Track { path: "yt://abcdefghijk".into(), title: "Song & More".into(), artist: "Artist".into(), album: "Album".into(), duration_secs: 123.4, ..Default::default() };
            let (found, errors) = fetch_betterlyrics(&track, &reqwest::Client::new(), &endpoint).await;
            match status {
                200 => { assert_eq!(found.len(), 1); assert_eq!(found[0].provider, "Better Lyrics"); assert_eq!(found[0].lyrics.lines[0].text, "Hello"); assert!(errors.is_empty()); }
                401 => { assert!(found.is_empty()); assert!(errors[0].contains("API-ключ")); }
                404 => { assert!(found.is_empty()); assert!(errors.is_empty()); }
                _ => unreachable!(),
            }
            let request = server.join().unwrap();
            assert!(request.starts_with("GET /getLyrics?"));
            assert!(request.contains("s=Song+%26+More"));
            assert!(request.contains("a=Artist") && request.contains("al=Album") && request.contains("d=123"));
            assert!(request.contains("videoId=abcdefghijk"));
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
        }
    }
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
            collect_provider_results(primary, async { (vec![], vec![]) }, public, unavailable, async { (vec![], vec![]) }, 10., |alts| {
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
    fn missing_jwt_requires_manual_verification() {
        assert_eq!(saved_token_or_manual(&JwtCache::default()).unwrap_err(), MANUAL_AUTH_REQUIRED);
        let saved = JwtCache { token: "fixture".into(), exp: crate::services::now_ms() / 1000 + 60 };
        assert_eq!(saved_token_or_manual(&saved).unwrap(), "fixture");
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
