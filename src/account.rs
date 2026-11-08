//! Vey account metadata and private audio. Credentials never enter Store/exports.
use crate::{
    services::{History, Playlist, RecommendationModel, Settings, Store, Track},
    MainWindow,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashSet},
    rc::Rc,
    time::Duration,
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Session {
    api: String,
    token: String,
    id: String,
    email: String,
    name: String,
    nickname: String,
    bio: String,
    discord: String,
    avatar_url: String,
    created_at: String,
    expires_at: u64,
    premium: bool,
    revision: Option<u64>,
    auto_sync: bool,
    auto_upload: bool,
    uploaded: BTreeSet<String>,
    /// Local acknowledgement of durable Blueberry queue submissions. This is
    /// never sent to the account API or shared catalog.
    catalog_uploaded: BTreeMap<String, String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CloudLibrary {
    tracks: Vec<Track>,
    likes: BTreeSet<String>,
    playlists: Vec<Playlist>,
    history: Vec<History>,
    #[serde(rename = "catalogLibraryKeys", skip_serializing_if = "BTreeSet::is_empty")]
    catalog_library_keys: BTreeSet<String>,
    recommendations: RecommendationModel,
    #[serde(serialize_with = "serialize_cloud_settings")]
    settings: Settings,
    offsets: BTreeMap<String, i64>,
    lyrics_choices: BTreeMap<String, crate::services::LyricsChoice>,
    #[serde(rename = "autoUpload")]
    auto_upload: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogImportTrack {
    path: String,
    title: String,
    artist: String,
    album: String,
    duration_secs: f64,
    thumbnail_url: String,
    clip_video_id: Option<String>,
    source_url: String,
    youtube_music_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_source_key: Option<String>,
    recording_type: String,
    lyrics_text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lyrics_tracks: Vec<CatalogLyricsTrack>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lyrics_variants: Vec<crate::lyrics::LyricsAlternative>,
}

pub(crate) fn recording_type(title: &str) -> &'static str {
    const MARKERS: &[(&str, &str)] = &[
        ("remix", "remix"), ("re mix", "remix"), ("mix", "mix"),
        ("live", "live"), ("acoustic", "acoustic"), ("instrumental", "instrumental"),
        ("cover", "cover"), ("karaoke", "mix"), ("demo", "mix"),
        ("radio edit", "mix"), ("remaster", "mix"), ("remastered", "mix"),
        ("sped up", "mix"), ("slowed", "mix"), ("nightcore", "mix"),
        ("8d", "mix"), ("bass boosted", "mix"), ("reverb", "mix"),
        ("chopped and screwed", "mix"), ("mashup", "mix"), ("bootleg", "mix"),
        ("fan made", "mix"), ("tribute", "mix"), ("extended mix", "mix"),
        ("club mix", "mix"), ("short", "mix"), ("teaser", "mix"),
        ("trailer", "mix"), ("preview", "mix"), ("loop", "mix"),
    ];
    fn has_phrase(text: &str, phrase: &str, suffix_only: bool) -> bool {
        let words = text.to_lowercase().split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let target = phrase.split_whitespace().collect::<Vec<_>>();
        if target.len() > words.len() { return false; }
        let matches = |window: &[String]| window.iter().map(String::as_str).eq(target.iter().copied());
        if suffix_only { matches(&words[words.len() - target.len()..]) }
        else { words.windows(target.len()).any(matches) }
    }

    let mut marked_segments = Vec::new();
    for (index, open) in title
        .char_indices()
        .filter(|(_, ch)| *ch == '(' || *ch == '[')
    {
        let close = if open == '(' { ')' } else { ']' };
        if let Some(end) = title[index + open.len_utf8()..].find(close) {
            marked_segments.push(&title[index + open.len_utf8()..index + open.len_utf8() + end]);
        }
    }
    if let Some((_, suffix)) = title.rsplit_once(" - ") { marked_segments.push(suffix); }
    for (marker, kind) in MARKERS {
        if marked_segments.iter().any(|segment| has_phrase(segment, marker, false))
            || has_phrase(title, marker, true)
        {
            return kind;
        }
    }
    "unknown"
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogLyricsTrack {
    kind: String,
    lang: String,
    text: String,
}

// Keep each request below the Worker's 1 MiB limit. A track can span requests;
// the caller acknowledges it only after its final fragment succeeds.
fn split_catalog_track(item: &CatalogImportTrack) -> Result<Vec<CatalogImportTrack>, String> {
    const LIMIT: usize = 890_000;
    let mut current = item.clone();
    let variants = std::mem::take(&mut current.lyrics_variants);
    let mut parts = Vec::new();
    for variant in variants {
        if serde_json::to_vec(&variant).map_err(|_| "Помилка тексту".to_string())?.len() > 800_000 {
            return Err("Одна версія тексту завелика для сервера каталогу".into());
        }
        current.lyrics_variants.push(variant);
        if current.lyrics_variants.len() > 100 || serde_json::to_vec(&current).map_err(|_| "Помилка каталогу".to_string())?.len() > LIMIT {
            let variant = current.lyrics_variants.pop().unwrap();
            parts.push(current);
            current = item.clone();
            current.lyrics_text.clear();
            current.lyrics_tracks.clear();
            current.lyrics_variants.clear();
            current.lyrics_variants.push(variant);
            if serde_json::to_vec(&current).map_err(|_| "Помилка каталогу".to_string())?.len() > LIMIT {
                return Err("Одна версія тексту завелика для сервера каталогу".into());
            }
        }
    }
    parts.push(current);
    Ok(parts)
}

async fn send_catalog_batch(s: &Session, batch: &[(String, String, CatalogImportTrack, bool)]) -> Result<(), String> {
    let payload = batch.iter().map(|(_, _, item, _)| item).collect::<Vec<_>>();
    let value = request(s, "catalog/import", "POST", Some(json!({"items":payload}))).await?;
    if value["catalogVersion"].as_u64() != Some(2) {
        return Err("Сервер каталогу ще не підтримує всі версії текстів".into());
    }
    if batch.iter().any(|(_, _, item, _)| item.path.starts_with("local://"))
        && value["localCatalogVersion"].as_u64() != Some(1) {
        return Err("Сервер каталогу ще не підтримує локальні записи".into());
    }
    if value["accepted"].as_u64().unwrap_or(0) as usize != batch.len() {
        return Err("Blueberry прийняв лише частину пакета каталогу".into());
    }
    Ok(())
}

fn catalog_lyrics_tracks(lyrics: &crate::lyrics::ParsedLyrics) -> Vec<CatalogLyricsTrack> {
    let source_language = lyrics.language.as_deref().unwrap_or("und");
    let romanization = lyrics
        .lines
        .iter()
        .map(|line| {
            if !line.romanization.trim().is_empty() {
                line.romanization.trim().to_string()
            } else {
                line.timed_romanization
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<String>()
            }
        })
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    if romanization.iter().any(|line| !line.trim().is_empty()) {
        result.push(CatalogLyricsTrack {
            kind: "romanization".into(),
            lang: format!("{}-Latn", source_language),
            text: romanization.join("\n"),
        });
    }

    let mut language_counts = BTreeMap::<String, usize>::new();
    for line in &lyrics.lines {
        for (language, text) in &line.translations {
            if !text.trim().is_empty() {
                *language_counts.entry(language.clone()).or_default() += 1;
            }
        }
    }
    let translation_language = language_counts
        .into_iter()
        .max_by(|(left_lang, left_count), (right_lang, right_count)| {
            left_count.cmp(right_count).then_with(|| right_lang.cmp(left_lang))
        })
        .map(|(language, _)| language);
    let translations = lyrics
        .lines
        .iter()
        .map(|line| {
            if !line.translation.trim().is_empty() {
                line.translation.trim().to_string()
            } else {
                translation_language
                    .as_ref()
                    .and_then(|language| line.translations.get(language))
                    .map(|text| text.trim().to_string())
                    .unwrap_or_default()
            }
        })
        .collect::<Vec<_>>();
    if translations.iter().any(|line| !line.trim().is_empty()) {
        result.push(CatalogLyricsTrack {
            kind: "translation".into(),
            lang: translation_language.unwrap_or_else(|| "und".into()),
            text: translations.join("\n"),
        });
    }
    result
}

// Store.tracks is a metadata cache as well as a library index. Only the
// library projection and actual playback events may contribute catalog songs.
fn catalog_sources(store: &Store) -> Vec<Track> {
    let mut tracks = store.library_tracks();
    tracks.extend(store.history.iter().filter(|event| {
        event.play_count > 0 || event.completion_count > 0 || event.played_at > 0
    }).map(|event| event.track.clone()));
    tracks
}

fn catalog_candidates(
    store: &Store,
    uploaded: &BTreeMap<String, String>,
) -> Vec<(String, String, CatalogImportTrack)> {
    use sha1::{Digest, Sha1};

    let identities = CloudIdentities::new(store);
    let tracks = catalog_sources(store);
    // A local file and its public track may each have different providers.
    let mut local_by_identity = BTreeMap::<String, Vec<&str>>::new();
    for track in &tracks {
        if let Some(id) = identities.track(track).map(|track| track.path) {
            local_by_identity.entry(id).or_default().push(track.path.as_str());
        }
    }
    let mut unique = BTreeMap::<String, Track>::new();
    for track in &tracks {
        let Some(track) = identities.track(track) else {
            continue;
        };
        if !track.path.starts_with("yt://")
            || recording_type(track.catalog_metadata.as_ref().map_or(&track.title, |meta| &meta.title)) != "unknown"
            || track.catalog_metadata.as_ref().map_or(&track.title, |meta| &meta.title).trim().is_empty()
            || track.catalog_metadata.as_ref().map_or(&track.artist, |meta| &meta.artist).trim().is_empty()
            || track.catalog_metadata.as_ref().map_or(&track.title, |meta| &meta.title).eq_ignore_ascii_case("unknown title")
            || track.catalog_metadata.as_ref().map_or(&track.artist, |meta| &meta.artist).eq_ignore_ascii_case("unknown artist")
            || track.catalog_metadata.as_ref().map_or(&track.title, |meta| &meta.title).chars().count() > 512
            || track.catalog_metadata.as_ref().map_or(&track.artist, |meta| &meta.artist).chars().count() > 512
            || track.catalog_metadata.as_ref().map_or(&track.album, |meta| &meta.album).chars().count() > 512
        {
            continue;
        }
        unique.entry(track.path.clone()).or_insert(track);
    }

    // A music video is an extra link on its audio recording. If that video is
    // also present in the library as a standalone YouTube row, send it only
    // through clipVideoId on the main song, never as a second catalog track.
    let attached_video_paths = unique
        .values()
        .filter_map(|track| {
            let song_id = track.path.strip_prefix("yt://")?;
            let clip_id = track.clip_video_id.as_deref()?;
            (clip_id != song_id && crate::services::video_id(clip_id))
                .then(|| format!("yt://{clip_id}"))
        })
        .collect::<HashSet<_>>();
    unique.retain(|path, _| !attached_video_paths.contains(path));

    unique
        .into_iter()
        .filter_map(|(id, track)| {
            let metadata = track.catalog_metadata.as_ref();
            let title = metadata.map_or_else(|| track.title.clone(), |meta| meta.title.clone());
            let artist = metadata.map_or_else(|| track.artist.clone(), |meta| meta.artist.clone());
            let album = metadata.map_or_else(|| track.album.clone(), |meta| meta.album.clone());
            let duration_secs = metadata.map_or(track.duration_secs, |meta| meta.duration_secs);
            let thumbnail_url = metadata.map_or_else(|| track.thumbnail_url.clone(), |meta| {
                if meta.thumbnail_url.trim().is_empty() { track.thumbnail_url.clone() } else { meta.thumbnail_url.clone() }
            });
            let local_source_key = metadata.and_then(|meta| meta.local_source_key.clone())
                .filter(|key| key.strip_prefix("local://").is_some_and(|hash| hash.len() == 64
                    && hash.bytes().all(|byte| byte.is_ascii_hexdigit())))
                .or_else(|| {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        local_by_identity.get(&id).into_iter().flatten().find_map(|path| {
                            if crate::services::key_is_remote(path) { return None; }
                            tracks.iter().find(|candidate| crate::services::normalize_path(&candidate.path)
                                == crate::services::normalize_path(path))
                                .and_then(local_catalog_track)
                                .map(|local| local.path)
                        })
                    }
                    #[cfg(target_arch = "wasm32")]
                    { None }
                });
            let mut alternatives = Vec::<crate::lyrics::LyricsAlternative>::new();
            for path in std::iter::once(track.path.as_str()).chain(
                local_by_identity.get(&id).into_iter().flatten().copied()
            ) {
                for mut alternative in store.cached_lyrics(path).unwrap_or_default() {
                    if !alternative.lyrics.lines.iter().any(|line| !line.text.trim().is_empty()) {
                        continue;
                    }
                    use crate::lyrics::{has_any_timing, has_part_level_timing, has_syllable_level_timing, LyricsSyncType};
                    alternative.sync_type = if has_syllable_level_timing(&alternative.lyrics) {
                        LyricsSyncType::RichSync
                    } else if has_part_level_timing(&alternative.lyrics) {
                        LyricsSyncType::Word
                    } else if has_any_timing(&alternative.lyrics) {
                        LyricsSyncType::Synced
                    } else {
                        LyricsSyncType::Plain
                    };
                    if let Some(prior) = alternatives.iter_mut().find(|prior| prior.sync_type == alternative.sync_type) {
                        if crate::braccato::providers::provider_rank(&alternative.provider)
                            < crate::braccato::providers::provider_rank(&prior.provider)
                        {
                            *prior = alternative;
                        }
                    } else {
                        alternatives.push(alternative);
                    }
                }
            }
            let selected = alternatives.iter().max_by_key(|item| {
                        let has_text = item.lyrics.lines.iter().any(|line| !line.text.trim().is_empty());
                        let quality = match item.sync_type {
                            crate::lyrics::LyricsSyncType::RichSync => 4,
                            crate::lyrics::LyricsSyncType::Word => 3,
                            crate::lyrics::LyricsSyncType::Synced => 2,
                            crate::lyrics::LyricsSyncType::Plain => 1,
                        };
                        (has_text, std::cmp::Reverse(crate::braccato::providers::provider_rank(&item.provider)), quality, item.lyrics.lines.len())
                    })
                .filter(|item| item.lyrics.lines.iter().any(|line| !line.text.trim().is_empty()));
            let lyrics_tracks = selected.map_or_else(Vec::new, |item| catalog_lyrics_tracks(&item.lyrics));
            let lyrics_text = selected.map_or_else(String::new, |item| {
                if item.sync_type == crate::lyrics::LyricsSyncType::Plain {
                    item.lyrics.lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n")
                } else {
                    let mut source = item.lyrics.clone();
                    for line in &mut source.lines {
                        line.translation.clear();
                        line.translations.clear();
                    }
                    crate::lyrics::serialize_lrc(&source)
                }
            });
            let youtube_music_url = id.strip_prefix("yt://")
                .map(|video| format!("https://music.youtube.com/watch?v={video}"))
                .unwrap_or_default();
            let item = CatalogImportTrack {
                path: id.clone(),
                source_url: youtube_music_url.clone(),
                youtube_music_url,
                local_source_key,
                recording_type: recording_type(&title).into(),
                clip_video_id: track.clip_video_id.clone().filter(|id| crate::services::video_id(id)),
                title,
                artist,
                album,
                duration_secs,
                thumbnail_url,
                lyrics_text: if lyrics_text.len() <= 120_000 { lyrics_text } else { String::new() },
                lyrics_tracks: lyrics_tracks
                    .into_iter()
                    .filter(|track| track.text.len() <= 120_000)
                    .collect(),
                lyrics_variants: alternatives,
            };
            let serialized = serde_json::to_vec(&item).ok()?;
            let mut hash = Sha1::new();
            hash.update(b"catalog-membership-v1\0");
            hash.update(serialized);
            let fingerprint = format!("{:x}", hash.finalize());
            (uploaded.get(&id) != Some(&fingerprint)).then_some((id, fingerprint, item))
        })
        .take(20)
        .collect()
}
// Match the deployed Worker's settings allowlist. Local extensions must not
// make otherwise valid playlist sync fail with "Unsupported setting".
fn serialize_cloud_settings<S: serde::Serializer>(
    settings: &Settings,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut value = serde_json::to_value(settings).map_err(serde::ser::Error::custom)?;
    if let Some(object) = value.as_object_mut() {
        object.retain(|key, _| {
            matches!(
                key.as_str(),
                "language"
                    | "color_mode"
                    | "art_mode"
                    | "download_badge_style"
                    | "download_badge_position"
                    | "lyrics_provider"
                    | "lyrics_translation_language"
                    | "repeat"
                    | "download_badge_shown"
                    | "lyrics_romanization"
                    | "lyrics_translation"
                    | "lyrics_stage"
                    | "lyrics_reduced_motion"
                    | "lyrics_passive_scroll"
                    | "shuffle"
                    | "volume"
                    | "color_hue"
                    | "kawarp"
            )
        });
    }
    value.serialize(serializer)
}
fn cloud_key(key: &str) -> bool {
    key.strip_prefix("yt://").is_some_and(|id| {
        id.len() == 11
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }) || key
        .strip_prefix("cloud://")
        .is_some_and(|id| id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))
}
fn portable_track(track: &Track) -> Track {
    let mut result = track.clone();
    let public_art = reqwest::Url::parse(&result.thumbnail_url)
        .ok()
        .is_some_and(|url| {
            url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
        });
    if !public_art {
        result.thumbnail_url.clear();
    }
    result
}

