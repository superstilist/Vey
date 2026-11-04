//! Import public Spotify playlists through their unauthenticated embed page.
//! This path reads metadata only and resolves every row to a YouTube Music ID.
use crate::{
    library_import::{local_id, ImportedPlaylist, Provider},
    native::{self, YtmSong},
    services::Track,
};
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{LazyLock, Mutex},
    time::Duration,
};
use tokio::task::JoinSet;

const MAX_EMBED_BYTES: usize = 4 * 1024 * 1024;
const MAX_TRACKS: usize = 20_000;
const EMBED_TRACK_LIMIT: usize = 100;
const MATCH_CONCURRENCY: usize = 3;
const MATCH_TIMEOUT: Duration = Duration::from_secs(180);
const CACHE_FILE: &str = "spotify-import-matches.json";
static MATCH_GATE: LazyLock<tokio::sync::Semaphore> =
    LazyLock::new(|| tokio::sync::Semaphore::new(MATCH_CONCURRENCY));
static CACHE_WRITE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone)]
struct Candidate {
    id: String,
    title: String,
    artist: String,
    duration_secs: f64,
}

fn valid_spotify_id(id: &str) -> bool {
    id.len() == 22 && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_youtube_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalized_artist(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let without_channel_suffix = [" - topic", " topic", " - vevo", " vevo"]
        .iter()
        .find_map(|suffix| lower.strip_suffix(suffix))
        .unwrap_or(&lower);
    normalized(without_channel_suffix)
}

fn cache_path() -> std::path::PathBuf {
    crate::storage::data_dir().join(CACHE_FILE)
}

fn load_matches() -> BTreeMap<String, String> {
    let path = cache_path();
    if path
        .metadata()
        .map(|meta| meta.len() > 4 * 1024 * 1024)
        .unwrap_or(true)
    {
        return BTreeMap::new();
    }
    let Ok(bytes) = std::fs::read(path) else {
        return BTreeMap::new();
    };
    let Ok(mut matches) = serde_json::from_slice::<BTreeMap<String, String>>(&bytes) else {
        return BTreeMap::new();
    };
    matches.retain(|source, target| valid_spotify_id(source) && valid_youtube_id(target));
    matches
        .into_iter()
        .take(MAX_TRACKS)
        .collect::<BTreeMap<_, _>>()
}

fn save_matches(matches: &BTreeMap<String, String>) -> Result<(), String> {
    let _guard = CACHE_WRITE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut merged = load_matches();
    for (source, target) in matches {
        if valid_spotify_id(source) && valid_youtube_id(target) {
            merged
                .entry(source.clone())
                .or_insert_with(|| target.clone());
        }
        if merged.len() >= MAX_TRACKS {
            break;
        }
    }
    let dir = crate::storage::data_dir();
    std::fs::create_dir_all(&dir).map_err(|_| "Не вдалося створити локальний кеш імпорту")?;
    let path = dir.join(CACHE_FILE);
    let pending = dir.join("spotify-import-matches.pending.json");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options
        .open(&pending)
        .map_err(|_| "Не вдалося зберегти кеш імпорту")?;
    file.write_all(&serde_json::to_vec(&merged).map_err(|_| "Помилка формату кешу імпорту")?)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Не вдалося зберегти кеш імпорту")?;
    drop(file);
    std::fs::rename(pending, path).map_err(|_| "Не вдалося оновити кеш імпорту".into())
}

pub(crate) fn migrate_legacy_matches(raw: &Value) {
    let Some(legacy) = raw.pointer("/spotify/matches").and_then(Value::as_object) else {
        return;
    };
    let mut matches = load_matches();
    for (source, target) in legacy {
        let Some(target) = target.as_str() else {
            continue;
        };
        if valid_spotify_id(source) && valid_youtube_id(target) {
            matches
                .entry(source.clone())
                .or_insert_with(|| target.to_owned());
        }
        if matches.len() >= MAX_TRACKS {
            break;
        }
    }
    let _ = save_matches(&matches);
}

async fn public_embed(id: &str) -> Result<(String, Vec<Value>), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Не вдалося створити підключення до Spotify")?;
    let url = format!("https://open.spotify.com/embed/playlist/{id}");
    let response = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130 Safari/537.36",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|_| {
            "Не вдалося відкрити публічний плейліст Spotify. Спробуйте пізніше.".to_owned()
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            403 | 404 => {
                "Плейліст Spotify приватний, видалений або недоступний у цьому регіоні.".into()
            }
            429 => format!(
                "Spotify обмежив частоту запитів. Повторіть через {} с.",
                response
                    .headers()
                    .get("retry-after")
                    .and_then(|header| header.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|seconds| seconds.min(86_400).to_string())
                    .unwrap_or_else(|| "60".into())
            ),
            code => format!("Spotify повернув HTTP {code} під час читання публічного плейліста."),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_EMBED_BYTES as u64)
    {
        return Err("Сторінка Spotify завелика для безпечного імпорту.".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Не вдалося дочитати публічний плейліст Spotify")?;
        if bytes.len().saturating_add(chunk.len()) > MAX_EMBED_BYTES {
            return Err("Сторінка Spotify завелика для безпечного імпорту.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let html = String::from_utf8(bytes)
        .map_err(|_| "Spotify повернув сторінку в невідомому кодуванні.")?;
    let marker = "<script id=\"__NEXT_DATA__\" type=\"application/json\">";
    let start = html
        .find(marker)
        .map(|position| position + marker.len())
        .ok_or("Spotify змінив формат публічної сторінки; плейліст не імпортовано.")?;
    let end = html[start..]
        .find("</script>")
        .map(|position| start + position)
        .ok_or("Некоректна публічна сторінка Spotify; плейліст не імпортовано.")?;
    if end - start > MAX_EMBED_BYTES / 2 {
        return Err("Дані плейліста Spotify завеликі для безпечного імпорту.".into());
    }
    let state: Value = serde_json::from_str(&html[start..end])
        .map_err(|_| "Spotify змінив формат даних плейліста; імпорт не виконано.")?;
    let entity = state
        .pointer("/props/pageProps/state/data/entity")
        .filter(|entity| entity["type"] == "playlist" && entity["id"] == id)
        .ok_or("Spotify не надав дані цього публічного плейліста.")?;
    let name = entity["name"]
        .as_str()
        .filter(|name| !name.trim().is_empty())
        .ok_or("Spotify не надав назву плейліста.")?
        .to_owned();
    let rows = entity["trackList"]
        .as_array()
        .ok_or("Spotify не надав список композицій.")?
        .clone();
    if rows.len() > MAX_TRACKS {
        return Err("Плейліст перевищує ліміт 20 000 композицій; змін не внесено.".into());
    }
    // The public embed returns no authoritative total count. At its 100-row
    // ceiling we cannot distinguish a complete list from a truncated one.
    if rows.len() >= EMBED_TRACK_LIMIT {
        return Err("Spotify повернув 100 рядків без загальної кількості. Щоб не зберегти обрізаний плейліст, імпорт скасовано; спробуйте список коротший за 100 композицій.".into());
    }
    Ok((name, rows))
}

fn candidate(row: &Value) -> Result<Candidate, String> {
    if row["entityType"] != "track" {
        return Err(
            "Плейліст містить епізод або інший елемент без відповідника в YouTube Music.".into(),
        );
    }
    let id = row["uri"]
        .as_str()
        .and_then(|uri| uri.strip_prefix("spotify:track:"))
        .filter(|id| valid_spotify_id(id))
        .ok_or("Spotify повернув некоректний ID композиції.")?;
    let title = row["title"]
        .as_str()
        .filter(|title| !title.trim().is_empty())
        .ok_or("Spotify повернув композицію без назви.")?;
    let artist = row["subtitle"]
        .as_str()
        .filter(|artist| !artist.trim().is_empty())
        .ok_or("Spotify повернув композицію без артиста.")?;
    let duration_ms = row["duration"]
        .as_u64()
        .filter(|duration| *duration > 0)
        .ok_or("Spotify не надав тривалість композиції для точного зіставлення.")?;
    Ok(Candidate {
        id: id.into(),
        title: title.into(),
        artist: artist.into(),
        duration_secs: duration_ms as f64 / 1000.0,
    })
}

fn youtube_track(song: YtmSong) -> Track {
    Track {
        path: format!("yt://{}", song.video_id),
        title: song.title,
        artist: song.artist,
        album: song.album,
        duration_secs: song.duration_secs as f64,
        video_id: Some(song.video_id),
        thumbnail_url: song.thumbnail_url,
        ..Default::default()
    }
}

fn cached_track(candidate: &Candidate, video_id: &str) -> Track {
    Track {
        path: format!("yt://{video_id}"),
        title: candidate.title.clone(),
        artist: candidate.artist.clone(),
        duration_secs: candidate.duration_secs,
        video_id: Some(video_id.into()),
        ..Default::default()
    }
}

async fn exact_match(candidate: Candidate) -> Result<(String, Track), String> {
    let songs = native::ytm_match_candidates(
        format!("{} {}", candidate.title, candidate.artist),
        Some(24),
    )
    .await?;
    let title_key = normalized(&candidate.title);
    let artist_key = normalized_artist(&candidate.artist);
    let primary_artist_key = normalized_artist(
        candidate
            .artist
            .split(',')
            .next()
            .unwrap_or(&candidate.artist),
    );
    let mut matched = HashMap::<String, YtmSong>::new();
    for song in songs.into_iter().filter(|song| {
        song.kind == "song"
            && normalized(&song.title) == title_key
            && (normalized_artist(&song.artist) == artist_key
                || normalized_artist(&song.artist) == primary_artist_key)
            && (song.duration_secs as f64 - candidate.duration_secs).abs() < 8.0
            && valid_youtube_id(&song.video_id)
    }) {
        matched.entry(song.video_id.clone()).or_insert(song);
    }
    match matched.len() {
        0 => Err(format!(
            "Не знайдено точний збіг у YouTube Music: {} — {}.",
            candidate.artist, candidate.title
        )),
        1 => {
            let (_, song) = matched.into_iter().next().expect("one match");
            let video_id = song.video_id.clone();
            Ok((video_id, youtube_track(song)))
        }
        _ => Err(format!(
            "Неоднозначний збіг у YouTube Music: {} — {}. Плейліст не імпортовано.",
            candidate.artist, candidate.title
        )),
    }
}

pub async fn import_playlist(id: String) -> Result<ImportedPlaylist, String> {
    if !valid_spotify_id(&id) {
        return Err("Некоректний ID плейліста Spotify.".into());
    }
    let (name, rows) = public_embed(&id).await?;
    let mut match_cache = load_matches();
    let mut row_ids = Vec::with_capacity(rows.len());
    let mut candidates = Vec::new();
    let mut candidate_ids = HashSet::new();
    let mut cached_tracks = HashMap::<String, Track>::new();
    for row in rows {
        let candidate = candidate(&row)?;
        row_ids.push(candidate.id.clone());
        if let Some(video_id) = match_cache
            .get(&candidate.id)
            .filter(|video_id| valid_youtube_id(video_id))
        {
            cached_tracks
                .entry(candidate.id.clone())
                .or_insert_with(|| cached_track(&candidate, video_id));
        } else if candidate_ids.insert(candidate.id.clone()) {
            candidates.push(candidate);
        }
    }
    let mut matches = JoinSet::new();
    for candidate in candidates {
        matches.spawn(async move {
            let _permit = MATCH_GATE.acquire().await.map_err(|_| "Імпорт зупинено.")?;
            exact_match(candidate).await
        });
    }
    let mut resolved = cached_tracks;
    let mut failures = Vec::new();
    let deadline = tokio::time::Instant::now() + MATCH_TIMEOUT;
    loop {
        match tokio::time::timeout_at(deadline, matches.join_next()).await {
            Err(_) => {
                matches.abort_all();
                failures.push("Зіставлення перевищило ліміт часу; плейліст не імпортовано.".into());
                break;
            }
            Ok(None) => break,
            Ok(Some(result)) => match result {
                Ok(Ok((source_id, track))) => {
                    if let Some(video_id) = track.video_id.clone() {
                        match_cache.insert(source_id.clone(), video_id);
                    }
                    resolved.insert(source_id, track);
                }
                Ok(Err(error)) => failures.push(error),
                Err(_) => failures.push("Не вдалося завершити зіставлення композиції.".into()),
            },
        }
    }
    if match_cache.len() > MAX_TRACKS {
        match_cache = match_cache.into_iter().rev().take(MAX_TRACKS).collect();
    }
    // Keep successful individual mappings after an all-or-nothing playlist
    // failure so a retry does not repeat work already verified as exact.
    let _ = save_matches(&match_cache);
    if !failures.is_empty() {
        let failure_count = failures.len();
        failures.truncate(5);
        let suffix = if failure_count > 5 {
            format!(" Ще помилок: {}.", failure_count - 5)
        } else {
            String::new()
        };
        return Err(format!(
            "Spotify плейліст не імпортовано повністю: {}{}",
            failures.join(" "),
            suffix
        ));
    }
    if row_ids.len() != resolved.len() && row_ids.iter().any(|id| !resolved.contains_key(id)) {
        return Err("Не вдалося зіставити всі композиції; плейліст не збережено.".into());
    }
    let tracks = row_ids
        .iter()
        .map(|id| {
            resolved.get(id).cloned().ok_or_else(|| {
                "Не вдалося відновити порядок композицій; імпорт скасовано.".to_owned()
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ImportedPlaylist {
        id: local_id(Provider::Spotify, "public", &id),
        name,
        tracks,
    })
}