/// Local retry identity based only on song metadata, never the local path.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn local_catalog_track(track: &Track) -> Option<Track> {
    use sha2::{Digest, Sha256};
    if crate::services::key_is_remote(&track.path) { return None; }
    let title = crate::services::normalize_music_text(&track.title);
    let artist = crate::services::normalize_music_text(&track.artist);
    if title.is_empty() || artist.is_empty() || title == "unknown title" || artist == "unknown artist" { return None; }
    let album = crate::services::normalize_music_text(&track.album);
    let duration = if track.duration_secs.is_finite() { track.duration_secs.max(0.).round() as u64 } else { 0 };
    let identity = format!("{title}\n{artist}\n{album}\n{duration}");
    let mut result = portable_track(track);
    result.path = format!("local://{:x}", Sha256::digest(identity.as_bytes()));
    Some(result)
}

/// Resolve a batch of account keys without repeatedly scanning the library.
struct CloudIdentities {
    tracks: BTreeMap<String, Option<String>>,
    downloads: BTreeMap<String, String>,
}

impl CloudIdentities {
    fn new(store: &Store) -> Self {
        let mut tracks = BTreeMap::new();
        for track in &store.tracks {
            tracks
                .entry(crate::services::normalize_path(&track.path))
                .or_insert_with(|| {
                    track.video_id.as_deref()
                        .filter(|id| crate::services::video_id(id))
                        .map(|id| format!("yt://{id}"))
                });
        }
        let mut downloads = BTreeMap::new();
        for (remote, path) in &store.downloads {
            if cloud_key(remote) {
                downloads
                    .entry(crate::services::normalize_path(path))
                    .or_insert_with(|| remote.clone());
            }
        }
        Self { tracks, downloads }
    }

    fn identity(&self, key: &str) -> Option<String> {
        if cloud_key(key) {
            return Some(key.to_owned());
        }
        if let Some(id) = key
            .strip_prefix("vid:")
            .filter(|id| crate::services::video_id(id))
        {
            return Some(format!("yt://{id}"));
        }
        let local = crate::services::normalize_path(key);
        self.tracks
            .get(&local)
            .and_then(Clone::clone)
            .or_else(|| self.downloads.get(&local).cloned())
    }

    fn track(&self, track: &Track) -> Option<Track> {
        let key = track
            .video_id
            .as_deref()
            .filter(|id| crate::services::video_id(id))
            .map(|id| format!("yt://{id}"))
            .or_else(|| self.identity(&track.path))?;
        let mut result = portable_track(track);
        result.path = key;
        Some(result)
    }
}
/// The portable key for anything the account may store: cloud keys pass
/// through, and a local file maps to `yt://<id>` when its tags, remembered
/// track or download link expose a YouTube Music identity.
fn cloud_identity(store: &Store, key: &str) -> Option<String> {
    if cloud_key(key) {
        return Some(key.to_string());
    }
    if let Some(id) = key
        .strip_prefix("vid:")
        .filter(|id| crate::services::video_id(id))
    {
        return Some(format!("yt://{id}"));
    }
    let local = crate::services::normalize_path(key);
    if let Some(id) = store
        .tracks
        .iter()
        .find(|track| crate::services::normalize_path(&track.path) == local)
        .and_then(|track| track.video_id.as_deref())
        .filter(|id| crate::services::video_id(id))
    {
        return Some(format!("yt://{id}"));
    }
    store
        .downloads
        .iter()
        .find(|(remote, path)| cloud_key(remote) && crate::services::normalize_path(path) == local)
        .map(|(remote, _)| remote.clone())
}
impl CloudLibrary {
    fn capture(store: &Store, auto_upload: bool) -> Self {
        let identities = CloudIdentities::new(store);
        let eligible: HashSet<String> = catalog_sources(store).iter()
            .filter_map(|track| identities.track(track).map(|track| track.path))
            .collect();
        let mut seen = BTreeSet::new();
        let tracks: Vec<Track> = store
            .tracks
            .iter()
            .filter_map(|track| identities.track(track))
            .filter(|track| !track.path.starts_with("yt://") || eligible.contains(&track.path))
            .filter(|track| seen.insert(track.path.clone()))
            .collect();
        let catalog_library_keys = store.library_tracks().into_iter()
            .filter(|track| !crate::services::key_is_remote(&track.path))
            .filter_map(|track| identities.track(&track).map(|track| track.path))
            .filter(|key| key.starts_with("yt://"))
            .take(20000)
            .collect();
        Self {
            tracks,
            catalog_library_keys,
            likes: store
                .likes
                .iter()
                .filter_map(|key| identities.identity(key))
                .collect(),
            playlists: store
                .playlists
                .iter()
                .map(|p| {
                    Playlist {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        tracks: p
                            .tracks
                            .iter()
                            .filter_map(|key| identities.identity(key))
                            .collect(),
                        ..p.clone()
                    }
                })
                .collect(),
            history: store
                .history
                .iter()
                .filter_map(|h| {
                    Some(History {
                        track: identities.track(&h.track)?,
                        ..h.clone()
                    })
                })
                .take(5000)
                .collect(),
            recommendations: RecommendationModel {
                schema_version: store.recommendations.schema_version.max(1),
                updated_at: store.recommendations.updated_at,
                profile_updated_at: store.recommendations.profile_updated_at,
                personalized: store.recommendations.personalized,
                track_weights: store
                    .recommendations
                    .track_weights
                    .iter()
                    .filter(|(key, _)| key.starts_with("yt://") && cloud_key(key))
                    .take(1000)
                    .map(|(key, value)| (key.clone(), *value))
                    .collect(),
                artist_weights: store
                    .recommendations
                    .artist_weights
                    .iter()
                    .filter(|(key, _)| !key.trim().is_empty() && key.len() <= 120)
                    .take(500)
                    .map(|(key, value)| (key.clone(), *value))
                    .collect(),
                disliked_tracks: store
                    .recommendations
                    .disliked_tracks
                    .iter()
                    .filter(|key| key.starts_with("yt://") && cloud_key(key))
                    .take(1000)
                    .cloned()
                    .collect(),
                tracks: store
                    .recommendations
                    .tracks
                    .iter()
                    .filter(|t| t.path.starts_with("yt://") && cloud_key(&t.path))
                    .take(12)
                    .map(portable_track)
                    .collect(),
            },
            settings: store.settings.clone(),
            offsets: store
                .offsets
                .iter()
                .filter_map(|(key, value)| identities.identity(key).map(|key| (key, *value)))
                .collect(),
            lyrics_choices: store
                .lyrics_choices
                .iter()
                .filter_map(|(key, value)| {
                    identities.identity(key).map(|key| (key, value.clone()))
                })
                .collect(),
            auto_upload,
        }
    }
    fn restore(self, store: &mut Store) {
        let CloudLibrary {
            tracks,
            likes,
            playlists,
            history,
            catalog_library_keys: _,
            settings,
            offsets,
            lyrics_choices,
            recommendations,
            auto_upload: _,
        } = self;
        store.recommendations.merge_newer(recommendations);
        store.tracks.retain(|t| !cloud_key(&t.path));
        store
            .tracks
            .extend(tracks.into_iter().filter(|t| cloud_key(&t.path)));
        // A local like and its uploaded `yt://` twin are one song. Keep the
        // portable key so a later capture can still send it from this device.
        let duplicates: HashSet<String> = store
            .likes
            .iter()
            .filter(|key| !cloud_key(key))
            .filter(|key| cloud_identity(store, key).is_some_and(|id| likes.contains(&id)))
            .cloned()
            .collect();
        store
            .likes
            .retain(|key| !cloud_key(key) && !duplicates.contains(key));
        store
            .likes
            .extend(likes.into_iter().filter(|key| cloud_key(key)));
        for remote in playlists {
            let remote_tracks: HashSet<String> = remote.tracks.iter().cloned().collect();
            let Some(index) = store.playlists.iter().position(|p| p.id == remote.id) else {
                store.playlists.push(remote);
                continue;
            };
            let local_only: Vec<String> = store.playlists[index]
                .tracks
                .iter()
                .filter(|key| !cloud_key(key))
                .filter(|key| {
                    !cloud_identity(store, key).is_some_and(|id| remote_tracks.contains(&id))
                })
                .cloned()
                .collect();
            let mut merged = remote;
            for key in local_only {
                if !merged.tracks.contains(&key) {
                    merged.tracks.push(key);
                }
            }
            store.playlists[index] = merged;
        }
        store.history.retain(|h| !cloud_key(&h.track.path));
        let local_history: HashSet<(Option<String>, u64)> = store
            .history
            .iter()
            .map(|h| (cloud_identity(store, &h.track.path), h.played_at))
            .collect();
        store.history.extend(
            history
                .into_iter()
                .filter(|h| !local_history.contains(&(Some(h.track.path.clone()), h.played_at))),
        );
        store.history.sort_by(|a, b| b.played_at.cmp(&a.played_at));
        let duplicates: HashSet<String> = store
            .offsets
            .keys()
            .filter(|key| !cloud_key(key))
            .filter(|key| cloud_identity(store, key).is_some_and(|id| offsets.contains_key(&id)))
            .cloned()
            .collect();
        store
            .offsets
            .retain(|key, _| !cloud_key(key) && !duplicates.contains(key));
        store.offsets.extend(offsets);
        let duplicates: HashSet<String> = store
            .lyrics_choices
            .keys()
            .filter(|key| !cloud_key(key))
            .filter(|key| {
                cloud_identity(store, key).is_some_and(|id| lyrics_choices.contains_key(&id))
            })
            .cloned()
            .collect();
        store
            .lyrics_choices
            .retain(|key, _| !cloud_key(key) && !duplicates.contains(key));
        store.lyrics_choices.extend(lyrics_choices);
        let local_settings = store.settings.clone();
        store.settings = settings;
        store.settings.home_layout = local_settings.home_layout;
        store.settings.home_rotate = local_settings.home_rotate;
        store.settings.lyrics_theme_css = local_settings.lyrics_theme_css;
        store.settings.lyrics_font_family = local_settings.lyrics_font_family;
        store.settings.theme_entire_player = local_settings.theme_entire_player;
        store.settings.navigation_blur = local_settings.navigation_blur;
        store.settings.lyrics_secondary_gap = local_settings.lyrics_secondary_gap;
    }
}
#[derive(Clone, Default, Deserialize)]
struct FileInfo {
    id: String,
    name: String,
    size: u64,
}
#[derive(Clone)]
enum Action {
    PreparePush(Store, bool),
    PrepareInitialSnapshot(Store, bool),
    #[cfg(not(target_arch = "wasm32"))]
    PrepareRestore {
        store: Store,
        cloud: CloudLibrary,
        cloud_revision: u64,
        store_revision: u64,
        last_snapshot: String,
        force: bool,
    },
    Prepare {
        store: Store,
        last_snapshot: String,
        verified: bool,
        check_due: bool,
        #[cfg(not(target_arch = "wasm32"))]
        match_attempts: BTreeMap<String, u64>,
        #[cfg(not(target_arch = "wasm32"))]
        match_busy: bool,
    },
    #[cfg(not(target_arch = "wasm32"))]
    MatchLocal(Track),
    Login {
        email: String,
        password: String,
        name: String,
        register: bool,
    },
    Push(CloudLibrary),
    Pull,
    Account,
    SaveProfile { email: String, name: String, nickname: String, bio: String, discord: String, current_password: String },
    ChangePassword { current_password: String, new_password: String },
    #[cfg(not(target_arch = "wasm32"))]
    UploadAvatar(String),
    #[cfg(target_arch = "wasm32")]
    UploadAvatar(Vec<u8>),
    DeleteAvatar,
    Files,
    Device,
    Poll(String),
    Logout,
    Upload(String),
    Auto(Vec<String>),
    CatalogImport(Vec<(String, String, CatalogImportTrack)>),
    Check,
    Download(FileInfo),
}
enum Output {
    PreparedPush(CloudLibrary),
    PreparedInitialSnapshot(String, bool),
    #[cfg(not(target_arch = "wasm32"))]
    PreparedRestore {
        store_revision: u64,
        cloud_revision: u64,
        prior_revision: Option<u64>,
        auto_sync: bool,
        prior_auto_upload: bool,
        auto_upload: bool,
        restored: Option<(Store, String)>,
    },
    Prepared(Option<Box<Action>>, bool, bool, Option<u64>),
    Unchanged,
    Session(Session),
    Pushed(u64, String),
    CatalogQueued(Vec<(String, String)>, usize, Option<String>),
    #[cfg(not(target_arch = "wasm32"))]
    LocalMatched(Track, Option<Track>, u64),
    Pulled(u64, CloudLibrary, bool),
    Account(Value, Option<image::RgbaImage>),
    Profile(Value),
    Avatar(Value, Option<image::RgbaImage>),
    Password,
    Files(Vec<FileInfo>),
    Device(Value),
    Pending(u64),
    Logout,
    Uploaded(FileInfo, String, String),
    Downloaded(FileInfo, String),
}
fn base(value: &str) -> Result<String, String> {
    let u = reqwest::Url::parse(value.trim()).map_err(|_| "Укажіть адресу API Vey".to_string())?;
    let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (u.scheme() != "https" && !(local && u.scheme() == "http"))
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("Потрібна HTTPS-адреса API без паролів і параметрів".into());
    }
    Ok(u.as_str().trim_end_matches('/').to_string())
}
pub(crate) fn recommendation_catalog_base() -> Option<String> {
    let session = load_session();
    let api = if session.api.trim().is_empty() {
        "https://blueberry-melody.superstilist.workers.dev"
    } else {
        session.api.trim()
    };
    base(api).ok()
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/web/account.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=accountRequest)]
    fn browser_request(url: &str, method: &str, token: &str, body: &str) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=accountBinaryRequest)]
    fn browser_binary_request(url: &str, method: &str, token: &str, content_type: &str, bytes: &js_sys::Uint8Array) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=accountAvatarRequest)]
    fn browser_avatar_request(url: &str, token: &str) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=pickAccountAvatar)]
    fn browser_pick_avatar() -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=uploadAccountFile)]
    fn browser_upload(api: &str, token: &str, path: &str) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=downloadAccountFile)]
    fn browser_download(api: &str, token: &str, id: &str) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=cancelAccountUpload)]
    fn browser_cancel();
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=openAccountLogin)]
    fn browser_open_login(url: &str);
}
async fn request(
    s: &Session,
    path: &str,
    method: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let url = format!("{}/vey/{}", base(&s.api)?, path);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Не вдалося створити запит".to_string())?;
        let mut req = client.request(
            method.parse().map_err(|_| "Невідомий запит".to_string())?,
            url,
        );
        if !s.token.is_empty() {
            req = req.bearer_auth(&s.token);
        }
        if let Some(data) = body {
            req = req.json(&data);
        }
        let mut response = req
            .send()
            .await
            .map_err(|e| format!("Помилка мережі: {}", e.without_url()))?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Відповідь API перервана".to_string())?
        {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err("Відповідь API завелика".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let data: Value = serde_json::from_slice(&bytes)
            .map_err(|_| format!("API повернув помилкову відповідь ({status})"))?;
        if status == 401 && !s.token.is_empty() {
            return Err("Сесія завершилася. Увійдіть знову.".into());
        }
        if status == 400
            && path == "oauth/token"
            && matches!(
                data["error"].as_str(),
                Some("authorization_pending" | "slow_down")
            )
        {
            return Ok(data);
        }
        if status >= 300 {
            if let Some(description) = data["error_description"].as_str() {
                return Err(description.into());
            }
            return Err(data["error"]["message"]
                .as_str()
                .unwrap_or("Запит відхилено")
                .to_string());
        }
        Ok(data)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let data = wasm_bindgen_futures::JsFuture::from(browser_request(
            &url,
            method,
            &s.token,
            &body.map(|b| b.to_string()).unwrap_or_default(),
        ))
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| "Помилка мережі".into()))?;
        serde_json::from_str(&data.as_string().unwrap_or_default())
            .map_err(|_| "Некоректна відповідь API".into())
    }
}
async fn request_binary_json(
    s: &Session,
    path: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<Value, String> {
    if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
        return Err("Аватар має бути розміром до 2 MiB".into());
    }
    let url = format!("{}/vey/{}", base(&s.api)?, path);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Не вдалося створити запит".to_string())?;
        let mut req = client.put(url).header(reqwest::header::CONTENT_TYPE, content_type);
        if !s.token.is_empty() { req = req.bearer_auth(&s.token); }
        let response = req.body(bytes.to_vec()).send().await
            .map_err(|e| format!("Помилка мережі: {}", e.without_url()))?;
        let status = response.status().as_u16();
        let body = response.bytes().await.map_err(|_| "Відповідь API перервана".to_string())?;
        if body.len() > 2 * 1024 * 1024 { return Err("Відповідь API завелика".into()); }
        let data: Value = serde_json::from_slice(&body).map_err(|_| format!("API повернув помилкову відповідь ({status})"))?;
        if status == 401 { return Err("Сесія завершилася. Увійдіть знову.".into()); }
        if status >= 300 { return Err(data["error"]["message"].as_str().unwrap_or("Запит відхилено").to_string()); }
        Ok(data)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let payload = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
        payload.copy_from(bytes);
        let data = wasm_bindgen_futures::JsFuture::from(browser_binary_request(&url, "PUT", &s.token, content_type, &payload))
            .await.map_err(|e| e.as_string().unwrap_or_else(|| "Помилка мережі".into()))?;
        serde_json::from_str(&data.as_string().unwrap_or_default()).map_err(|_| "Некоректна відповідь API".into())
    }
}
async fn request_avatar(s: &Session) -> Result<Vec<u8>, String> {
    let url = format!("{}/vey/account/avatar", base(&s.api)?);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Не вдалося створити запит".to_string())?;
        let mut req = client.get(url);
        if !s.token.is_empty() { req = req.bearer_auth(&s.token); }
        let mut response = req.send().await.map_err(|e| format!("Помилка мережі: {}", e.without_url()))?;
        let status = response.status().as_u16();
        if status == 404 { return Ok(Vec::new()); }
        if status == 401 { return Err("Сесія завершилася. Увійдіть знову.".into()); }
        if status >= 300 { return Err("Не вдалося завантажити аватар".into()); }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "Відповідь API перервана".to_string())? {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 { return Err("Аватар завеликий".into()); }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let data = wasm_bindgen_futures::JsFuture::from(browser_avatar_request(&url, &s.token))
            .await.map_err(|e| e.as_string().unwrap_or_else(|| "Помилка мережі".into()))?;
        let bytes = js_sys::Uint8Array::new(&data);
        if bytes.length() > 2 * 1024 * 1024 { return Err("Аватар завеликий".into()); }
        Ok(bytes.to_vec())
    }
}
fn decode_avatar(bytes: &[u8]) -> Option<image::RgbaImage> {
    if bytes.is_empty() { None } else { crate::artwork::decode(bytes).ok() }
}
async fn load_account(s: &Session) -> Result<Output, String> {
    let profile = request(s, "account", "GET", None).await?;
    let avatar = if profile["avatar_url"].as_str().is_some_and(|value| !value.is_empty()) {
        decode_avatar(&request_avatar(s).await?)
    } else { None };
    Ok(Output::Account(profile, avatar))
}
async fn upload_avatar(s: &Session, bytes: Vec<u8>) -> Result<Output, String> {
    let format = image::guess_format(&bytes).map_err(|_| "Оберіть JPEG, PNG або WebP зображення".to_string())?;
    let content_type = match format {
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err("Оберіть JPEG, PNG або WebP зображення".into()),
    };
    if crate::artwork::decode(&bytes).is_err() { return Err("Не вдалося прочитати це зображення".into()); }
    let profile = request_binary_json(s, "account/avatar", content_type, &bytes).await?;
    let avatar = decode_avatar(&request_avatar(s).await?);
    Ok(Output::Avatar(profile, avatar))
}
fn login_result(mut s: Session, value: Value) -> Result<Session, String> {
    s.token = value
        .get("access_token")
        .or_else(|| value.get("token"))
        .and_then(Value::as_str)
        .filter(|v| v.starts_with("vey_"))
        .ok_or("Відсутня сесія")?
        .into();
    s.id = value["user"]["id"]
        .as_str()
        .ok_or("Відсутній акаунт")?
        .into();
    s.email = value["user"]["email"].as_str().unwrap_or_default().into();
    s.name = value["user"]["display_name"]
        .as_str()
        .unwrap_or_default()
        .into();
    s.expires_at = value["expires_at"]
        .as_u64()
        .ok_or("Відсутній термін сесії")?;
    s.premium = value["premium"].as_bool().unwrap_or(false);
    s.revision = None;
    s.uploaded.clear();
    Ok(s)
}
#[cfg(not(target_arch = "wasm32"))]
fn select_local_youtube_match(local: &Track, candidates: Vec<Track>) -> Option<Track> {
    if recording_type(&local.title) != "unknown" || crate::services::track_video_id(local).is_some() {
        return None;
    }
    let mut matches = BTreeMap::new();
    for candidate in candidates {
        let Some(id) = crate::services::track_video_id(&candidate) else { continue };
        if recording_type(&candidate.title) == "unknown"
            && crate::services::same_track_identity(local, &candidate)
        {
            matches.entry(id.to_string()).or_insert(candidate);
        }
    }
    (matches.len() == 1).then(|| matches.into_values().next()).flatten()
}
async fn work(s: Session, action: Action) -> Result<Output, String> {
    match action {
        Action::PreparePush(store, auto_upload) => Ok(Output::PreparedPush(CloudLibrary::capture(&store, auto_upload))),
        Action::PrepareInitialSnapshot(store, auto_upload) => Ok(Output::PreparedInitialSnapshot(
            serde_json::to_string(&CloudLibrary::capture(&store, auto_upload)).unwrap_or_default(), auto_upload,
        )),
        #[cfg(not(target_arch = "wasm32"))]
        Action::PrepareRestore { mut store, cloud, cloud_revision, store_revision, last_snapshot, force } => {
            let auto_upload = cloud.auto_upload;
            let conflict = !force && last_snapshot != serde_json::to_string(&CloudLibrary::capture(&store, s.auto_upload)).unwrap_or_default();
            let restored = if conflict { None } else {
                cloud.restore(&mut store);
                store.migrate_identities();
                let snapshot = if cloud_revision == 0 && s.auto_sync { String::new() }
                    else { serde_json::to_string(&CloudLibrary::capture(&store, auto_upload)).unwrap_or_default() };
                Some((store, snapshot))
            };
            Ok(Output::PreparedRestore { store_revision, cloud_revision, prior_revision: s.revision,
                auto_sync: s.auto_sync, prior_auto_upload: s.auto_upload, auto_upload, restored })
        }
        Action::Prepare { store, last_snapshot, verified, check_due, #[cfg(not(target_arch = "wasm32"))] match_attempts, #[cfg(not(target_arch = "wasm32"))] match_busy } => {
            let action = if !verified {
                Some(Action::Account)
            } else if s.revision.is_none() && s.auto_sync {
                check_due.then_some(Action::Check)
            } else {
                let changed = if s.auto_sync {
                    let current = CloudLibrary::capture(&store, s.auto_upload);
                    let text = serde_json::to_string(&current).unwrap_or_default();
                    (last_snapshot != text).then_some(Action::Push(current))
                } else { None };
                if changed.is_some() { changed }
                else if check_due {
                    let pending = if s.auto_sync { catalog_candidates(&store, &s.catalog_uploaded) } else { Vec::new() };
                    if !pending.is_empty() { Some(Action::CatalogImport(pending)) }
                    else {
                        let fallback = || if s.auto_upload && s.premium {
                            Some(Action::Auto(store.tracks.iter().filter(|t| !cloud_key(&t.path)).map(|t| t.path.clone()).collect()))
                        } else if s.auto_sync { Some(Action::Check) } else { None };
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let identities = CloudIdentities::new(&store);
                            let now = crate::services::now_ms();
                            let local = (s.auto_sync && !match_busy).then(|| catalog_sources(&store).into_iter().filter_map(|track| {
                                if crate::services::key_is_remote(&track.path)
                                    || identities.identity(&track.path).is_some()
                                    || !track.duration_secs.is_finite() || track.duration_secs <= 0.0
                                    || recording_type(&track.title) != "unknown" { return None; }
                                let key = local_catalog_track(&track)?;
                                let retry_at = match_attempts.get(&key.path).copied().unwrap_or(0);
                                (retry_at <= now).then_some((retry_at, track))
                            }).min_by_key(|(retry_at, _)| *retry_at).map(|(_, track)| track.clone())).flatten();
                            local.map(Action::MatchLocal).or_else(fallback)
                        }
                        #[cfg(target_arch = "wasm32")]
                        { fallback() }
                    }
                } else { None }
            };
            Ok(Output::Prepared(action.map(Box::new), s.auto_sync, s.auto_upload, s.revision))
        }
        #[cfg(not(target_arch = "wasm32"))]
        Action::MatchLocal(local) => {
            let query = format!("{} {}", local.artist, local.title);
            let results = tokio::time::timeout(Duration::from_secs(20), crate::native::ytm_match_candidates(query, Some(24))).await;
            let (candidates, mut retry_ms) = match results {
                Ok(Ok(songs)) => (songs.into_iter().filter(|song| song.kind == "song" && !song.has_video)
                    .map(|song| Track { path: format!("yt://{}", song.video_id), video_id: Some(song.video_id),
                        title: song.title, artist: song.artist, album: song.album, duration_secs: song.duration_secs as f64,
                        thumbnail_url: song.thumbnail_url, clip_video_id: (!song.clip_video_id.is_empty()).then_some(song.clip_video_id),
                        catalog_metadata: None })
                    .collect(), 6 * 60 * 60 * 1000),
                _ => (Vec::new(), 5 * 60 * 1000),
            };
            let mut matched = select_local_youtube_match(&local, candidates);
            if let Some(candidate) = &matched {
                let id = candidate.video_id.clone().unwrap_or_default();
                let playable = tokio::time::timeout(Duration::from_secs(12), crate::native::ytm_stream_url(id)).await;
                if !matches!(playable, Ok(Ok(Some(ref stream))) if stream.duration_secs == 0
                    || (stream.duration_secs as f64 - local.duration_secs).abs() <= 5.0) {
                    matched = None;
                    retry_ms = 5 * 60 * 1000;
                }
            }
            Ok(Output::LocalMatched(local, matched, retry_ms))
        }
        Action::Login {
            email,
            password,
            name,
            register,
        } => {
            let value = request(
                &Session {
                    token: String::new(),
                    ..s.clone()
                },
                if register {
                    "auth/register"
                } else {
                    "auth/login"
                },
                "POST",
                Some(json!({"email":email,"password":password,"display_name":name})),
            )
            .await?;
            Ok(Output::Session(login_result(s, value)?))
        }
        Action::Push(data) => {
            let revision = match s.revision {
                Some(r) => r,
                None => request(&s, "library", "GET", None).await?["revision"]
                    .as_u64()
                    .ok_or("Немає версії бібліотеки")?,
            };
            let sent =
                serde_json::to_string(&data).map_err(|_| "Помилка бібліотеки".to_string())?;
            let sent_value =
                serde_json::to_value(&data).map_err(|_| "Помилка бібліотеки".to_string())?;
            let result = request(
                &s,
                "library",
                "PUT",
                Some(json!({"revision":revision,"library":data.clone()})),
            )
            .await;
            // Older Workers reject unknown library fields. Keep ordinary library
            // sync working until the Worker supports recommendation snapshots.
            let v = match result {
                Err(ref e) if e.contains("Unsupported library field") => {
                    let mut legacy = serde_json::to_value(&data).map_err(|_| "Помилка бібліотеки".to_string())?;
                    if let Some(object) = legacy.as_object_mut() {
                        object.remove("recommendations");
                        object.remove("catalogLibraryKeys");
                    }
                    request(&s, "library", "PUT", Some(json!({"revision":revision,"library":legacy}))).await?
                }
                Err(error) => {
                    // The server can commit a PUT and lose its response. Read
                    // back before treating a retry's revision conflict as a
                    // real two-device edit.
                    match request(&s, "library", "GET", None).await {
                        Ok(remote) if remote["library"] == sent_value => remote,
                        _ => return Err(error),
                    }
                }
                Ok(value) => value,
            };
            Ok(Output::Pushed(
                v["revision"].as_u64().ok_or("Немає версії бібліотеки")?,
                sent,
            ))
        }
        Action::Pull => {
            let v = request(&s, "library", "GET", None).await?;
            let revision = v["revision"].as_u64().ok_or("Немає версії бібліотеки")?;
            if revision == 0 {
                return Err("Хмарна бібліотека порожня. Збережіть дані цього пристрою.".into());
            }
            let data = serde_json::from_value(v["library"].clone())
                .map_err(|_| "Непідтримувана бібліотека".to_string())?;
            Ok(Output::Pulled(revision, data, true))
        }
        Action::Account => load_account(&s).await,
        Action::SaveProfile { email, name, nickname, bio, discord, current_password } => {
            let value = request(&s, "account", "PATCH", Some(json!({
                "email": email,
                "display_name": name,
                "nickname": nickname,
                "bio": bio,
                "discord": discord,
                "current_password": current_password,
            }))).await?;
            Ok(Output::Profile(value))
        }
        Action::ChangePassword { current_password, new_password } => {
            request(&s, "account/password", "POST", Some(json!({
                "current_password": current_password,
                "new_password": new_password,
            }))).await?;
            Ok(Output::Password)
        }
        #[cfg(not(target_arch = "wasm32"))]
        Action::UploadAvatar(path) => {
            let metadata = std::fs::metadata(&path).map_err(|_| "Не вдалося відкрити зображення".to_string())?;
            if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 2 * 1024 * 1024 {
                return Err("Аватар має бути розміром до 2 MiB".into());
            }
            let bytes = std::fs::read(path).map_err(|_| "Не вдалося прочитати зображення".to_string())?;
            upload_avatar(&s, bytes).await
        }
        #[cfg(target_arch = "wasm32")]
        Action::UploadAvatar(bytes) => upload_avatar(&s, bytes).await,
        Action::DeleteAvatar => {
            request(&s, "account/avatar", "DELETE", None).await?;
            load_account(&s).await
        }
        Action::Files => {
            let v = request(&s, "files", "GET", None).await?;
            Ok(Output::Files(
                serde_json::from_value(v["items"].clone())
                    .map_err(|_| "Непідтримуваний список файлів".to_string())?,
            ))
        }
        Action::Device => Ok(Output::Device(
            request(
                &Session {
                    token: String::new(),
                    ..s
                },
                "oauth/device_authorization",
                "POST",
                Some(json!({"client_id":"vey","scope":"vey"})),
            )
            .await?,
        )),
        Action::Poll(code) => {
            let v = request(
                &Session {
                    token: String::new(),
                    ..s.clone()
                },
                "oauth/token",
                "POST",
                Some(json!({"client_id":"vey","grant_type":"urn:ietf:params:oauth:grant-type:device_code","device_code":code})),
            )
            .await?;
            if matches!(
                v["error"].as_str(),
                Some("authorization_pending" | "slow_down")
            ) {
                Ok(Output::Pending(if v["error"] == "slow_down" {
                    5000
                } else {
                    0
                }))
            } else {
                Ok(Output::Session(login_result(s, v)?))
            }
        }
        Action::Logout => {
            request(&s, "auth/logout", "POST", Some(json!({}))).await?;
            Ok(Output::Logout)
        }
        Action::Upload(path) => upload_file(&s, path).await,
        Action::Check => check_library(&s).await,
        Action::Auto(paths) => {
            for path in paths {
                let stamp = fingerprint(&path);
                if !stamp.is_empty() && !s.uploaded.contains(&stamp) {
                    return upload_file(&s, path).await;
                }
            }
            if s.auto_sync && s.revision.is_some() {
                check_library(&s).await
            } else {
                Ok(Output::Unchanged)
            }
        }
        Action::CatalogImport(items) => {
            let mut acknowledged = Vec::new();
            let mut accepted = 0usize;
            let mut failure = None;
            let mut requests = 0usize;
            let mut batch = Vec::<(String, String, CatalogImportTrack, bool)>::new();
            let mut bytes = 32usize;
            'tracks: for (id, fingerprint, item) in items {
                if requests >= 10 { break; }
                let parts = match split_catalog_track(&item) {
                    Ok(parts) => parts,
                    Err(error) => { failure = Some(error); break; }
                };
                let count = parts.len();
                for (part_index, part) in parts.into_iter().enumerate() {
                    let size = serde_json::to_vec(&part).map_err(|_| "Помилка каталогу".to_string())?.len() + 1;
                    if !batch.is_empty() && (bytes + size > 900_000 || batch.len() == 100 || part_index > 0) {
                        if let Err(error) = send_catalog_batch(&s, &batch).await { failure = Some(error); break 'tracks; }
                        requests += 1;
                        for (id, fingerprint, _, last) in batch.drain(..) {
                            if last { acknowledged.push((id, fingerprint)); accepted += 1; }
                        }
                        bytes = 32;
                    }
                    bytes += size;
                    batch.push((id.clone(), fingerprint.clone(), part, part_index + 1 == count));
                }
            }
            if failure.is_none() && !batch.is_empty() {
                match send_catalog_batch(&s, &batch).await {
                    Ok(()) => for (id, fingerprint, _, last) in batch {
                        if last { acknowledged.push((id, fingerprint)); accepted += 1; }
                    },
                    Err(error) => failure = Some(error),
                }
            }
            Ok(Output::CatalogQueued(acknowledged, accepted, failure))
        }
        Action::Download(file) => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                tokio::select! {
                    result=download_file(&s,file)=>result,
                    _=async {while !CANCEL_UPLOAD.load(std::sync::atomic::Ordering::Relaxed){tokio::time::sleep(Duration::from_millis(100)).await;}}=>Err("Завантаження скасовано".into())
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                download_file(&s, file).await
            }
        }
    }
}
async fn check_library(s: &Session) -> Result<Output, String> {
    let v = request(s, "library", "GET", None).await?;
    let revision = v["revision"].as_u64().ok_or("Відсутня версія бібліотеки")?;
    if Some(revision) == s.revision {
        return Ok(Output::Unchanged);
    }
    let cloud = serde_json::from_value(v["library"].clone())
        .map_err(|_| "Непідтримувана бібліотека".to_string())?;
    Ok(Output::Pulled(revision, cloud, false))
}
#[cfg(not(target_arch = "wasm32"))]
fn fingerprint(path: &str) -> String {
    use sha1::{Digest, Sha1};
    let Ok(meta) = std::fs::metadata(path) else {
        return String::new();
    };
    if !meta.is_file() {
        return String::new();
    }
    let modified = meta
        .modified()
        .ok()
        .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|v| v.as_millis())
        .unwrap_or(0);
    format!(
        "{:x}",
        Sha1::digest(format!("{path}:{}:{modified}", meta.len()).as_bytes())
    )
}
#[cfg(target_arch = "wasm32")]
fn fingerprint(path: &str) -> String {
    path.to_string()
}
static TRANSFER_DONE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static TRANSFER_TOTAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CANCEL_UPLOAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(crate) fn snapshot(store: &Store) -> Store {
    Store {
        settings: store.settings.clone(),
        tracks: store.tracks.clone(),
        likes: store.likes.clone(),
        playlists: store.playlists.clone(),
        history: store.history.clone(),
        recommendations: store.recommendations.clone(),
        offsets: store.offsets.clone(),
        downloads: store.downloads.clone(),
        library_excluded: store.library_excluded.clone(),
        lyrics_choices: store.lyrics_choices.clone(),
        lyrics_cache: store.lyrics_cache.clone(),
        ..Store::default()
    }
}
fn load_session() -> Session {
    #[cfg(not(target_arch = "wasm32"))]
    let text =
        std::fs::read_to_string(crate::storage::data_dir().join("account-session.json")).ok();
    #[cfg(target_arch = "wasm32")]
    let text = web_sys::window()
        .and_then(|w| w.session_storage().ok().flatten())
        .and_then(|s| s.get_item("vey.account.session").ok().flatten());
    text.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn save_session(s: &Session) -> Result<(), String> {
    let text = serde_json::to_string(s).map_err(|_| "Не вдалося зберегти сесію".to_string())?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Write;
        let dir = crate::storage::data_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("account-session.pending.json");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&path).map_err(|e| e.to_string())?;
        f.write_all(text.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(path, dir.join("account-session.json")).map_err(|e| e.to_string())?;
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        storage
            .set_item("vey.account.session", &text)
            .map_err(|_| "Не вдалося зберегти сесію".to_string())?;
    }
    Ok(())
}

// Account writes are ordered and coalesced on one worker; filesystem sync must
// never stall playback. Closing the app flushes the last queued state.
#[cfg(not(target_arch = "wasm32"))]
struct SessionWriter {
    sender: Option<std::sync::mpsc::Sender<Session>>,
    errors: std::sync::mpsc::Receiver<String>,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl SessionWriter {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<Session>();
        let (error_tx, errors) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            while let Ok(mut session) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() { session = newer; }
                if let Err(error) = save_session(&session) { let _ = error_tx.send(error); }
            }
        });
        Self { sender: Some(sender), errors, worker: Some(worker) }
    }
    fn request(&self, session: &Session) -> Result<(), String> {
        self.sender.as_ref().ok_or("Збереження сесії зупинено")?
            .send(session.clone()).map_err(|_| "Не вдалося зберегти сесію".into())
    }
    fn take_error(&self) -> Option<String> { self.errors.try_recv().ok() }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for SessionWriter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
#[cfg(target_arch = "wasm32")]
struct SessionWriter;
#[cfg(target_arch = "wasm32")]
impl SessionWriter {
    fn new() -> Self { Self }
    fn request(&self, session: &Session) -> Result<(), String> { save_session(session) }
    fn take_error(&self) -> Option<String> { None }
}

#[cfg(not(target_arch = "wasm32"))]
async fn upload_file(s: &Session, path: String) -> Result<Output, String> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(&path).map_err(|_| "Не вдалося відкрити аудіофайл".to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > 512 * 1024 * 1024 {
        return Err("Потрібен аудіофайл розміром до 512 MiB".into());
    }
    TRANSFER_DONE.store(0, std::sync::atomic::Ordering::Relaxed);
    TRANSFER_TOTAL.store(meta.len() as usize, std::sync::atomic::Ordering::Relaxed);
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("Некоректне ім’я файлу")?;
    let fingerprint = fingerprint(&path);
    if s.uploaded.contains(&fingerprint) {
        return Err("Цю версію файлу вже збережено у хмарі".into());
    }
    let start = request(
        s,
        "uploads",
        "POST",
        Some(json!({"name":name,"size":meta.len()})),
    )
    .await?;
    let id = start["id"]
        .as_str()
        .ok_or("Відсутній ID завантаження")?
        .to_string();
    let part_size = start["part_size"]
        .as_u64()
        .filter(|v| *v == 5 * 1024 * 1024)
        .ok_or("Непідтримуваний розмір частини")? as usize;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let result = async {
        let mut remaining = meta.len();
        let mut part = 1;
        while remaining > 0 {
            if CANCEL_UPLOAD.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("Завантаження скасовано".to_string());
            }
            let count = remaining.min(part_size as u64) as usize;
            let mut buffer = vec![0u8; count];
            file.read_exact(&mut buffer)
                .map_err(|_| "Читання аудіо перервано".to_string())?;
            let response = client
                .put(format!("{}/vey/uploads/{id}/parts/{part}", s.api))
                .bearer_auth(&s.token)
                .header("Content-Type", "application/octet-stream")
                .body(buffer)
                .send()
                .await
                .map_err(|e| e.without_url().to_string())?;
            if !response.status().is_success() {
                return Err(format!("Частину файлу відхилено ({})", response.status()));
            }
            remaining -= count as u64;
            TRANSFER_DONE.store(
                (meta.len() - remaining) as usize,
                std::sync::atomic::Ordering::Relaxed,
            );
            part += 1;
        }
        let value = request(
            s,
            &format!("uploads/{id}/complete"),
            "POST",
            Some(json!({})),
        )
        .await?;
        serde_json::from_value::<FileInfo>(value).map_err(|_| "Некоректні дані файлу".to_string())
    }
    .await;
    if result.is_err() {
        let _ = request(s, &format!("files/{id}"), "DELETE", None).await;
    }
    Ok(Output::Uploaded(result?, path, fingerprint))
}
#[cfg(target_arch = "wasm32")]
async fn upload_file(s: &Session, path: String) -> Result<Output, String> {
    let value = wasm_bindgen_futures::JsFuture::from(browser_upload(&s.api, &s.token, &path))
        .await
        .map_err(|e| {
            e.as_string()
                .unwrap_or_else(|| "Завантаження не вдалося".into())
        })?;
    let file: FileInfo = serde_json::from_str(&value.as_string().unwrap_or_default())
        .map_err(|_| "Некоректні дані файлу".to_string())?;
    Ok(Output::Uploaded(file, path.clone(), path))
}
#[cfg(not(target_arch = "wasm32"))]
struct PartialFile(std::path::PathBuf);
#[cfg(not(target_arch = "wasm32"))]
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
async fn download_file(s: &Session, file: FileInfo) -> Result<Output, String> {
    if file.id.len() != 36 || !file.id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return Err("Некоректний файл".into());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use sha1::{Digest, Sha1};
        use std::io::Write;
        let scope = format!(
            "{:x}",
            Sha1::digest(format!("{}:{}", s.api, s.id).as_bytes())
        );
        let dir = crate::storage::data_dir().join("cloud-audio").join(scope);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let ext = std::path::Path::new(&file.name)
            .extension()
            .and_then(|v| v.to_str())
            .filter(|s| s.len() <= 8 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
            .unwrap_or("audio");
        let path = dir.join(format!("{}.{}", file.id, ext));
        let pending = path.with_extension("pending");
        let _cleanup = PartialFile(pending.clone());
        TRANSFER_DONE.store(0, std::sync::atomic::Ordering::Relaxed);
        TRANSFER_TOTAL.store(file.size as usize, std::sync::atomic::Ordering::Relaxed);
        let result = async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?;
            let mut response = client
                .get(format!("{}/vey/files/{}", s.api, file.id))
                .bearer_auth(&s.token)
                .send()
                .await
                .map_err(|e| e.without_url().to_string())?
                .error_for_status()
                .map_err(|e| e.without_url().to_string())?;
            if file.size > 512 * 1024 * 1024 {
                return Err("Файл завеликий".to_string());
            }
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut output = options.open(&pending).map_err(|e| e.to_string())?;
            let mut received = 0;
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "Завантаження перервано".to_string())?
            {
                received += chunk.len() as u64;
                TRANSFER_DONE.store(received as usize, std::sync::atomic::Ordering::Relaxed);
                if received > file.size {
                    return Err("Невірний розмір файлу".into());
                }
                output.write_all(&chunk).map_err(|e| e.to_string())?;
            }
            if received != file.size {
                return Err("Неповний аудіофайл".into());
            }
            output.sync_all().map_err(|e| e.to_string())?;
            drop(output);
            std::fs::rename(&pending, &path).map_err(|e| e.to_string())?;
            Ok(path.to_string_lossy().to_string())
        }
        .await;
        if result.is_err() {
            let _ = std::fs::remove_file(&pending);
        }
        Ok(Output::Downloaded(file, result?))
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::JsFuture::from(browser_download(&s.api, &s.token, &file.id))
            .await
            .map_err(|e| {
                e.as_string()
                    .unwrap_or_else(|| "Завантаження не вдалося".into())
            })?;
        let key = format!("cloud://{}", file.id);
        Ok(Output::Downloaded(file, key))
    }
}

fn add_file(store: &mut Store, file: &FileInfo, path: String, source: Option<&str>) {
    let key = format!("cloud://{}", file.id);
    let mut track = source
        .or(Some(key.as_str()))
        .and_then(|old| store.tracks.iter().find(|t| t.path == old).cloned())
        .unwrap_or_default();
    track.path = key.clone();
    track.video_id = None;
    track.clip_video_id = None;
    if track.title.is_empty() {
        track.title = file.name.clone();
    }
    if let Some(old) = source {
        store.tracks.retain(|t| t.path != old);
        if store.likes.remove(old) {
            store.likes.insert(key.clone());
        }
        for p in &mut store.playlists {
            for t in &mut p.tracks {
                if t == old {
                    *t = key.clone();
                }
            }
        }
        for h in &mut store.history {
            if h.track.path == old {
                h.track = track.clone();
            }
        }
        if let Some(choice) = store.lyrics_choices.remove(old) {
            store.lyrics_choices.insert(key.clone(), choice);
        }
        if let Some(offset) = store.offsets.remove(old) {
            store.offsets.insert(key.clone(), offset);
        }
    }
    store.tracks.retain(|t| t.path != key);
    store.tracks.push(track);
    store.downloads.insert(key, path);
}

fn qr_image(uri: &str) -> Option<slint::Image> {
    use qrcode::{Color, EcLevel, QrCode};
    let qr = QrCode::with_error_correction_level(uri.as_bytes(), EcLevel::M).ok()?;
    let modules = qr.width();
    let quiet = 4usize;
    let scale = 4usize;
    let side = (modules + quiet * 2) * scale;
    let mut rgba = vec![255u8; side * side * 4];
    for y in 0..modules {
        for x in 0..modules {
            if qr[(x, y)] == Color::Dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let at = (((y + quiet) * scale + dy) * side + (x + quiet) * scale + dx) * 4;
                        rgba[at..at + 3].fill(0);
                    }
                }
            }
        }
    }
    Some(slint::Image::from_rgba8(slint::SharedPixelBuffer::<
        slint::Rgba8Pixel,
    >::clone_from_slice(
        &rgba, side as u32, side as u32
    )))
}
fn clear_device_ui(ui: &MainWindow) {
    ui.set_account_verification_url("".into());
    ui.set_account_verification_page("".into());
    ui.set_account_user_code("".into());
    ui.set_account_seconds_left(0);
    ui.set_account_qr(slint::Image::default());
}
fn clear_profile_ui(ui: &MainWindow) {
    ui.set_account_nickname("".into());
    ui.set_account_bio("".into());
    ui.set_account_discord("".into());
    ui.set_account_created_at("".into());
    ui.set_account_avatar(slint::Image::default());
}
fn apply_profile(session: &mut Session, ui: &MainWindow, value: &Value) {
    if let Some(id) = value["id"].as_str() { session.id = id.to_owned(); }
    session.email = value["email"].as_str().unwrap_or_default().to_owned();
    session.name = value["display_name"].as_str().unwrap_or_default().to_owned();
    session.nickname = value["nickname"].as_str().unwrap_or_default().to_owned();
    session.bio = value["bio"].as_str().unwrap_or_default().to_owned();
    session.discord = value["discord"].as_str().unwrap_or_default().to_owned();
    session.avatar_url = value["avatar_url"].as_str().unwrap_or_default().to_owned();
    session.created_at = value["created_at"].as_str().unwrap_or_default().to_owned();
    ui.set_account_email(session.email.clone().into());
    ui.set_account_name(session.name.clone().into());
    ui.set_account_nickname(session.nickname.clone().into());
    ui.set_account_bio(session.bio.clone().into());
    ui.set_account_discord(session.discord.clone().into());
    ui.set_account_created_at(session.created_at.clone().into());
}
fn set_avatar_image(ui: &MainWindow, image: Option<image::RgbaImage>) {
    let image = image.map(|pixels| slint::Image::from_rgba8(
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            pixels.as_raw(), pixels.width(), pixels.height(),
        ),
    )).unwrap_or_default();
    ui.set_account_avatar(image);
}

pub fn install(
    w: &MainWindow,
    get: Rc<dyn Fn() -> Store>,
    apply: Rc<dyn Fn(Store)>,
    #[cfg(not(target_arch = "wasm32"))] store_revision: Rc<dyn Fn() -> u64>,
    #[cfg(not(target_arch = "wasm32"))] runtime: std::sync::Arc<tokio::runtime::Runtime>,
) -> slint::Timer {
    struct State {
        session: Session,
        active: bool,
        generation: u64,
        last_snapshot: String,
        device: Option<(String, u64)>,
        open_browser: bool,
        files: Vec<FileInfo>,
        next_poll: u64,
        poll_interval: u64,
        next_sync: u64,
        next_ui: u64,
        verified: bool,
        locals: Vec<String>,
        #[cfg(not(target_arch = "wasm32"))]
        match_attempts: BTreeMap<String, u64>,
        #[cfg(not(target_arch = "wasm32"))]
        match_generation: Option<u64>,
    }
    let mut session = load_session();
    if session.api.is_empty() {
        session.api = "https://blueberry-melody.superstilist.workers.dev".into();
        session.auto_sync = true;
    }
    if session.expires_at < crate::services::now_ms() {
        session.token.clear();
        session.revision = None;
    }
    w.set_account_api(session.api.clone().into());
    w.set_account_email(session.email.clone().into());
    w.set_account_name(session.name.clone().into());
    w.set_account_nickname(session.nickname.clone().into());
    w.set_account_bio(session.bio.clone().into());
    w.set_account_discord(session.discord.clone().into());
    w.set_account_created_at(session.created_at.clone().into());
    w.set_account_avatar(slint::Image::default());
    w.set_account_current_password("".into());
    w.set_account_new_password("".into());
    w.set_account_signed_in(!session.token.is_empty());
    w.set_account_premium(session.premium);
    w.set_account_auto_sync(session.auto_sync);
    w.set_account_auto_upload(session.auto_upload);
    let state = Rc::new(RefCell::new(State {
        session,
        active: false,
        generation: 0,
        last_snapshot: String::new(),
        device: None,
        open_browser: true,
        files: vec![],
        next_poll: 0,
        poll_interval: 5000,
        next_sync: crate::services::now_ms() + 3000,
        next_ui: 0,
        verified: false,
        locals: Vec::new(),
        #[cfg(not(target_arch = "wasm32"))]
        match_attempts: BTreeMap::new(),
        #[cfg(not(target_arch = "wasm32"))]
        match_generation: None,
    }));
    let session_writer = Rc::new(SessionWriter::new());
    let (tx, rx) = std::sync::mpsc::channel::<(u64, Result<Output, String>)>();
    let dispatch: Rc<dyn Fn(u64, Session, Action)> = {
        let tx = tx.clone();
        Rc::new(move |generation, s, action| {
            let tx = tx.clone();
            let transfer = matches!(
                &action,
                Action::Upload(_) | Action::Auto(_) | Action::Download(_) | Action::UploadAvatar(_)
            );
            #[cfg(not(target_arch = "wasm32"))]
            runtime.spawn(async move {
                let result = work(s, action).await;
                if transfer {
                    TRANSFER_TOTAL.store(0, std::sync::atomic::Ordering::Relaxed);
                }
                let _ = tx.send((generation, result));
            });
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let result = work(s, action).await;
                if transfer {
                    TRANSFER_TOTAL.store(0, std::sync::atomic::Ordering::Relaxed);
                }
                let _ = tx.send((generation, result));
            });
        })
    };
    let start: Rc<dyn Fn(&MainWindow, Action)> = {
        let state = state.clone();
        let dispatch = dispatch.clone();
        Rc::new(move |ui, action| {
            let mut a = state.borrow_mut();
            if a.active {
                return;
            }
            a.active = true;
            ui.set_account_busy(true);
            ui.set_account_status("З’єднання…".into());
            dispatch(a.generation, a.session.clone(), action);
        })
    };
    {
        let weak = w.as_weak();
        let state = state.clone();
        let start = start.clone();
        let get = get.clone();
        let dispatch = dispatch.clone();
        let session_writer = session_writer.clone();
        #[cfg(target_arch = "wasm32")]
        let tx = tx.clone();
        w.on_account_command(move |command| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let command = command.as_str();
            if command == "website" {
                let url = ui.get_account_verification_url().to_string();
                if !url.is_empty() {
                    open_page(&url);
                }
                return;
            }
            if command == "cancel-login" {
                let mut a = state.borrow_mut();
                a.generation += 1;
                a.device = None;
                a.active = false;
                ui.set_account_busy(false);
                clear_device_ui(&ui);
                ui.set_account_status("Вхід скасовано".into());
                close_login_page();
                return;
            }
            if command == "cancel" {
                CANCEL_UPLOAD.store(true, std::sync::atomic::Ordering::Relaxed);
                #[cfg(target_arch = "wasm32")]
                browser_cancel();
                ui.set_account_auto_upload(false);
                ui.set_account_status("Скасування завантаження…".into());
                return;
            }
            if state.borrow().active {
                return;
            }
            #[cfg(target_arch = "wasm32")]
            if command == "upload-avatar" {
                let promise = browser_pick_avatar();
                let (session, generation) = {
                    let mut a = state.borrow_mut();
                    a.active = true;
                    (a.session.clone(), a.generation)
                };
                ui.set_account_busy(true);
                ui.set_account_status("Виберіть зображення для аватара".into());
                let tx = tx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let result = async {
                        let value = wasm_bindgen_futures::JsFuture::from(promise).await
                            .map_err(|e| e.as_string().unwrap_or_else(|| "Не вдалося відкрити вибір зображення".into()))?;
                        if value.is_null() || value.is_undefined() { return Ok(Output::Unchanged); }
                        let bytes = js_sys::Uint8Array::new(&value).to_vec();
                        if bytes.is_empty() { return Ok(Output::Unchanged); }
                        work(session, Action::UploadAvatar(bytes)).await
                    }.await;
                    let _ = tx.send((generation, result));
                });
                return;
            }
            if command == "logout" {
                let mut a = state.borrow_mut();
                let old = a.session.clone();
                let generation = a.generation;
                a.generation += 1;
                a.session = Session {
                    api: old.api.clone(),
                    email: old.email.clone(),
                    ..Session::default()
                };
                a.device = None;
                a.files.clear();
                a.last_snapshot.clear();
                ui.set_account_signed_in(false);
                ui.set_account_premium(false);
                ui.set_account_auto_upload(false);
                ui.set_account_avatar(slint::Image::default());
                ui.set_account_nickname("".into());
                ui.set_account_bio("".into());
                ui.set_account_discord("".into());
                ui.set_account_created_at("".into());
                ui.set_account_current_password("".into());
                ui.set_account_new_password("".into());
                clear_device_ui(&ui);
                ui.set_account_cloud_files(slint::ModelRc::default());
                ui.set_account_status("Ви вийшли з акаунта".into());
                let _ = session_writer.request(&a.session);
                dispatch(generation, old, Action::Logout);
                return;
            }
            if matches!(command, "login" | "register" | "device" | "qr") {
                let url = match base(&ui.get_account_api()) {
                    Ok(v) => v,
                    Err(e) => {
                        ui.set_account_status(e.into());
                        return;
                    }
                };
                let mut a = state.borrow_mut();
                a.generation += 1;
                a.session = Session {
                    api: url,
                    auto_sync: ui.get_account_auto_sync(),
                    ..Session::default()
                };
                a.last_snapshot.clear();
                a.device = None;
                a.open_browser = command == "device";
                a.files.clear();
                ui.set_account_cloud_files(slint::ModelRc::default());
                ui.set_account_signed_in(false);
                ui.set_account_premium(false);
                ui.set_account_auto_upload(false);
                clear_device_ui(&ui);
            }
            let action = match command {
                "login" | "register" => {
                    let email = ui.get_account_email().to_string();
                    let password = ui.get_account_password().to_string();
                    let name = ui.get_account_name().to_string();
                    ui.set_account_password("".into());
                    Action::Login {
                        email,
                        password,
                        name,
                        register: command == "register",
                    }
                }
                "device" => {
                    if !prepare_login_page() {
                        ui.set_account_status(
                            "Дозвольте відкриття вікна входу або скористайтеся посиланням нижче."
                                .into(),
                        );
                    }
                    Action::Device
                }
                "qr" => Action::Device,
                "logout" => Action::Logout,
                "refresh" => Action::Account,
                "save-profile" => {
                    let email = ui.get_account_email().to_string();
                    let name = ui.get_account_name().to_string();
                    let nickname = ui.get_account_nickname().to_string();
                    let bio = ui.get_account_bio().to_string();
                    let discord = ui.get_account_discord().to_string();
                    let current_password = ui.get_account_current_password().to_string();
                    ui.set_account_current_password("".into());
                    Action::SaveProfile { email, name, nickname, bio, discord, current_password }
                }
                "change-password" => {
                    let current_password = ui.get_account_current_password().to_string();
                    let new_password = ui.get_account_new_password().to_string();
                    if current_password.is_empty() || new_password.len() < 10 {
                        ui.set_account_status("Вкажіть поточний пароль і новий пароль від 10 символів".into());
                        return;
                    }
                    ui.set_account_current_password("".into());
                    ui.set_account_new_password("".into());
                    Action::ChangePassword { current_password, new_password }
                }
                #[cfg(not(target_arch = "wasm32"))]
                "upload-avatar" => {
                    let Some(path) = crate::dialogs::FileDialog::new()
                        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                        .pick_file() else { return; };
                    Action::UploadAvatar(path.to_string_lossy().into_owned())
                }
                "delete-avatar" => Action::DeleteAvatar,
                "files" => Action::Files,
                "push" => Action::PreparePush(get(), ui.get_account_auto_upload()),
                "pull" => Action::Pull,
                "upload" => {
                    if !state.borrow().session.premium {
                        ui.set_account_status(
                            "Потрібне запрошення Premium для тестувальника або розробника".into(),
                        );
                        return;
                    }
                    let state = state.borrow();
                    let Some(path) = state
                        .locals
                        .get(ui.get_account_local_index().max(0) as usize)
                        .cloned()
                    else {
                        ui.set_account_status("Спершу додайте локальну пісню до бібліотеки".into());
                        return;
                    };
                    Action::Upload(path)
                }
                "download" => {
                    let a = state.borrow();
                    let Some(file) = a
                        .files
                        .get(ui.get_account_cloud_index().max(0) as usize)
                        .cloned()
                    else {
                        return;
                    };
                    Action::Download(file)
                }
                _ => return,
            };
            CANCEL_UPLOAD.store(false, std::sync::atomic::Ordering::Relaxed);
            start(&ui, action);
        });
    }
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(100),move||{
        let Some(ui)=weak.upgrade()else{return;};
        if let Some(error)=session_writer.take_error(){ui.set_account_status(error.into());}
        #[cfg(not(target_arch = "wasm32"))]
        let _timing = crate::player_debug::begin_span(&ui, "account_timer_callback");
        let total=TRANSFER_TOTAL.load(std::sync::atomic::Ordering::Relaxed);
        if total>0 {let sent=TRANSFER_DONE.load(std::sync::atomic::Ordering::Relaxed);ui.set_account_status(format!("Завантаження аудіо · {}%",sent*100/total).into());}
        while let Ok((generation,result))=rx.try_recv(){
            let mut a=state.borrow_mut();
            #[cfg(not(target_arch = "wasm32"))]
            let is_local_match=matches!(&result,Ok(Output::LocalMatched(..)));
            #[cfg(not(target_arch = "wasm32"))]
            if is_local_match && a.match_generation==Some(generation){a.match_generation=None;}
            if generation!=a.generation{continue;}
            #[cfg(not(target_arch = "wasm32"))]
            if !is_local_match{a.active=false;ui.set_account_busy(false);}
            #[cfg(target_arch = "wasm32")]
            {a.active=false;ui.set_account_busy(false);}
            match result {
                Ok(Output::PreparedPush(cloud))=>{
                    a.active=true;ui.set_account_busy(true);dispatch(a.generation,a.session.clone(),Action::Push(cloud));continue;
                }
                Ok(Output::PreparedInitialSnapshot(snapshot,auto_upload))=>{
                    if a.session.revision.is_none() && a.last_snapshot.is_empty() && a.session.auto_upload==auto_upload {a.last_snapshot=snapshot;}
                    continue;
                }
                Ok(Output::Prepared(action, auto_sync, auto_upload, revision))=>{
                    if a.session.auto_sync==auto_sync && a.session.auto_upload==auto_upload && a.session.revision==revision {
                        if let Some(action)=action{
                            #[cfg(not(target_arch = "wasm32"))]
                            if matches!(&*action,Action::MatchLocal(_)) {
                                if a.match_generation!=Some(a.generation){a.match_generation=Some(a.generation);dispatch(a.generation,a.session.clone(),*action);}
                                continue;
                            }
                            CANCEL_UPLOAD.store(false,std::sync::atomic::Ordering::Relaxed);a.active=true;ui.set_account_busy(true);dispatch(a.generation,a.session.clone(),*action);
                        }
                    }
                    continue;
                }
                #[cfg(not(target_arch = "wasm32"))]
                Ok(Output::PreparedRestore{store_revision:original,cloud_revision,prior_revision,auto_sync,prior_auto_upload,auto_upload,restored})=>{
                    if store_revision()!=original || a.session.revision!=prior_revision || a.session.auto_sync!=auto_sync || a.session.auto_upload!=prior_auto_upload || restored.is_none() {
                        a.session.auto_sync=false;ui.set_account_auto_sync(false);
                        ui.set_account_status("Дані змінилися під час відновлення. Виберіть зберегти або відновити бібліотеку.".into());
                    } else if let Some((store,snapshot))=restored {
                        a.session.revision=Some(cloud_revision);a.session.auto_upload=auto_upload;ui.set_account_auto_upload(auto_upload);
                        apply(store);a.last_snapshot=snapshot;ui.set_account_status("Бібліотеку й налаштування відновлено".into());
                    }
                }
                Ok(Output::Unchanged)=>{}
                Ok(Output::Session(s))=>{a.verified=false;a.session=s;ui.set_account_email(a.session.email.clone().into());ui.set_account_name(a.session.name.clone().into());clear_profile_ui(&ui);a.device=None;a.last_snapshot.clear();ui.set_account_signed_in(true);ui.set_account_premium(a.session.premium);clear_device_ui(&ui);close_login_page();ui.set_account_status("Вхід виконано. Збережіть дані пристрою або відновіть хмарну бібліотеку.".into());}
                Ok(Output::Pushed(revision,sent))=>{a.session.revision=Some(revision);a.last_snapshot=sent;ui.set_account_status("Бібліотеку збережено у хмарі".into());}
                Ok(Output::CatalogQueued(items,count,failure))=>{a.session.catalog_uploaded.extend(items);ui.set_account_status(match failure{Some(error)=>format!("До черги Blueberry додано {count}; решту буде повторено автоматично ({error})"),None=>format!("До черги Blueberry додано {count} треків")}.into());}
                #[cfg(not(target_arch = "wasm32"))]
                Ok(Output::LocalMatched(source, matched, retry_ms))=>{
                    if let Some(key)=local_catalog_track(&source){a.match_attempts.insert(key.path,crate::services::now_ms()+retry_ms);}
                    if let Some(matched)=matched.filter(|_|a.session.auto_sync && ui.get_account_auto_sync()) {
                        let mut store=get();
                        if let Some(track)=store.tracks.iter_mut().find(|track|
                            crate::services::normalize_path(&track.path)==crate::services::normalize_path(&source.path)
                                && crate::services::track_video_id(track).is_none()
                                && crate::services::same_track_identity(track,&source)) {
                            track.video_id=matched.video_id.clone();
                            track.catalog_metadata=Some(crate::services::CatalogMetadata {
                                title: matched.title.clone(),
                                artist: matched.artist.clone(),
                                album: matched.album.clone(),
                                duration_secs: matched.duration_secs,
                                thumbnail_url: matched.thumbnail_url.clone(),
                                local_source_key: local_catalog_track(&source).map(|local| local.path),
                            });
                            if track.clip_video_id.is_none(){track.clip_video_id=matched.clip_video_id;}
                            if let Some(id)=track.video_id.as_deref(){store.downloads.entry(format!("yt://{id}")).or_insert_with(||source.path.clone());}
                            store.migrate_identities();
                            apply(store);
                        }
                    }
                }
                Ok(Output::Pulled(revision,cloud,force))=>{
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let action=Action::PrepareRestore{store:get(),cloud,cloud_revision:revision,store_revision:store_revision(),last_snapshot:a.last_snapshot.clone(),force};
                        a.active=true;ui.set_account_busy(true);dispatch(a.generation,a.session.clone(),action);
                        continue;
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if !force && a.last_snapshot!=serde_json::to_string(&CloudLibrary::capture(&get(),ui.get_account_auto_upload())).unwrap_or_default(){a.session.auto_sync=false;ui.set_account_auto_sync(false);ui.set_account_status("Дані змінилися на обох пристроях. Виберіть зберегти або відновити бібліотеку.".into());let _=session_writer.request(&a.session);continue;}a.session.revision=Some(revision);a.session.auto_upload=cloud.auto_upload;ui.set_account_auto_upload(cloud.auto_upload);let mut store=get();cloud.restore(&mut store);apply(store);a.last_snapshot=if revision==0&&a.session.auto_sync{String::new()}else{serde_json::to_string(&CloudLibrary::capture(&get(),a.session.auto_upload)).unwrap_or_default()};ui.set_account_status("Бібліотеку й налаштування відновлено".into());
                    }
                }
                Ok(Output::Account(value,avatar))=>{a.verified=true;a.session.premium=value["premium"].as_bool().unwrap_or(false);apply_profile(&mut a.session,&ui,&value);set_avatar_image(&ui,avatar);ui.set_account_premium(a.session.premium);ui.set_account_status(if a.session.premium{"Premium · доступ розробника або тестувальника"}else{"Базовий акаунт · синхронізація налаштувань, улюбленого та історії"}.into());if a.session.revision.is_none()&&a.last_snapshot.is_empty(){let action=Action::PrepareInitialSnapshot(get(),a.session.auto_upload);a.active=true;dispatch(a.generation,a.session.clone(),action);}}
                Ok(Output::Profile(value))=>{apply_profile(&mut a.session,&ui,&value);ui.set_account_status("Дані акаунта збережено".into());}
                Ok(Output::Avatar(value,avatar))=>{apply_profile(&mut a.session,&ui,&value);set_avatar_image(&ui,avatar);ui.set_account_status("Аватар оновлено".into());}
                Ok(Output::Password)=>{ui.set_account_current_password("".into());ui.set_account_new_password("".into());ui.set_account_status("Пароль змінено. Інші Vey-сесії завершено".into());}
                Ok(Output::Files(files))=>{ui.set_account_cloud_files(Rc::new(slint::VecModel::from(files.iter().map(|f|format!("{} · {:.1} MiB",f.name,f.size as f64/1048576.).into()).collect::<Vec<slint::SharedString>>())).into());a.files=files;ui.set_account_status("Хмарні файли оновлено".into());}
                Ok(Output::Device(value))=>{
                    if let (Some(code),Some(uri),Some(public_code))=(value["device_code"].as_str(),value.get("verification_uri_complete").or_else(||value.get("verification_uri")).and_then(Value::as_str),value["user_code"].as_str()) {
                        let trusted=reqwest::Url::parse(uri).ok().zip(reqwest::Url::parse(&a.session.api).ok()).is_some_and(|(u,b)|u.origin()==b.origin()&&u.username().is_empty()&&u.password().is_none());
                        if trusted&&public_code.len()==8&&public_code.bytes().all(|b|b.is_ascii_hexdigit()) {
                            let lifetime=value["expires_in"].as_u64().unwrap_or(600).clamp(1,600);
                            a.device=Some((code.into(),crate::services::now_ms()+lifetime*1000));
                            a.poll_interval=value["interval"].as_u64().unwrap_or(5).max(5)*1000;a.next_poll=0;
                            ui.set_account_user_code(format!("{} {}",&public_code[..4],&public_code[4..]).into());
                            ui.set_account_seconds_left(lifetime as i32);
                            ui.set_account_verification_page(value["verification_uri"].as_str().unwrap_or_default().into());
                            ui.set_account_qr(qr_image(uri).unwrap_or_default());
                            ui.set_account_verification_url(uri.into());
                            if a.open_browser {open_page(uri);}
                            ui.set_account_status("Порівняй код і підтвердь вхід у своєму акаунті Vey.".into());
                        } else {clear_device_ui(&ui);close_login_page();ui.set_account_status("Сервер повернув некоректний код входу".into());}
                    } else {clear_device_ui(&ui);close_login_page();ui.set_account_status("Не вдалося створити код входу".into());}
                }
                Ok(Output::Pending(extra))=>{a.poll_interval+=extra;a.next_poll=crate::services::now_ms()+a.poll_interval;}
                Ok(Output::Logout)=>{a.generation+=1;a.session=Session{api:a.session.api.clone(),email:a.session.email.clone(),..Session::default()};a.files.clear();a.device=None;ui.set_account_signed_in(false);ui.set_account_premium(false);ui.set_account_auto_upload(false);clear_profile_ui(&ui);ui.set_account_status("Ви вийшли з акаунта".into());}
                Ok(Output::Uploaded(file,path,fingerprint))=>{a.session.uploaded.insert(fingerprint);let mut store=get();add_file(&mut store,&file,path.clone(),Some(&path));apply(store);a.files.push(file);ui.set_account_status("Аудіофайл збережено у приватній хмарі".into());}
                Ok(Output::Downloaded(file,path))=>{let mut store=get();add_file(&mut store,&file,path,None);apply(store);ui.set_account_status("Пісню додано до бібліотеки пристрою".into());}
                Err(error)=>{a.device=None;if a.session.token.is_empty(){close_login_page();clear_device_ui(&ui);}if error=="Сесія завершилася. Увійдіть знову."{a.session.token.clear();a.session.revision=None;a.files.clear();ui.set_account_signed_in(false);ui.set_account_premium(false);ui.set_account_cloud_files(slint::ModelRc::default());clear_profile_ui(&ui);}ui.set_account_status(error.clone().into());if a.session.revision.is_some()&&(error.contains("Library changed on another device")||error.contains("Conflict")){a.session.auto_sync=false;ui.set_account_auto_sync(false);}}
            }
            if let Err(error)=session_writer.request(&a.session){ui.set_account_status(error.into());}
        }
        let mut a=state.borrow_mut();let now=crate::services::now_ms();
        let changed=a.session.auto_sync!=ui.get_account_auto_sync()||a.session.auto_upload!=ui.get_account_auto_upload();
        a.session.auto_sync=ui.get_account_auto_sync();a.session.auto_upload=ui.get_account_auto_upload();
        if changed {let _=session_writer.request(&a.session);}
        if let Some((_,expires))=a.device.as_ref(){
            ui.set_account_seconds_left(((expires.saturating_sub(now)+999)/1000) as i32);
            if now>=*expires{a.generation+=1;a.active=false;a.device=None;ui.set_account_busy(false);clear_device_ui(&ui);close_login_page();ui.set_account_status("Код входу застарів. Почніть знову.".into());return;}
        }
        if a.active{return;}
        if let Some((code,_))=a.device.clone(){
            if now>=a.next_poll{a.next_poll=now+a.poll_interval;a.active=true;ui.set_account_busy(true);dispatch(a.generation,a.session.clone(),Action::Poll(code));}return;
        }
        if now<a.next_sync && now<a.next_ui{return;}
        let showing=ui.get_active_view()=="settings"&&ui.get_settings_section()=="account";
        if !showing&&a.session.token.is_empty(){a.next_ui=now+3000;return;}
        let store=get();
        if showing && now>=a.next_ui{
            a.next_ui=now+3000;
            a.locals=store.tracks.iter().filter(|t|!cloud_key(&t.path)).map(|t|t.path.clone()).collect();
            let names=store.tracks.iter().filter(|t|!cloud_key(&t.path)).map(|t|slint::SharedString::from(t.title.clone())).collect::<Vec<_>>();
            let model=ui.get_account_local_files();
            if model.row_count()!=names.len()||names.iter().enumerate().any(|(i,n)|model.row_data(i).as_ref()!=Some(n)){ui.set_account_local_files(Rc::new(slint::VecModel::from(names)).into());}
        }
        if !a.session.token.is_empty(){
            let check_due=now>=a.next_sync;
            if check_due{a.next_sync=now+15000;}
            if !showing { a.next_ui=now+3000; }
            let action=Action::Prepare{store,last_snapshot:a.last_snapshot.clone(),verified:a.verified,check_due,
                #[cfg(not(target_arch = "wasm32"))] match_attempts:a.match_attempts.clone(),
                #[cfg(not(target_arch = "wasm32"))] match_busy:a.match_generation==Some(a.generation)};
            a.active=true;dispatch(a.generation,a.session.clone(),action);
        }
    });
    timer
}
fn prepare_login_page() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        browser_open_login("");
        true
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}
fn close_login_page() {
    #[cfg(target_arch = "wasm32")]
    {
        browser_open_login("#close");
    }
}
fn open_page(url: &str) {
    #[cfg(target_arch = "wasm32")]
    browser_open_login(url);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = crate::native::open_url(url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unplayed_recommendations_are_not_submitted_to_shared_catalog() {
        let suggestion = Track { path: "yt://abcdefghijk".into(), video_id: Some("abcdefghijk".into()),
            title: "Suggested song".into(), artist: "Artist".into(), ..Default::default() };
        let mut store = Store::default();
        store.recommendations.tracks.push(suggestion.clone());
        store.remember(suggestion.clone());
        store.history.push(History { track: suggestion.clone(), ..Default::default() });
        assert!(catalog_candidates(&store, &BTreeMap::new()).is_empty());
        assert!(CloudLibrary::capture(&store, false).tracks.is_empty());
        store.history[0].play_count = 1;
        assert_eq!(catalog_candidates(&store, &BTreeMap::new()).len(), 1);
        store.history.clear();
        store.likes.insert(suggestion.path);
        assert_eq!(catalog_candidates(&store, &BTreeMap::new()).len(), 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn local_youtube_matching_requires_one_standard_recording() {
        let local = Track { path: "C:/Music/song.flac".into(), title: "Song".into(),
            artist: "Artist".into(), duration_secs: 180.0, ..Default::default() };
        let good = Track { path: "yt://abcdefghijk".into(), video_id: Some("abcdefghijk".into()),
            duration_secs: 183.0, ..local.clone() };
        let wrong_artist = Track { artist: "Another artist".into(), ..good.clone() };
        let wrong_length = Track { duration_secs: 200.0, ..good.clone() };
        let cover = Track { title: "Song (Cover)".into(), ..good.clone() };
        assert_eq!(select_local_youtube_match(&local, vec![wrong_artist, wrong_length, cover, good.clone()]), Some(good.clone()));
        let second = Track { path: "yt://other000001".into(), video_id: Some("other000001".into()), ..good.clone() };
        assert!(select_local_youtube_match(&local, vec![good.clone(), second]).is_none());
        assert!(select_local_youtube_match(&Track { duration_secs: 0.0, ..local.clone() }, vec![good.clone()]).is_none());

        let linked = Track { video_id: good.video_id, ..local.clone() };
        let store = Store { tracks: vec![linked], likes: BTreeSet::from([local.path.clone()]),
            playlists: vec![Playlist { tracks: vec![local.path], ..Default::default() }], ..Default::default() };
        let cloud = CloudLibrary::capture(&store, false);
        assert_eq!(cloud.tracks[0].path, "yt://abcdefghijk");
        assert!(cloud.likes.contains("yt://abcdefghijk"));
        assert_eq!(cloud.playlists[0].tracks, vec!["yt://abcdefghijk"]);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn automatic_local_matching_runs_only_when_sync_is_enabled_and_retry_is_due() {
        let local = Track { path: "C:/Music/song.flac".into(), title: "Song".into(),
            artist: "Artist".into(), duration_secs: 180.0, ..Default::default() };
        let store = Store { tracks: vec![local.clone()], ..Default::default() };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        for (enabled, retry_at, busy, expect_match) in [(true, 0, false, true), (false, 0, false, false),
            (true, u64::MAX, false, false), (true, 0, true, false)] {
            let session = Session { auto_sync: enabled, revision: Some(1), ..Default::default() };
            let snapshot = serde_json::to_string(&CloudLibrary::capture(&store, false)).unwrap();
            let attempts = BTreeMap::from([(local_catalog_track(&local).unwrap().path, retry_at)]);
            let output = runtime.block_on(work(session, Action::Prepare { store: store.clone(),
                last_snapshot: snapshot, verified: true, check_due: true, match_attempts: attempts, match_busy: busy })).unwrap();
            let Output::Prepared(action, ..) = output else { panic!("prepared action expected") };
            assert_eq!(action.is_some_and(|action| matches!(*action, Action::MatchLocal(_))), expect_match);
        }
    }

    #[test]
    fn indexed_account_identities_preserve_first_match_and_download_fallback() {
        let store = Store {
            tracks: vec![
                Track { path: "C:/Music/a.mp3".into(), ..Default::default() },
                Track { path: "C:/Music/a.mp3".into(), video_id: Some("abcdefghijk".into()), ..Default::default() },
                Track { path: "C:/Music/b.mp3".into(), video_id: Some("lmnopqrstuv".into()), ..Default::default() },
            ],
            downloads: BTreeMap::from([("yt://12345678901".into(), "C:/Music/a.mp3".into())]),
            ..Default::default()
        };
        let index = CloudIdentities::new(&store);
        for key in ["C:/Music/a.mp3", "C:/Music/b.mp3", "C:/Music/no.mp3", "yt://abcdefghijk", "vid:abcdefghijk"] {
            assert_eq!(index.identity(key), cloud_identity(&store, key), "{key}");
        }
        assert_eq!(index.identity("C:/Music/a.mp3").as_deref(), Some("yt://12345678901"));
    }

    #[test]
    fn catalog_collects_lyrics_from_local_alias_when_public_track_has_none() {
        let remote = Track { path: "yt://abcdefghijk".into(), title: "Song".into(), artist: "Artist".into(), ..Default::default() };
        let local = Track { path: "C:/Music/song.mp3".into(), video_id: Some("abcdefghijk".into()), title: "Song".into(), artist: "Artist".into(), ..Default::default() };
        let mut store = Store { tracks: vec![remote, local.clone()], ..Default::default() };
        store.cache_lyrics(local.path, vec![crate::lyrics::LyricsAlternative {
            provider: "local".into(),
            sync_type: crate::lyrics::LyricsSyncType::Plain,
            format: "text".into(),
            lyrics: crate::lyrics::ParsedLyrics { lines: vec![crate::lyrics::LyricLine { text: "Text".into(), ..Default::default() }], ..Default::default() },
        }]);
        let pending = catalog_candidates(&store, &BTreeMap::new());
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].2.lyrics_text, "Text");
        assert_eq!(pending[0].2.lyrics_variants.len(), 1);
    }

    #[test]
    fn catalog_snapshot_includes_best_provider_per_available_timing() {
        let remote = Track { path: "yt://abcdefghijk".into(), title: "Song".into(), artist: "Artist".into(), ..Default::default() };
        let local = Track { path: "C:/Music/song.mp3".into(), video_id: Some("abcdefghijk".into()), ..remote.clone() };
        let mut store = Store { tracks: vec![remote.clone(), local.clone()], ..Default::default() };
        let make = |provider: &str, start| crate::lyrics::LyricsAlternative {
            provider: provider.into(), sync_type: crate::lyrics::LyricsSyncType::Synced, format: "lrc".into(),
            lyrics: crate::lyrics::ParsedLyrics { lines: vec![crate::lyrics::LyricLine {
                text: "Lyrics".into(), start_ms: start, translation: "Translation".into(), ..Default::default()
            }], ..Default::default() },
        };
        store.cache_lyrics(remote.path, vec![make("provider-a", 0), make("provider-b", 1000)]);
        store.cache_lyrics(local.path, vec![make("Unison", 2000)]);
        let items = catalog_candidates(&snapshot(&store), &BTreeMap::new());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].2.recording_type, "unknown");
        assert_eq!(items[0].2.lyrics_variants.len(), 2);
        assert_eq!(items[0].2.lyrics_variants[0].sync_type, crate::lyrics::LyricsSyncType::Plain);
        assert_eq!(items[0].2.lyrics_variants[1].provider, "Unison");
        assert_eq!(items[0].2.lyrics_variants[1].lyrics.lines[0].start_ms, 2000);
        assert_eq!(items[0].2.lyrics_variants[1].lyrics.lines[0].translation, "Translation");
        assert_eq!(items[0].2.source_url, "https://music.youtube.com/watch?v=abcdefghijk");
    }

    #[test]
    fn recording_categories_require_explicit_version_labels() {
        assert_eq!(recording_type("Song (Club Mix)"), "mix");
        assert_eq!(recording_type("Song - Live at Home"), "live");
        assert_eq!(recording_type("I Believe"), "unknown");
        assert_eq!(recording_type("Cover Me"), "unknown");
    }

    #[test]
    fn catalog_waits_for_a_public_identity_for_local_tracks() {
        let local = Track { path: "C:/private/music/song.flac".into(), title: "Song".into(),
            artist: "Artist".into(), album: "Record".into(), duration_secs: 123., ..Default::default() };
        let mut store = Store { tracks: vec![local.clone()], ..Default::default() };
        store.cache_lyrics(local.path.clone(), vec![crate::lyrics::LyricsAlternative {
            provider: "fixture".into(), format: "plain".into(), sync_type: crate::lyrics::LyricsSyncType::Plain,
            lyrics: crate::lyrics::ParsedLyrics { lines: vec![crate::lyrics::LyricLine { text: "Words".into(), ..Default::default() }], ..Default::default() },
        }]);
        let items = catalog_candidates(&snapshot(&store), &BTreeMap::new());
        assert!(items.is_empty());
        let key = local_catalog_track(&local).unwrap().path;
        assert!(!key.contains("C:/private"));
        let mut moved = local; moved.path = "D:/elsewhere/song.flac".into();
        assert_eq!(local_catalog_track(&moved).unwrap().path, key);
    }

    #[test]
    fn catalog_does_not_submit_orphaned_lyrics_even_with_song_metadata() {
        let mut store = Store::default();
        let lyrics = |title: Option<&str>, artist: Option<&str>| crate::lyrics::LyricsAlternative {
            provider: "fixture".into(), format: "plain".into(), sync_type: crate::lyrics::LyricsSyncType::Plain,
            lyrics: crate::lyrics::ParsedLyrics { title: title.map(str::to_owned), artist: artist.map(str::to_owned),
                lines: vec![crate::lyrics::LyricLine { text: "Words".into(), ..Default::default() }], ..Default::default() },
        };
        store.cache_lyrics("C:/private/one.mp3".into(), vec![lyrics(Some("Song"), Some("Artist"))]);
        store.cache_lyrics("C:/private/two.mp3".into(), vec![lyrics(None, None)]);
        store.cache_lyrics("yt://abcdefghijk".into(), vec![lyrics(Some("Song"), Some("Artist"))]);
        let items = catalog_candidates(&snapshot(&store), &BTreeMap::new());
        assert!(items.is_empty());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn cloud_restore_is_prepared_off_thread_and_keeps_conflict_checks() {
        let local = Track { path: "C:/Music/local.flac".into(), title: "Local".into(), artist: "Artist".into(), ..Default::default() };
        let remote = Track { path: "yt://abcdefghijk".into(), title: "Remote".into(), artist: "Artist".into(), ..Default::default() };
        let store = Store { tracks: vec![local.clone()], ..Default::default() };
        let cloud = CloudLibrary::capture(&Store {
            tracks: vec![remote.clone()],
            likes: BTreeSet::from([remote.path.clone()]),
            ..Default::default()
        }, false);
        let baseline = serde_json::to_string(&CloudLibrary::capture(&store, false)).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        for (force, snapshot, expected) in [(false, baseline.as_str(), true), (false, "stale", false), (true, "stale", true)] {
            let output = runtime.block_on(work(Session::default(), Action::PrepareRestore {
                store: store.clone(), cloud: cloud.clone(), cloud_revision: 4, store_revision: 7,
                last_snapshot: snapshot.into(), force,
            })).unwrap();
            let Output::PreparedRestore { restored, store_revision, cloud_revision, .. } = output else { panic!("restore output expected") };
            assert_eq!((store_revision, cloud_revision), (7, 4));
            assert_eq!(restored.is_some(), expected);
            if let Some((restored, snapshot)) = restored {
                assert!(restored.tracks.iter().any(|track| track.path == local.path));
                assert!(restored.tracks.iter().any(|track| track.path == remote.path));
                assert_eq!(snapshot, serde_json::to_string(&CloudLibrary::capture(&restored, false)).unwrap());
            }
        }
    }

    #[test]
    fn catalog_splits_provider_versions_without_losing_them() {
        let item = CatalogImportTrack {
            path: "yt://abcdefghijk".into(), title: "Song".into(), artist: "Artist".into(), album: String::new(),
            duration_secs: 1.0, thumbnail_url: String::new(), clip_video_id: None,
            source_url: "https://music.youtube.com/watch?v=abcdefghijk".into(),
            youtube_music_url: "https://music.youtube.com/watch?v=abcdefghijk".into(), local_source_key: None,
            recording_type: "unknown".into(),
            lyrics_text: "Hello".into(), lyrics_tracks: Vec::new(),
            lyrics_variants: (0..101).map(|i| crate::lyrics::LyricsAlternative {
                provider: format!("provider-{i}"), format: "plain".into(), sync_type: crate::lyrics::LyricsSyncType::Plain,
                lyrics: crate::lyrics::ParsedLyrics { lines: vec![crate::lyrics::LyricLine { text: "Hello".into(), ..Default::default() }], ..Default::default() },
            }).collect(),
        };
        let parts = split_catalog_track(&item).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts.iter().map(|part| part.lyrics_variants.len()).sum::<usize>(), 101);
        assert_eq!(parts[0].lyrics_text, "Hello");
        assert!(parts[1].lyrics_text.is_empty());
        assert!(parts.iter().all(|part| serde_json::to_vec(part).unwrap().len() < 900_000));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn catalog_acknowledges_only_after_every_fragment_and_v2_response() {
        use std::io::{Read, Write};
        for scenario in ["success", "second_failed", "legacy"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let api = format!("http://{}", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let mut seen = 0;
                for index in 0..if scenario == "legacy" { 1 } else { 2 } {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                    let mut request = Vec::new();
                    let end = loop {
                        let mut bytes = [0; 4096];
                        let size = stream.read(&mut bytes).unwrap();
                        assert!(size > 0);
                        request.extend_from_slice(&bytes[..size]);
                        if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                            let size = headers.lines().find_map(|line| line.strip_prefix("content-length:")?.trim().parse::<usize>().ok()).unwrap();
                            if request.len() >= end + 4 + size { break end; }
                        }
                    };
                    let payload: Value = serde_json::from_slice(&request[end + 4..]).unwrap();
                    seen += payload["items"][0]["lyricsVariants"].as_array().unwrap().len();
                    let (status, body) = if scenario == "legacy" { (200, r#"{"accepted":1}"#) }
                        else if scenario == "second_failed" && index == 1 { (500, r#"{"error":{"message":"fixture"}}"#) }
                        else { (200, r#"{"accepted":1,"catalogVersion":2}"#) };
                    write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                }
                seen
            });
            let variant = crate::lyrics::LyricsAlternative { provider: "fixture".into(), format: "plain".into(),
                sync_type: crate::lyrics::LyricsSyncType::Plain, lyrics: crate::lyrics::ParsedLyrics::default() };
            let item = CatalogImportTrack { path: "yt://abcdefghijk".into(), title: "Song".into(), artist: "Artist".into(),
                album: String::new(), duration_secs: 0., thumbnail_url: String::new(), clip_video_id: None,
                source_url: "https://music.youtube.com/watch?v=abcdefghijk".into(),
                youtube_music_url: "https://music.youtube.com/watch?v=abcdefghijk".into(), local_source_key: None,
                recording_type: "unknown".into(),
                lyrics_text: String::new(), lyrics_tracks: Vec::new(), lyrics_variants: vec![variant; 101] };
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let result = runtime.block_on(work(Session { api, ..Default::default() },
                Action::CatalogImport(vec![(item.path.clone(), "fingerprint".into(), item)]))).unwrap();
            let Output::CatalogQueued(acknowledged, accepted, failure) = result else { panic!("catalog output expected") };
            let seen = server.join().unwrap();
            if scenario == "success" {
                assert_eq!(seen, 101); assert_eq!(accepted, 1); assert_eq!(acknowledged.len(), 1); assert!(failure.is_none());
            } else {
                assert_eq!(accepted, 0); assert!(acknowledged.is_empty()); assert!(failure.is_some());
            }
        }
    }

    #[test]
    #[ignore = "comparative performance fixture"]
    fn catalog_alias_lookup_performance() {
        let store = if let Some(path) = std::env::var_os("VEY_ACCOUNT_BENCH_STATE") {
            let contents = std::fs::read(path).expect("read benchmark state");
            let original: Store = serde_json::from_slice(&contents).expect("parse benchmark state");
            snapshot(&original)
        } else {
            let mut tracks = (0..88).map(|i| Track {
                path: format!("C:/Music/local-{i}.mp3"), ..Default::default()
            }).collect::<Vec<_>>();
            tracks.extend((0..472).map(|i| Track {
                path: format!("yt://{i:011}"), title: "Song".into(), artist: "Artist".into(), ..Default::default()
            }));
            Store { tracks, ..Default::default() }
        };
        let ids = store.tracks.iter().filter(|t| t.path.starts_with("yt://")).map(|t| t.path.clone()).collect::<Vec<_>>();
        let start = std::time::Instant::now();
        let old = ids.iter().map(|id| store.tracks.iter().position(|track| cloud_identity(&store, &track.path).as_deref() == Some(id.as_str()))).collect::<Vec<_>>();
        let old_time = start.elapsed();
        let start = std::time::Instant::now();
        let identities = CloudIdentities::new(&store);
        let mut indexed = BTreeMap::new();
        for (position, track) in store.tracks.iter().enumerate() {
            if let Some(id) = identities.identity(&track.path) { indexed.entry(id).or_insert(position); }
        }
        let new = ids.iter().map(|id| indexed.get(id).copied()).collect::<Vec<_>>();
        let new_time = start.elapsed();
        assert_eq!(old, new);
        println!("{} tracks, {} lookups: old={old_time:?} indexed={new_time:?}", store.tracks.len(), ids.len());
    }

    #[test]
    fn catalog_retry_keeps_public_metadata_and_cached_lyrics_only_until_acknowledged() {
        let track = Track {
            path: "C:/Music/song.flac".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_secs: 181.0,
            thumbnail_url: "https://img.youtube.com/vi/abcdefghijk/hqdefault.jpg".into(),
            ..Default::default()
        };
        let mut store = Store {
            tracks: vec![track.clone()],
            ..Default::default()
        };
        store.cache_lyrics(
            track.path.clone(),
            vec![crate::lyrics::LyricsAlternative {
                provider: "local".into(),
                sync_type: crate::lyrics::LyricsSyncType::Synced,
                format: "lrc".into(),
                lyrics: crate::lyrics::ParsedLyrics {
                    lines: vec![crate::lyrics::LyricLine {
                        start_ms: 1250,
                        text: "歌詞".into(),
                        romanization: "kashi".into(),
                        translation: "Переклад".into(),
                        translations: BTreeMap::from([("uk".into(), "Переклад".into())]),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            }],
        );

        let pending = catalog_candidates(&store, &BTreeMap::new());
        assert_eq!(pending.len(), 1);
        let (id, fingerprint, item) = &pending[0];
        assert_eq!(id, "yt://abcdefghijk");
        assert!(item.path.starts_with("yt://"));
        assert!(item.thumbnail_url.starts_with("https://"));
        assert!(item.lyrics_text.contains("歌詞"));
        assert!(item.lyrics_tracks.iter().any(|layer| layer.kind == "romanization" && layer.text.contains("kashi")));
        assert!(item.lyrics_tracks.iter().any(|layer| layer.kind == "translation" && layer.text.contains("Переклад")));
        let public_json = serde_json::to_string(item).unwrap();
        assert!(!public_json.contains("C:/Music"));
        assert!(!public_json.contains("audio"));

        let acknowledged = BTreeMap::from([(id.clone(), fingerprint.clone())]);
        assert!(catalog_candidates(&store, &acknowledged).is_empty());
        store.tracks[0].album = "Updated album".into();
        assert_eq!(catalog_candidates(&store, &acknowledged).len(), 1);
    }

    #[test]
    fn imported_playlists_use_deployed_cloud_contract_and_keep_local_appearance() {
        let mut store = Store::default();
        let imported = crate::library_import::ImportedPlaylist {
            id: "youtube-PL1234567890".into(),
            name: "Imported".into(),
            tracks: vec![Track {
                path: "yt://abcdefghijk".into(),
                video_id: Some("abcdefghijk".into()),
                ..Default::default()
            }],
        };
        crate::library_import::apply(&mut store, &imported, None).unwrap();
        store.settings.home_layout = 4;
        store.settings.home_rotate = true;
        store.settings.lyrics_theme_css = "local style".into();
        let cloud = CloudLibrary::capture(&store, false);
        let value = serde_json::to_value(&cloud).unwrap();
        assert_eq!(value["playlists"][0]["tracks"][0], "yt://abcdefghijk");
        assert!(value["settings"].get("home_layout").is_none());
        assert!(value["settings"].get("lyrics_theme_css").is_none());
        let received: CloudLibrary = serde_json::from_value(value).unwrap();
        received.restore(&mut store);
        assert_eq!(store.settings.home_layout, 4);
        assert!(store.settings.home_rotate);
        assert_eq!(store.settings.lyrics_theme_css, "local style");
    }
    #[test]
    fn cloud_snapshot_excludes_local_paths_and_credentials_and_preserves_settings() {
        let mut store = Store::default();
        let remote = Track {
            path: "yt://abcdefghijk".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Saved song".into(),
            thumbnail_url: "C:/private/cover.jpg".into(),
            ..Track::default()
        };
        let local = Track {
            path: "C:/private/music/song.flac".into(),
            title: "Local song".into(),
            ..Track::default()
        };
        store.tracks = vec![remote.clone(), local.clone()];
        store
            .likes
            .extend([remote.path.clone(), local.path.clone()]);
        store.playlists.push(Playlist {
            id: "mixed".into(),
            name: "My playlist".into(),
            tracks: vec![remote.path.clone(), local.path.clone()],
            ..Default::default()
        });
        store.history = vec![
            History {
                track: remote.clone(),
                played_at: 123,
                play_count: 4,
                ..Default::default()
            },
            History {
                track: local.clone(),
                played_at: 124,
                play_count: 2,
                ..Default::default()
            },
        ];
        store
            .legacy
            .insert("password".into(), json!("private-password"));
        store.sources.push(crate::services::Source {
            id: "source".into(),
            kind: "folder".into(),
            path: "C:/private/music".into(),
        });
        store.settings.volume = 0.25;
        store.settings.lyrics_translation = true;
        let mut cloud = CloudLibrary::capture(&store, false);
        let text = serde_json::to_string(&cloud).unwrap();
        assert!(!text.contains("C:/private"));
        assert!(!text.contains("private-password"));
        assert_eq!(cloud.history.len(), 1);
        assert_eq!(cloud.history[0].play_count, 4);
        assert_eq!(cloud.settings.volume, 0.25);
        assert!(cloud.settings.lyrics_translation);
        cloud.likes.clear();
        cloud.settings.volume = 0.6;
        cloud.restore(&mut store);
        assert!(store.tracks.iter().any(|t| t.path == local.path));
        assert!(store.likes.contains(&local.path));
        assert!(!store.likes.contains(&remote.path));
        assert!(store.playlists[0].tracks.contains(&local.path));
        assert_eq!(store.settings.volume, 0.6);
        assert_eq!(store.history.len(), 2);
        assert!(store.legacy.contains_key("password"));
    }

    #[test]
    fn recommendation_profile_is_private_and_restores_from_account_snapshot() {
        let mut local = Store::default();
        local.recommendations.profile_updated_at = 10;
        local.recommendations.track_weights.insert("yt://abcdefghijk".into(), 7);
        local.recommendations.track_weights.insert("C:/private/song.mp3".into(), 9);
        let captured = CloudLibrary::capture(&local, false);
        assert!(captured.recommendations.track_weights.contains_key("yt://abcdefghijk"));
        assert!(!captured.recommendations.track_weights.contains_key("C:/private/song.mp3"));
        assert!(!serde_json::to_string(&captured).unwrap().contains("C:/private"));

        let restored = CloudLibrary {
            recommendations: RecommendationModel {
                schema_version: 1,
                profile_updated_at: 20,
                updated_at: 30,
                personalized: true,
                track_weights: BTreeMap::from([("yt://abcdefghijk".into(), 42)]),
                artist_weights: BTreeMap::from([("artist".into(), 11)]),
                disliked_tracks: BTreeSet::from(["yt://lmnopqrstuv".into()]),
                tracks: vec![Track {
                    path: "yt://lmnopqrstuv".into(),
                    video_id: Some("lmnopqrstuv".into()),
                    title: "Recommended".into(),
                    ..Default::default()
                }],
            },
            ..Default::default()
        };
        restored.restore(&mut local);
        assert_eq!(local.recommendations.profile_updated_at, 20);
        assert_eq!(local.recommendations.track_weights["yt://abcdefghijk"], 42);
        assert!(local.recommendations.disliked_tracks.contains("yt://lmnopqrstuv"));
        assert_eq!(local.recommendations.tracks[0].title, "Recommended");
    }

    #[test]
    fn tagged_local_files_sync_under_their_video_key() {
        let local = Track {
            path: "C:/private/music/song.mp3".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Local song".into(),
            ..Default::default()
        };
        let mut store = Store {
            tracks: vec![local.clone()],
            ..Default::default()
        };
        store.likes.insert(local.path.clone());
        store.playlists.push(Playlist {
            id: "mix".into(),
            name: "Mix".into(),
            tracks: vec![local.path.clone()],
            ..Default::default()
        });
        store.history.push(History {
            track: local.clone(),
            played_at: 10,
            play_count: 3,
            ..Default::default()
        });
        store.offsets.insert(local.path.clone(), 500);
        store.lyrics_choices.insert(
            local.path.clone(),
            crate::services::LyricsChoice {
                provider: "auto".into(),
                format: "lrc".into(),
                sync_type: crate::lyrics::LyricsSyncType::Synced,
                fingerprint: "match".into(),
            },
        );
        let cloud = CloudLibrary::capture(&store, false);
        let text = serde_json::to_string(&cloud).unwrap();
        assert!(!text.contains("C:/private"));
        assert_eq!(cloud.tracks[0].path, "yt://abcdefghijk");
        assert_eq!(cloud.likes.iter().collect::<Vec<_>>(), ["yt://abcdefghijk"]);
        assert_eq!(cloud.playlists[0].tracks, ["yt://abcdefghijk"]);
        assert_eq!(cloud.history[0].track.path, "yt://abcdefghijk");
        assert_eq!(cloud.offsets["yt://abcdefghijk"], 500);
        assert!(cloud.lyrics_choices.contains_key("yt://abcdefghijk"));

        let mut restored = store.clone();
        cloud.restore(&mut restored);
        assert!(restored.tracks.iter().any(|t| t.path == local.path));
        assert!(restored.likes.contains("yt://abcdefghijk"));
        assert!(!restored.likes.contains(&local.path));
        assert_eq!(restored.playlists[0].tracks, ["yt://abcdefghijk"]);
        assert_eq!(restored.history.len(), 1);
        assert_eq!(restored.offsets["yt://abcdefghijk"], 500);
    }

    #[test]
    fn api_url_cannot_embed_credentials_or_downgrade_https() {
        assert_eq!(
            base("https://example.test/api/v1/").unwrap(),
            "https://example.test/api/v1"
        );
        assert!(base("http://127.0.0.1:4879").is_ok());
        for bad in [
            "http://example.test",
            "https://user:pass@example.test",
            "https://example.test?token=secret",
            "javascript:alert(1)",
        ] {
            assert!(base(bad).is_err(), "{bad}");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires isolated Miniflare fixture on 127.0.0.1:4879"]
    fn native_client_worker_contract() {
        let api =
            std::env::var("VEY_FIXTURE_API").expect("set VEY_FIXTURE_API for isolated fixture");
        assert_eq!(api, "http://127.0.0.1:4879");
        let dir = std::env::current_dir()
            .unwrap()
            .join("target/account-contract");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PLAYERVF_DATA_DIR", &dir);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initial = Session {
                api,
                auto_sync: true,
                ..Session::default()
            };
            let email = format!("rust-{}@example.test", crate::services::now_ms());
            let Output::Session(mut signed) = work(
                initial.clone(),
                Action::Login {
                    email,
                    password: "Fixture-only-password-123".into(),
                    name: "Rust fixture".into(),
                    register: true,
                },
            )
            .await
            .unwrap() else {
                panic!("session expected")
            };
            let Output::Device(device) = work(signed.clone(), Action::Device).await.unwrap() else {
                panic!("OAuth device expected")
            };
            let code = device["device_code"].as_str().unwrap().to_owned();
            assert!(matches!(
                work(signed.clone(), Action::Poll(code.clone()))
                    .await
                    .unwrap(),
                Output::Pending(0)
            ));
            request(
                &signed,
                "auth/device/approve",
                "POST",
                Some(json!({"user_code":device["user_code"]})),
            )
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(5200)).await;
            let Output::Session(oauth) = work(signed.clone(), Action::Poll(code)).await.unwrap()
            else {
                panic!("OAuth session expected")
            };
            assert_eq!(oauth.id, signed.id);
            work(oauth, Action::Logout).await.unwrap();
            let mut store = Store::default();
            let track = Track {
                path: "yt://abcdefghijk".into(),
                video_id: Some("abcdefghijk".into()),
                title: "Rust contract".into(),
                ..Track::default()
            };
            store.tracks.push(track.clone());
            store.likes.insert(track.path.clone());
            store.history.push(History {
                track: track.clone(),
                played_at: 1234,
                play_count: 7,
                ..Default::default()
            });
            store.lyrics_choices.insert(
                track.path.clone(),
                crate::services::LyricsChoice {
                    provider: "fixture".into(),
                    format: "lrc".into(),
                    sync_type: crate::lyrics::LyricsSyncType::Synced,
                    fingerprint: "a".repeat(40),
                },
            );
            let expected = CloudLibrary::capture(&store, false);
            let Output::Pushed(revision, _) = work(signed.clone(), Action::Push(expected.clone()))
                .await
                .unwrap()
            else {
                panic!("push expected")
            };
            signed.revision = Some(revision);
            let Output::Pulled(_, actual, _) = work(signed.clone(), Action::Pull).await.unwrap()
            else {
                panic!("pull expected")
            };
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
            let file = dir.join("contract.mp3");
            let mut audio = vec![0u8; 5 * 1024 * 1024 + 2048];
            audio[..3].copy_from_slice(b"ID3");
            std::fs::write(&file, &audio).unwrap();
            assert!(upload_file(&signed, file.to_string_lossy().into())
                .await
                .is_err());
            let Output::Session(admin) = work(
                initial,
                Action::Login {
                    email: "admin@example.test".into(),
                    password: "Fixture-only-password-123".into(),
                    name: String::new(),
                    register: false,
                },
            )
            .await
            .unwrap() else {
                panic!("admin expected")
            };
            request(
                &admin,
                &format!("admin/entitlements/{}", signed.id),
                "PUT",
                Some(json!({"premium":true,"reason":"tester"})),
            )
            .await
            .unwrap();
            let Output::Uploaded(uploaded, _, _) =
                upload_file(&signed, file.to_string_lossy().into())
                    .await
                    .unwrap()
            else {
                panic!("upload expected")
            };
            let Output::Downloaded(_, path) =
                download_file(&signed, uploaded.clone()).await.unwrap()
            else {
                panic!("download expected")
            };
            assert_eq!(std::fs::read(&path).unwrap(), audio);
            request(&signed, &format!("files/{}", uploaded.id), "DELETE", None)
                .await
                .unwrap();
            std::fs::remove_file(path).unwrap();
            std::fs::remove_file(file).unwrap();
            work(signed.clone(), Action::Logout).await.unwrap();
            assert!(request(&signed, "account", "GET", None).await.is_err());
        });
    }
}
