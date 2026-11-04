//! Portable application data. No window, audio device, Tauri, or account state.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Track {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub video_id: Option<String>,
    /// Paired music-video ID used for the visual clip while `video_id` stays the audio source.
    pub clip_video_id: Option<String>,
    pub thumbnail_url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct Source {
    pub id: String,
    pub kind: String,
    pub path: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<String>,
    pub author: String,
    pub updated_at: u64,
    pub is_public: bool,
    pub source_playlist_id: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct History {
    pub track: Track,
    #[serde(alias = "played_at")]
    pub played_at: u64,
    #[serde(alias = "play_count")]
    pub play_count: u32,
    #[serde(alias = "completion_count")]
    pub completion_count: u32,
    #[serde(alias = "skip_count")]
    pub skip_count: u32,
}
/// Private, compact recommendation profile and its cached public candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecommendationModel {
    pub schema_version: u16,
    pub updated_at: u64,
    pub profile_updated_at: u64,
    pub personalized: bool,
    /// Stable track identities only when syncing; local paths are filtered out.
    pub track_weights: BTreeMap<String, i16>,
    pub artist_weights: BTreeMap<String, i16>,
    pub disliked_tracks: BTreeSet<String>,
    pub tracks: Vec<Track>,
}
impl Default for RecommendationModel {
    fn default() -> Self {
        Self {
            schema_version: 1,
            updated_at: 0,
            profile_updated_at: 0,
            personalized: false,
            track_weights: BTreeMap::new(),
            artist_weights: BTreeMap::new(),
            disliked_tracks: BTreeSet::new(),
            tracks: Vec::new(),
        }
    }
}
impl RecommendationModel {
    pub fn merge_newer(&mut self, incoming: RecommendationModel) {
        if incoming.profile_updated_at > self.profile_updated_at {
            self.profile_updated_at = incoming.profile_updated_at;
            self.track_weights = incoming.track_weights;
            self.artist_weights = incoming.artist_weights;
            self.disliked_tracks = incoming.disliked_tracks;
        }
        if incoming.updated_at > self.updated_at {
            self.updated_at = incoming.updated_at;
            self.personalized = incoming.personalized;
            self.tracks = incoming.tracks;
        }
        self.schema_version = self.schema_version.max(incoming.schema_version);
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub home_layout: u8,
    pub home_rotate: bool,
    pub language: String,
    pub kawarp: KawarpSettings,
    pub volume: f32,
    pub color_mode: ColorMode,
    pub color_hue: u16,
    // Retained internally for audio/lyric clocks; removed user preference.
    #[serde(skip, default = "normal_playback_rate")]
    pub rate: f32,
    pub art_mode: String,
    pub download_badge_shown: bool,
    pub download_badge_style: String,
    pub download_badge_position: String,
    pub lyrics_provider: String,
    pub lyrics_theme_css: String,
    pub lyrics_font_family: String,
    pub theme_entire_player: bool,
    pub lyrics_romanization: bool,
    pub lyrics_translation: bool,
    pub lyrics_secondary_gap: f32,
    pub lyrics_translation_language: String,
    pub lyrics_stage: bool,
    pub lyrics_reduced_motion: bool,
    pub navigation_blur: bool,
    pub lyrics_passive_scroll: bool,
    pub shuffle: bool,
    pub repeat: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct KawarpSettings {
    pub opacity: f32,
    pub warp: f32,
    pub blur: u32,
    pub saturation: f32,
    pub dithering: f32,
    pub dim_bright: bool,
}
impl Default for KawarpSettings {
    fn default() -> Self {
        Self {
            opacity: 0.28,
            warp: 1.,
            blur: 15,
            saturation: 1.5,
            dithering: 0.02,
            dim_bright: true,
        }
    }
}
impl KawarpSettings {
    pub fn normalized(mut self) -> Self {
        let finite = |value: f32, fallback: f32, max: f32| {
            if value.is_finite() {
                value.clamp(0., max)
            } else {
                fallback
            }
        };
        self.opacity = finite(self.opacity, 0.28, 1.);
        self.warp = finite(self.warp, 1., 1.);
        self.blur = self.blur.clamp(1, 40);
        self.saturation = finite(self.saturation, 1.5, 2.);
        self.dithering = finite(self.dithering, 0.02, 0.05);
        self
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    Static,
    Background,
    #[default]
    #[serde(other)]
    Dynamic,
}
impl ColorMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Background => "background",
            Self::Dynamic => "dynamic",
        }
    }
    pub fn from_name(name: &str) -> Self {
        match name {
            "static" => Self::Static,
            "background" => Self::Background,
            _ => Self::Dynamic,
        }
    }
}
fn normal_playback_rate() -> f32 {
    1.
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            home_layout: 0,
            home_rotate: false,
            language: "uk".into(),
            kawarp: KawarpSettings::default(),
            volume: 1.,
            color_mode: ColorMode::Dynamic,
            color_hue: 85,
            rate: 1.,
            art_mode: "cover".into(),
            download_badge_shown: true,
            download_badge_style: "triangle".into(),
            download_badge_position: "bottom-right".into(),
            lyrics_provider: "auto".into(),
            lyrics_theme_css: String::new(),
            lyrics_font_family: String::new(),
            theme_entire_player: true,
            lyrics_romanization: true,
            lyrics_translation: true,
            lyrics_secondary_gap: 0.65,
            lyrics_translation_language: "auto".into(),
            lyrics_stage: false,
            lyrics_reduced_motion: false,
            navigation_blur: true,
            lyrics_passive_scroll: false,
            shuffle: false,
            repeat: "off".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedLyrics {
    #[serde(default)]
    pub parser_version: u32,
    pub saved_at: u64,
    pub alternatives: Vec<crate::lyrics::LyricsAlternative>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsChoice {
    pub provider: String,
    pub format: String,
    pub sync_type: crate::lyrics::LyricsSyncType,
    pub fingerprint: String,
}
fn lyrics_fingerprint(alt: &crate::lyrics::LyricsAlternative) -> String {
    use sha1::{Digest, Sha1};
    format!(
        "{:x}",
        Sha1::digest(serde_json::to_vec(alt).unwrap_or_default())
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub version: u32,
    pub migrated: bool,
    pub settings: Settings,
    pub sources: Vec<Source>,
    pub tracks: Vec<Track>,
    pub likes: BTreeSet<String>,
    pub playlists: Vec<Playlist>,
    pub history: Vec<History>,
    pub recommendations: RecommendationModel,
    pub home_catalog: crate::discovery::Catalog,
    pub offsets: BTreeMap<String, i64>,
    pub legacy: BTreeMap<String, Value>,
    pub lyrics_cache: BTreeMap<String, std::sync::Arc<CachedLyrics>>,
    /// Public track key -> complete permanent local file, never the stream cache.
    pub downloads: BTreeMap<String, String>,
    /// Explicitly hidden local library entries. This is optional in the v1
    /// store so older saves load unchanged.
    #[serde(default)]
    pub library_excluded: BTreeSet<String>,
    pub lyrics_choices: BTreeMap<String, LyricsChoice>,
    pub lyrics_enrichment: BTreeMap<String, crate::lyrics_translation::Enrichment>,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            version: 1,
            migrated: false,
            settings: Settings::default(),
            sources: vec![],
            tracks: vec![],
            likes: BTreeSet::new(),
            playlists: vec![],
            history: vec![],
            recommendations: RecommendationModel::default(),
            home_catalog: crate::discovery::Catalog::default(),
            offsets: BTreeMap::new(),
            legacy: BTreeMap::new(),
            lyrics_cache: BTreeMap::new(),
            downloads: BTreeMap::new(),
            library_excluded: BTreeSet::new(),
            lyrics_choices: BTreeMap::new(),
            lyrics_enrichment: BTreeMap::new(),
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> u64 {
    js_sys::Date::now().max(0.) as u64
}
#[cfg(not(target_arch = "wasm32"))]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
/// YouTube video ids are always 11 URL-safe characters.
pub fn video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
/// Keys that identify a stream/cloud entry rather than a local file.
pub fn key_is_remote(key: &str) -> bool {
    key.starts_with("yt://") || key.starts_with("cloud://")
}
pub fn track_video_id(track: &Track) -> Option<&str> {
    track
        .video_id
        .as_deref()
        .filter(|id| video_id(id))
        .or_else(|| {
            track
                .path
                .strip_prefix("yt://")
                .filter(|id| video_id(id))
        })
}
fn normalized_track_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}
fn same_track_metadata(a: &Track, b: &Track) -> bool {
    let title_a = normalized_track_text(&a.title);
    let title_b = normalized_track_text(&b.title);
    let artist_a = normalized_track_text(&a.artist);
    let artist_b = normalized_track_text(&b.artist);
    !title_a.is_empty()
        && !artist_a.is_empty()
        && title_a == title_b
        && artist_a == artist_b
        && a.duration_secs.is_finite()
        && b.duration_secs.is_finite()
        && a.duration_secs > 0.0
        && b.duration_secs > 0.0
        && (a.duration_secs - b.duration_secs).abs() <= 5.0
}
/// Match canonical YouTube IDs first. When only one side has an ID, use an
/// unambiguous exact metadata/duration match to pair a local file with its web
/// result. Two different IDs always represent separate versions.
pub fn same_track_identity(a: &Track, b: &Track) -> bool {
    match (track_video_id(a), track_video_id(b)) {
        (Some(left), Some(right)) => left == right,
        _ => same_track_metadata(a, b),
    }
}
fn merge_track_metadata(existing: &mut Track, incoming: Track) {
    if normalize_path(&existing.path) == normalize_path(&incoming.path) {
        *existing = incoming;
        return;
    }
    if key_is_remote(&existing.path) && !key_is_remote(&incoming.path) {
        existing.path = incoming.path.clone();
    }
    if track_video_id(existing).is_none() {
        existing.video_id = track_video_id(&incoming).map(str::to_owned);
    }
    if existing.title.trim().is_empty() || existing.title.eq_ignore_ascii_case("unknown title") {
        existing.title = incoming.title;
    }
    if existing.artist.trim().is_empty() || existing.artist.eq_ignore_ascii_case("unknown artist") {
        existing.artist = incoming.artist;
    }
    if existing.album.trim().is_empty() {
        existing.album = incoming.album;
    }
    if !existing.duration_secs.is_finite() || existing.duration_secs <= 0.0 {
        existing.duration_secs = incoming.duration_secs;
    }
    if existing.thumbnail_url.is_empty() {
        existing.thumbnail_url = incoming.thumbnail_url;
    }
}
/// Deduplicate a catalog/search/library batch in first-seen order. Metadata
/// fallback is accepted only when exactly one candidate fits.
pub fn deduplicate_tracks(tracks: impl IntoIterator<Item = Track>) -> Vec<Track> {
    let mut result = Vec::<Track>::new();
    let mut by_id = std::collections::HashMap::<String, usize>::new();
    let mut by_path = std::collections::HashMap::<String, usize>::new();
    let mut by_metadata = std::collections::HashMap::<(String, String), Vec<usize>>::new();
    for track in tracks {
        let id = track_video_id(&track).map(str::to_owned);
        let path = normalize_path(&track.path);
        let key = (
            normalized_track_text(&track.title),
            normalized_track_text(&track.artist),
        );
        let mut matched = id.as_ref().and_then(|id| by_id.get(id).copied());
        if matched.is_none() {
            matched = by_path.get(&path).copied();
        }
        if matched.is_none() && !key.0.is_empty() && !key.1.is_empty() {
            let candidates: Vec<usize> = by_metadata
                .get(&key)
                .into_iter()
                .flatten()
                .copied()
                .filter(|index| same_track_identity(&result[*index], &track))
                .collect();
            if candidates.len() == 1 {
                matched = candidates.first().copied();
            }
        }
        if let Some(index) = matched {
            merge_track_metadata(&mut result[index], track);
            by_path.insert(normalize_path(&result[index].path), index);
            if let Some(id) = track_video_id(&result[index]).map(str::to_owned) {
                by_id.entry(id).or_insert(index);
            }
        } else {
            let index = result.len();
            by_path.entry(path).or_insert(index);
            if let Some(id) = id {
                by_id.entry(id).or_insert(index);
            }
            if !key.0.is_empty() && !key.1.is_empty() {
                by_metadata.entry(key).or_default().push(index);
            }
            result.push(track);
        }
    }
    result
}
pub fn normalize_path(path: &str) -> String {
    let slash = path.trim().replace('\\', "/");
    let unc = slash.starts_with("//");
    let mut result = String::new();
    for part in slash.split('/').filter(|p| !p.is_empty()) {
        if !result.is_empty() {
            result.push('/');
        }
        result.push_str(part);
    }
    if unc {
        result.insert_str(0, "//");
    } else if slash.starts_with('/') {
        result.insert(0, '/');
    }
    if result.len() == 2 && result.ends_with(':') {
        result.push('/');
    }
    if result.is_empty() {
        result.push('/');
    }
    result
}
pub fn normalize_sources(sources: Vec<Source>) -> Vec<Source> {
    let mut seen = BTreeSet::new();
    sources
        .into_iter()
        .filter(|s| ["folder", "file"].contains(&s.kind.as_str()) && !s.path.trim().is_empty())
        .map(|mut s| {
            s.path = normalize_path(&s.path);
            s
        })
        .filter(|s| {
            let path = if cfg!(windows)
                || s.path.as_bytes().get(1) == Some(&b':')
                || s.path.starts_with("//")
            {
                s.path.to_lowercase()
            } else {
                s.path.clone()
            };
            seen.insert(format!("{}:{path}", s.kind))
        })
        .collect()
}
impl Store {
    fn lyrics_choice_key(&self, track: &Track) -> String {
        if let Some(id) = &track.video_id {
            return format!("yt://{id}");
        }
        self.downloads
            .iter()
            .find(|(_, p)| normalize_path(p) == normalize_path(&track.path))
            .map(|(key, _)| key.clone())
            .unwrap_or_else(|| track.path.clone())
    }
    pub fn remember_lyrics_choice(
        &mut self,
        track: &Track,
        alt: &crate::lyrics::LyricsAlternative,
    ) {
        let key = self.lyrics_choice_key(track);
        self.lyrics_choices.insert(
            key,
            LyricsChoice {
                provider: alt.provider.clone(),
                format: alt.format.clone(),
                sync_type: alt.sync_type,
                fingerprint: lyrics_fingerprint(alt),
            },
        );
    }
    pub fn preferred_lyrics_index(
        &self,
        track: &Track,
        alts: &[crate::lyrics::LyricsAlternative],
    ) -> usize {
        let Some(choice) = self.lyrics_choices.get(&self.lyrics_choice_key(track)) else {
            return 0;
        };
        alts.iter()
            .position(|alt| lyrics_fingerprint(alt) == choice.fingerprint)
            .or_else(|| {
                alts.iter().position(|alt| {
                    alt.provider.eq_ignore_ascii_case(&choice.provider)
                        && alt.format.eq_ignore_ascii_case(&choice.format)
                        && alt.sync_type == choice.sync_type
                })
            })
            .or_else(|| {
                alts.iter()
                    .position(|alt| alt.provider.eq_ignore_ascii_case(&choice.provider))
            })
            .unwrap_or(0)
    }
    pub fn lyrics_request_provider(&self, track: &Track) -> String {
        // A saved song choice takes precedence over the default provider filter.
        if self
            .lyrics_choices
            .contains_key(&self.lyrics_choice_key(track))
        {
            "auto".into()
        } else {
            self.settings.lyrics_provider.clone()
        }
    }
    pub fn remember(&mut self, track: Track) {
        self.remember_all(std::iter::once(track));
    }
    /// Merge scan/search batches by path or stable track identity, keeping the
    /// first-seen order and preferring a playable local path over a stream URL.
    pub fn remember_all(&mut self, tracks: impl IntoIterator<Item = Track>) {
        self.tracks = deduplicate_tracks(std::mem::take(&mut self.tracks).into_iter().chain(tracks));
    }
    /// Public identity shared by a local file and its streamed counterpart.
    /// Likes, playlists and account sync use it so the same song keeps one key
    /// across devices, while an unidentifiable local file keeps its own path.
    pub fn identity_key(&self, track: &Track) -> String {
        if let Some(id) = track_video_id(track) {
            return format!("yt://{id}");
        }
        if key_is_remote(&track.path) {
            return track.path.clone();
        }
        let local = normalize_path(&track.path);
        self.downloads
            .iter()
            .find(|(key, path)| key_is_remote(key) && normalize_path(path) == local)
            .map(|(key, _)| key.clone())
            .unwrap_or_else(|| track.path.clone())
    }
    pub fn is_library_excluded(&self, track: &Track) -> bool {
        self.library_excluded.contains(&track.path)
            || self.library_excluded.contains(&self.identity_key(track))
    }
    pub fn set_library_excluded(&mut self, track: &Track, excluded: bool) {
        let key = self.identity_key(track);
        if excluded {
            self.library_excluded.insert(key);
        } else {
            self.library_excluded.remove(&key);
            self.library_excluded.remove(&track.path);
        }
    }
    /// Relink a local file path while retaining every saved collection and
    /// playback reference that points at the old path.
    pub fn relink_local_path(&mut self, old_path: &str, replacement: Track) -> bool {
        let old_normalized = normalize_path(old_path);
        let Some(index) = self.tracks.iter().position(|track| {
            track.path == old_path || normalize_path(&track.path) == old_normalized
        }) else {
            return false;
        };
        if key_is_remote(&replacement.path) || replacement.path.trim().is_empty() {
            return false;
        }
        let old = self.tracks[index].clone();
        let old_key = self.identity_key(&old);
        let new_key = track_video_id(&replacement)
            .map(|id| format!("yt://{id}"))
            .or_else(|| key_is_remote(&old_key).then(|| old_key.clone()))
            .unwrap_or_else(|| replacement.path.clone());
        self.tracks[index] = replacement.clone();
        let replace_key = |key: &str| {
            if key == old_key
                || key == old_path
                || (!key_is_remote(key) && normalize_path(key) == old_normalized)
            {
                new_key.clone()
            } else {
                key.to_owned()
            }
        };
        self.likes = self.likes.iter().map(|key| replace_key(key)).collect();
        for playlist in &mut self.playlists {
            for key in &mut playlist.tracks {
                *key = replace_key(key);
            }
        }
        for entry in &mut self.history {
            if entry.track.path == old_path || normalize_path(&entry.track.path) == old_normalized {
                entry.track = replacement.clone();
            }
        }
        for path in self.downloads.values_mut() {
            if path == old_path || normalize_path(path) == old_normalized {
                *path = replacement.path.clone();
            }
        }
        if key_is_remote(&old_key) {
            self.downloads.insert(old_key.clone(), replacement.path.clone());
        }
        self.offsets = std::mem::take(&mut self.offsets)
            .into_iter()
            .map(|(key, value)| (replace_key(&key), value))
            .collect();
        self.lyrics_choices = std::mem::take(&mut self.lyrics_choices)
            .into_iter()
            .map(|(key, value)| (replace_key(&key), value))
            .collect();
        self.lyrics_cache = std::mem::take(&mut self.lyrics_cache)
            .into_iter()
            .map(|(key, value)| (replace_key(&key), value))
            .collect();
        self.lyrics_enrichment = std::mem::take(&mut self.lyrics_enrichment)
            .into_iter()
            .map(|(key, value)| (replace_key(&key), value))
            .collect();
        self.library_excluded = self.library_excluded.iter().map(|key| replace_key(key)).collect();
        self.refresh_recommendation_signals();
        true
    }
    pub fn is_liked(&self, track: &Track) -> bool {
        if self.likes.contains(&track.path) {
            return true;
        }
        let key = self.identity_key(track);
        if self.likes.contains(&key) {
            return true;
        }
        self.downloads
            .get(&key)
            .is_some_and(|local| self.likes.contains(local))
    }
    pub fn toggle_like_track(&mut self, track: &Track) {
        let key = self.identity_key(track);
        self.toggle_like(&key);
        self.recommendations.disliked_tracks.remove(&key);
        self.refresh_recommendation_signals();
    }
    /// Rewrite local paths in saved identities to their YouTube Music key once
    /// the file tags or download map reveal it. Run after every scan/restore.
    pub fn migrate_identities(&mut self) {
        let mut ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for track in &self.tracks {
            if let Some(id) = track_video_id(track) {
                ids.entry(normalize_path(&track.path))
                    .or_insert_with(|| format!("yt://{id}"));
            }
        }
        for (key, path) in &self.downloads {
            if key_is_remote(key) {
                ids.entry(normalize_path(path))
                    .or_insert_with(|| key.clone());
            }
        }
        let resolve = |key: &str| {
            (!key_is_remote(key))
                .then(|| ids.get(&normalize_path(key)).cloned())
                .flatten()
        };
        self.likes = self
            .likes
            .iter()
            .map(|key| resolve(key).unwrap_or_else(|| key.clone()))
            .collect();
        for playlist in &mut self.playlists {
            playlist.tracks = playlist
                .tracks
                .iter()
                .map(|key| resolve(key).unwrap_or_else(|| key.clone()))
                .collect();
        }
        let offsets = std::mem::take(&mut self.offsets);
        self.offsets = offsets
            .into_iter()
            .map(|(key, value)| (resolve(&key).unwrap_or(key), value))
            .collect();
        let choices = std::mem::take(&mut self.lyrics_choices);
        self.lyrics_choices = choices
            .into_iter()
            .map(|(key, value)| (resolve(&key).unwrap_or(key), value))
            .collect();
        self.recommendations.disliked_tracks = self
            .recommendations
            .disliked_tracks
            .iter()
            .map(|key| resolve(key).unwrap_or_else(|| key.clone()))
            .collect();
        self.library_excluded = self
            .library_excluded
            .iter()
            .map(|key| resolve(key).unwrap_or_else(|| key.clone()))
            .collect();
        self.refresh_recommendation_signals();
    }
    /// The library list: scanned local files plus saved remote songs. Search
    /// results, stream-cache entries and remote duplicates of local files stay
    /// out, so a synced account cannot inflate the library count.
    pub fn library_tracks(&self) -> Vec<Track> {
        let saved: HashSet<&str> = self
            .playlists
            .iter()
            .flat_map(|p| p.tracks.iter().map(String::as_str))
            .chain(self.likes.iter().map(String::as_str))
            .collect();
        let downloaded: HashSet<&str> = self
            .downloads
            .iter()
            .filter(|(_, path)| std::path::Path::new(path).is_file())
            .filter_map(|(key, _)| key.strip_prefix("yt://"))
            .collect();
        let local = self.tracks
            .iter()
            .filter(|track| !key_is_remote(&track.path) && !self.is_library_excluded(track))
            .cloned();
        let remote = self
            .tracks
            .iter()
            .filter(|track| {
                track.path.strip_prefix("yt://").is_some_and(|id| {
                    saved.contains(track.path.as_str())
                        && !downloaded.contains(id)
                        && !self.is_library_excluded(track)
                })
            })
            .cloned();
        deduplicate_tracks(local.chain(remote))
    }
    pub fn excluded_library_tracks(&self) -> Vec<Track> {
        deduplicate_tracks(
            self.tracks
                .iter()
                .filter(|track| self.is_library_excluded(track))
                .cloned(),
        )
    }
    /// Resolve in playlist order, including repeats; ignore missing references.
    pub fn playlist_tracks(&self, id: &str) -> Vec<Track> {
        let Some(playlist) = self.playlists.iter().find(|p| p.id == id) else {
            return vec![];
        };
        let mut tracks = std::collections::HashMap::with_capacity(self.tracks.len() * 2);
        for track in &self.tracks {
            tracks.entry(track.path.clone()).or_insert(track);
            tracks.entry(self.identity_key(track)).or_insert(track);
        }
        playlist
            .tracks
            .iter()
            .filter_map(|path| tracks.get(path.as_str()))
            .map(|track| (*track).clone())
            .collect()
    }
    pub fn record(&mut self, track: &Track) {
        self.remember(track.clone());
        let previous = self
            .history
            .iter()
            .position(|entry| same_track_identity(&entry.track, track))
            .map(|index| self.history.remove(index));
        self.history.insert(
            0,
            History {
                track: previous.as_ref().map_or_else(
                    || track.clone(),
                    |entry| {
                        let mut merged = entry.track.clone();
                        merge_track_metadata(&mut merged, track.clone());
                        merged
                    },
                ),
                played_at: now_ms(),
                play_count: previous
                    .as_ref()
                    .map_or(1, |entry| entry.play_count.saturating_add(1)),
                completion_count: previous
                    .as_ref()
                    .map_or(0, |entry| entry.completion_count),
                skip_count: previous.as_ref().map_or(0, |entry| entry.skip_count),
            },
        );
        self.history.truncate(100);
        self.refresh_recommendation_signals();
    }
    fn history_index(&self, track: &Track) -> Option<usize> {
        if let Some(id) = track_video_id(track) {
            if let Some(index) = self
                .history
                .iter()
                .position(|entry| track_video_id(&entry.track) == Some(id))
            {
                return Some(index);
            }
        }
        let candidates: Vec<_> = self
            .history
            .iter()
            .enumerate()
            .filter(|(_, entry)| same_track_metadata(&entry.track, track))
            .map(|(index, _)| index)
            .collect();
        (candidates.len() == 1).then(|| candidates[0])
    }
    pub fn record_completion(&mut self, track: &Track) {
        if let Some(index) = self.history_index(track) {
            self.history[index].completion_count =
                self.history[index].completion_count.saturating_add(1);
            self.history[index].played_at = now_ms();
        }
        self.refresh_recommendation_signals();
    }
    pub fn record_skip(&mut self, track: &Track) {
        if let Some(index) = self.history_index(track) {
            self.history[index].skip_count = self.history[index].skip_count.saturating_add(1);
        }
        self.refresh_recommendation_signals();
    }
    pub fn record_skip_if_early(&mut self, track: &Track, position_secs: f64, duration_secs: f64) {
        if duration_secs.is_finite()
            && duration_secs >= 15.0
            && position_secs.is_finite()
            && position_secs >= 0.0
            && position_secs / duration_secs < 0.6
        {
            self.record_skip(track);
        }
    }
    pub fn is_disliked(&self, track: &Track) -> bool {
        self.recommendations
            .disliked_tracks
            .contains(&self.identity_key(track))
    }
    pub fn toggle_dislike_track(&mut self, track: &Track) {
        let key = self.identity_key(track);
        if !self.recommendations.disliked_tracks.remove(&key) {
            self.likes.remove(&key);
            self.recommendations.disliked_tracks.insert(key);
        }
        self.refresh_recommendation_signals();
    }
    /// Rebuild bounded private ranking signals from listening and library
    /// activity. Account serialization strips non-portable local path keys.
    pub fn refresh_recommendation_signals(&mut self) {
        let now = now_ms();
        let mut track_weights = std::collections::HashMap::<String, i16>::new();
        let mut artist_weights = std::collections::HashMap::<String, i16>::new();
        for entry in &self.history {
            let age_days = now.saturating_sub(entry.played_at) / 86_400_000;
            let recent = 5i16.saturating_sub(age_days.min(5) as i16);
            let frequency = entry.play_count.min(12) as i16;
            let completion = entry.completion_count.min(8) as i16 * 2;
            let skips = entry.skip_count.min(8) as i16 * 3;
            let score = (frequency + completion + recent - skips).clamp(-24, 24);
            *track_weights.entry(self.identity_key(&entry.track)).or_default() += score;
            let artist = normalized_track_text(&entry.track.artist);
            if !artist.is_empty() {
                *artist_weights.entry(artist).or_default() += score;
            }
        }
        for key in &self.likes {
            *track_weights.entry(key.clone()).or_default() += 8;
            if let Some(track) = self
                .tracks
                .iter()
                .find(|track| self.identity_key(track) == *key)
            {
                let artist = normalized_track_text(&track.artist);
                if !artist.is_empty() {
                    *artist_weights.entry(artist).or_default() += 4;
                }
            }
        }
        for playlist in &self.playlists {
            for key in &playlist.tracks {
                *track_weights.entry(key.clone()).or_default() += 2;
            }
        }
        for key in &self.recommendations.disliked_tracks {
            *track_weights.entry(key.clone()).or_default() -= 12;
        }
        let bounded = |values: std::collections::HashMap<String, i16>, limit: usize| {
            let mut values: Vec<_> = values.into_iter().collect();
            values.sort_by(|a, b| {
                b.1.unsigned_abs()
                    .cmp(&a.1.unsigned_abs())
                    .then_with(|| a.0.cmp(&b.0))
            });
            values.truncate(limit);
            values
                .into_iter()
                .map(|(key, value)| (key, value.clamp(-100, 100)))
                .collect()
        };
        self.recommendations.track_weights = bounded(track_weights, 1000);
        self.recommendations.artist_weights = bounded(artist_weights, 500);
        self.recommendations.profile_updated_at = now;
    }
    pub fn cached_lyrics(&self, path: &str) -> Option<Vec<crate::lyrics::LyricsAlternative>> {
        let c = self.lyrics_cache.get(path)?;
        ((c.parser_version == 4 || c.alternatives.iter().all(|a| a.provider == "Власні слова"))
            && now_ms().saturating_sub(c.saved_at) < 30 * 24 * 3600 * 1000)
            .then(|| c.alternatives.clone())
    }
    pub fn cache_lyrics(
        &mut self,
        path: String,
        alternatives: Vec<crate::lyrics::LyricsAlternative>,
    ) {
        self.lyrics_cache.insert(
            path,
            std::sync::Arc::new(CachedLyrics {
                parser_version: 4,
                saved_at: now_ms(),
                alternatives,
            }),
        );
    }
    pub fn offset_for(&self, track: &Track) -> i64 {
        let key = self.identity_key(track);
        let video = track.video_id.as_ref().map(|id| format!("vid:{id}"));
        let tag = format!(
            "tag:{}|{}",
            track.artist.trim().to_lowercase(),
            track.title.trim().to_lowercase()
        );
        self.offsets
            .get(&track.path)
            .or_else(|| self.offsets.get(&key))
            .or_else(|| video.as_ref().and_then(|v| self.offsets.get(v)))
            .or_else(|| self.offsets.get(&tag))
            .copied()
            .unwrap_or(0)
    }
    pub fn toggle_like(&mut self, path: &str) {
        if !self.likes.remove(path) {
            self.likes.insert(path.into());
        }
    }
    pub fn create_playlist(&mut self, name: &str) -> Option<String> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let id = format!("local-{}-{}", now_ms(), self.playlists.len());
        self.playlists.push(Playlist {
            id: id.clone(),
            name: name.into(),
            tracks: vec![],
            updated_at: now_ms(),
            is_public: true,
            ..Default::default()
        });
        Some(id)
    }
    pub fn playlist_add(&mut self, id: &str, path: &str) {
        let mut changed = false;
        if let Some(p) = self.playlists.iter_mut().find(|p| p.id == id) {
            p.tracks.push(path.into());
            p.updated_at = now_ms();
            changed = true;
        }
        if changed {
            self.refresh_recommendation_signals();
        }
    }
    /// Add by public identity so a local file and its streamed counterpart do
    /// not become two rows, and the entry can sync to another device.
    pub fn playlist_add_track(&mut self, id: &str, track: &Track) {
        let key = self.identity_key(track);
        let mut changed = false;
        if let Some(p) = self.playlists.iter_mut().find(|p| p.id == id) {
            p.tracks.push(key);
            p.updated_at = now_ms();
            changed = true;
        }
        if changed {
            self.refresh_recommendation_signals();
        }
    }
    pub fn rename_playlist(&mut self, id: &str, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let Some(playlist) = self.playlists.iter_mut().find(|playlist| playlist.id == id) else {
            return false;
        };
        playlist.name = name.to_owned();
        playlist.updated_at = now_ms();
        true
    }
    pub fn move_playlist_track(&mut self, id: &str, from: usize, to: usize) -> bool {
        let Some(playlist) = self.playlists.iter_mut().find(|playlist| playlist.id == id) else {
            return false;
        };
        if from >= playlist.tracks.len() || to >= playlist.tracks.len() || from == to {
            return false;
        }
        let track = playlist.tracks.remove(from);
        playlist.tracks.insert(to, track);
        playlist.updated_at = now_ms();
        true
    }
    /// Import an explicitly exported localStorage object, once. Values may be JSON strings.
    /// An allowlist prevents credentials, Google sessions, JWTs and challenge tokens entering the store.
    pub fn migrate(&mut self, raw: &str) -> Result<usize, String> {
        if self.migrated {
            return Err("Дані вже перенесено".into());
        }
        let object: BTreeMap<String, Value> =
            serde_json::from_str(raw).map_err(|e| e.to_string())?;
        let mut count = 0;
        for (key, value) in object {
            let allowed = [
                "pvf:liked",
                "pvf:playlists",
                "pvf:library-sources",
                "pvf:folders",
                "pvf:listen-history:v1",
                "pvf:lyrics-provider",
                "pvf:ui-font",
                "pvf:ui-theme",
                "pvf:auto-download-ytm",
                "pvf:auto-stream-ytm",
                "playervf.artMode",
                "pvf:lyrics:custom-themes",
                "pvf:lyrics-theme",
                "pvf:lyrics:theme",
            ];
            if !(allowed.contains(&key.as_str())
                || key.starts_with("pvf:lyrics:offset:")
                || key.starts_with("pvf:lyrics:cache:")
                || key.starts_with("pvf:lyrics:video:")
                || key.starts_with("pvf:lyrics:v1:"))
            {
                continue;
            }
            let decoded = value
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or(value);
            match key.as_str() {
                "pvf:liked" => {
                    if let Ok(v) = serde_json::from_value(decoded.clone()) {
                        self.likes = v;
                    }
                }
                "pvf:library-sources" => {
                    if let Ok(v) = serde_json::from_value(decoded.clone()) {
                        self.sources = normalize_sources(v);
                    }
                }
                "pvf:folders" if self.sources.is_empty() => {
                    if let Some(v) = decoded.as_array() {
                        self.sources = normalize_sources(
                            v.iter()
                                .filter_map(Value::as_str)
                                .enumerate()
                                .map(|(i, p)| Source {
                                    id: format!("migrated-{i}"),
                                    kind: "folder".into(),
                                    path: p.into(),
                                })
                                .collect(),
                        );
                    }
                }
                "pvf:listen-history:v1" => {
                    if let Ok(v) = serde_json::from_value::<Vec<History>>(decoded.clone()) {
                        self.history = v.into_iter().take(100).collect();
                    }
                }
                "pvf:playlists" => {
                    if let Some(items) = decoded.as_array() {
                        self.playlists = items
                            .iter()
                            .filter_map(|p| {
                                let name = p.get("name").and_then(Value::as_str)?;
                                let tracks = p
                                    .get("tracks")
                                    .and_then(Value::as_array)
                                    .map(|ts| {
                                        ts.iter()
                                            .filter_map(|t| {
                                                t.as_str().or_else(|| {
                                                    t.get("path").and_then(Value::as_str)
                                                })
                                            })
                                            .map(str::to_string)
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                Some(Playlist {
                                    id: p.get("id").and_then(Value::as_str).unwrap_or(name).into(),
                                    name: name.into(),
                                    tracks,
                                    ..Default::default()
                                })
                            })
                            .collect();
                    }
                }
                "playervf.artMode" => {
                    self.settings.art_mode = decoded.as_str().unwrap_or("cover").into()
                }
                "pvf:lyrics-provider" => {
                    self.settings.lyrics_provider = decoded.as_str().unwrap_or("auto").into()
                }
                _ => {
                    if let Some(track) = key.strip_prefix("pvf:lyrics:offset:") {
                        if let Some(ms) = decoded
                            .as_i64()
                            .or_else(|| decoded.as_str().and_then(|s| s.parse().ok()))
                        {
                            self.offsets.insert(track.into(), ms.clamp(-60000, 60000));
                        }
                    }
                }
            }
            self.legacy.insert(key, decoded);
            count += 1;
        }
        let history = self
            .history
            .iter()
            .map(|h| h.track.clone())
            .collect::<Vec<_>>();
        self.remember_all(history);
        self.migrated = true;
        Ok(count)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LibraryFilter {
    #[default]
    All,
    Local,
    Downloaded,
    Liked,
    Missing,
    Excluded,
}

impl LibraryFilter {
    pub(crate) fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Local,
            2 => Self::Downloaded,
            3 => Self::Liked,
            4 => Self::Missing,
            5 => Self::Excluded,
            _ => Self::All,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LibrarySort {
    #[default]
    Artist,
    Title,
    Album,
    Duration,
    Source,
    RecentlyPlayed,
}

impl LibrarySort {
    pub(crate) fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Title,
            2 => Self::Album,
            3 => Self::Duration,
            4 => Self::Source,
            5 => Self::RecentlyPlayed,
            _ => Self::Artist,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LibraryCategory {
    #[default]
    Songs,
    Albums,
    Artists,
    Playlists,
    Downloads,
}

impl LibraryCategory {
    pub(crate) fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Albums,
            2 => Self::Artists,
            3 => Self::Playlists,
            4 => Self::Downloads,
            _ => Self::Songs,
        }
    }
}

/// A session-facing local album, artist, or playlist projection. It is built
/// from Store v1 data and never persisted independently.
#[derive(Debug, Clone, Default)]
pub(crate) struct LibraryCollection {
    pub kind: String,
    pub key: String,
    pub title: String,
    pub subtitle: String,
    pub tracks: Vec<Track>,
    pub track_count: usize,
    pub representative: Track,
    pub last_played: u64,
}

pub(crate) fn normalize_music_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(crate) fn shuffle_tracks(mut tracks: Vec<Track>) -> Vec<Track> {
    use std::hash::{Hash, Hasher};
    let seed = now_ms();
    tracks.sort_by(|left, right| {
        let score = |track: &Track| {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            seed.hash(&mut hash);
            track.path.hash(&mut hash);
            hash.finish()
        };
        score(left).cmp(&score(right)).then_with(|| left.path.cmp(&right.path))
    });
    tracks
}

/// Session-only controls for the Library route. Selected keys are canonical
/// track identities, so sorting and filtering never change which rows are
/// selected.
#[derive(Debug, Default)]
pub(crate) struct LibraryViewState {
    pub query: String,
    pub filter: LibraryFilter,
    pub sort: LibrarySort,
    pub category: LibraryCategory,
    pub selected: BTreeSet<String>,
}

impl LibraryViewState {
    pub(crate) fn apply(
        &self,
        store: &Store,
        tracks: &[Track],
        mut available_locally: impl FnMut(&Track) -> bool,
    ) -> Vec<Track> {
        let query = self.query.trim().to_lowercase();
        let mut result: Vec<_> = tracks
            .iter()
            .filter(|track| {
                let local = !key_is_remote(&track.path);
                let available = available_locally(track);
                let downloaded = available
                    || store.downloads.contains_key(&track.path)
                    || store.downloads.contains_key(&store.identity_key(track));
                let matches_filter = match self.filter {
                    LibraryFilter::All => true,
                    LibraryFilter::Local => local,
                    LibraryFilter::Downloaded => downloaded,
                    LibraryFilter::Liked => store.is_liked(track),
                    LibraryFilter::Missing => local && !available,
                    LibraryFilter::Excluded => store.is_library_excluded(track),
                };
                if !matches_filter
                    || (self.category == LibraryCategory::Downloads && !downloaded)
                {
                    return false;
                }
                query.is_empty()
                    || track.title.to_lowercase().contains(&query)
                    || track.artist.to_lowercase().contains(&query)
                    || track.album.to_lowercase().contains(&query)
                    || track.path.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        result.sort_by(|left, right| {
            let text_cmp = |a: &str, b: &str| a.to_lowercase().cmp(&b.to_lowercase());
            match self.sort {
                LibrarySort::Artist => text_cmp(&left.artist, &right.artist)
                    .then_with(|| text_cmp(&left.title, &right.title)),
                LibrarySort::Title => text_cmp(&left.title, &right.title)
                    .then_with(|| text_cmp(&left.artist, &right.artist)),
                LibrarySort::Album => text_cmp(&left.album, &right.album)
                    .then_with(|| text_cmp(&left.title, &right.title)),
                LibrarySort::Duration => left
                    .duration_secs
                    .total_cmp(&right.duration_secs)
                    .then_with(|| text_cmp(&left.title, &right.title)),
                LibrarySort::Source => text_cmp(&left.path, &right.path)
                    .then_with(|| text_cmp(&left.title, &right.title)),
                LibrarySort::RecentlyPlayed => {
                    let played_at = |track: &Track| {
                        let identity = store.identity_key(track);
                        store
                            .history
                            .iter()
                            .filter(|entry| {
                                store.identity_key(&entry.track) == identity
                                    || same_track_identity(&entry.track, track)
                            })
                            .map(|entry| entry.played_at)
                            .max()
                            .unwrap_or_default()
                    };
                    played_at(right)
                        .cmp(&played_at(left))
                        .then_with(|| text_cmp(&left.artist, &right.artist))
                        .then_with(|| text_cmp(&left.title, &right.title))
                }
            }
        });
        result
    }

    pub(crate) fn collections(
        &self,
        store: &Store,
        tracks: &[Track],
        mut available_locally: impl FnMut(&Track) -> bool,
    ) -> Vec<LibraryCollection> {
        let query = self.query.trim().to_lowercase();
        let mut groups: Vec<LibraryCollection> = match self.category {
            LibraryCategory::Albums => {
                let mut grouped: BTreeMap<(String, String), Vec<Track>> = BTreeMap::new();
                for track in tracks {
                    let album = track.album.trim();
                    let artist = track.artist.trim();
                    if album.is_empty() || artist.is_empty() || artist.eq_ignore_ascii_case("unknown artist") {
                        continue;
                    }
                    grouped
                        .entry((normalize_music_text(artist), normalize_music_text(album)))
                        .or_default()
                        .push(track.clone());
                }
                grouped
                    .into_iter()
                    .map(|((artist_key, album_key), mut members)| {
                        members.sort_by(|left, right| {
                            normalize_music_text(&left.title)
                                .cmp(&normalize_music_text(&right.title))
                                .then_with(|| left.path.cmp(&right.path))
                        });
                        make_library_collection(
                            store,
                            "album",
                            format!("album:{artist_key}\u{1f}{album_key}"),
                            members[0].album.trim().to_string(),
                            members[0].artist.trim().to_string(),
                            members,
                        )
                    })
                    .collect()
            }
            LibraryCategory::Artists => {
                let mut grouped: BTreeMap<String, Vec<Track>> = BTreeMap::new();
                for track in tracks {
                    let artist = track.artist.trim();
                    let key = normalize_music_text(artist);
                    if key.is_empty() || key == "unknown artist" {
                        continue;
                    }
                    grouped.entry(key).or_default().push(track.clone());
                }
                grouped
                    .into_iter()
                    .map(|(key, members)| {
                        let title = members[0].artist.trim().to_string();
                        make_library_collection(
                            store,
                            "artist",
                            format!("artist:{key}"),
                            title,
                            String::new(),
                            members,
                        )
                    })
                    .collect()
            }
            LibraryCategory::Playlists => store
                .playlists
                .iter()
                .map(|playlist| {
                    let members = store.playlist_tracks(&playlist.id);
                    let representative = members
                        .first()
                        .cloned()
                        .or_else(|| {
                            playlist.tracks.first().map(|key| Track {
                                path: key.clone(),
                                ..Default::default()
                            })
                        })
                        .unwrap_or_default();
                    make_library_collection(
                        store,
                        "playlist",
                        playlist.id.clone(),
                        playlist.name.clone(),
                        String::new(),
                        members,
                    )
                    .with_representative(representative)
                    .with_track_count(playlist.tracks.len())
                })
                .collect(),
            LibraryCategory::Songs | LibraryCategory::Downloads => Vec::new(),
        };

        groups.retain(|group| {
            if !query.is_empty()
                && !group.title.to_lowercase().contains(&query)
                && !group.subtitle.to_lowercase().contains(&query)
                && !group.tracks.iter().any(|track| {
                    track.title.to_lowercase().contains(&query)
                        || track.artist.to_lowercase().contains(&query)
                        || track.album.to_lowercase().contains(&query)
                })
            {
                return false;
            }
            if self.filter == LibraryFilter::All {
                return true;
            }
            !group.tracks.is_empty()
                && group.tracks.iter().any(|track| match self.filter {
                    LibraryFilter::All => true,
                    LibraryFilter::Local => !key_is_remote(&track.path),
                    LibraryFilter::Downloaded => {
                        available_locally(track)
                            || store.downloads.contains_key(&track.path)
                            || store.downloads.contains_key(&store.identity_key(track))
                    }
                    LibraryFilter::Liked => store.is_liked(track),
                    LibraryFilter::Missing => !key_is_remote(&track.path) && !available_locally(track),
                    LibraryFilter::Excluded => store.is_library_excluded(track),
                })
        });

        groups.sort_by(|left, right| match self.sort {
            LibrarySort::RecentlyPlayed => right
                .last_played
                .cmp(&left.last_played)
                .then_with(|| normalize_music_text(&left.title).cmp(&normalize_music_text(&right.title))),
            LibrarySort::Duration => {
                let left_duration: f64 = left.tracks.iter().map(|track| track.duration_secs).sum();
                let right_duration: f64 = right.tracks.iter().map(|track| track.duration_secs).sum();
                left_duration.total_cmp(&right_duration)
                    .then_with(|| normalize_music_text(&left.title).cmp(&normalize_music_text(&right.title)))
            }
            LibrarySort::Source => left
                .representative
                .path
                .cmp(&right.representative.path)
                .then_with(|| normalize_music_text(&left.title).cmp(&normalize_music_text(&right.title))),
            LibrarySort::Artist if self.category == LibraryCategory::Albums => normalize_music_text(&left.subtitle)
                .cmp(&normalize_music_text(&right.subtitle))
                .then_with(|| normalize_music_text(&left.title).cmp(&normalize_music_text(&right.title))),
            _ => normalize_music_text(&left.title).cmp(&normalize_music_text(&right.title))
                .then_with(|| normalize_music_text(&left.subtitle).cmp(&normalize_music_text(&right.subtitle))),
        });
        groups
    }
}

impl LibraryCollection {
    fn with_representative(mut self, representative: Track) -> Self {
        self.representative = representative;
        self
    }

    fn with_track_count(mut self, track_count: usize) -> Self {
        self.track_count = track_count;
        self
    }
}

fn make_library_collection(
    store: &Store,
    kind: &str,
    key: String,
    title: String,
    subtitle: String,
    tracks: Vec<Track>,
) -> LibraryCollection {
    let last_played = tracks
        .iter()
        .flat_map(|track| {
            store
                .history
                .iter()
                .filter(move |entry| same_track_identity(&entry.track, track))
                .map(|entry| entry.played_at)
        })
        .max()
        .unwrap_or_default();
    LibraryCollection {
        kind: kind.into(),
        key,
        title,
        subtitle,
        representative: tracks.first().cloned().unwrap_or_default(),
        track_count: tracks.len(),
        tracks,
        last_played,
    }
}
#[derive(Debug, Default)]
pub struct Queue {
    pub tracks: Vec<Track>,
    pub index: Option<usize>,
    pub shuffle: bool,
    pub repeat: String,
    step: usize,
}
impl Queue {
    pub fn select(&mut self, tracks: Vec<Track>, index: usize) -> Option<Track> {
        if index >= tracks.len() {
            return None;
        }
        self.tracks = tracks;
        self.index = Some(index);
        self.tracks.get(index).cloned()
    }
    pub fn append(&mut self, tracks: impl IntoIterator<Item = Track>) {
        self.tracks.extend(tracks);
    }
    pub fn insert_next(&mut self, tracks: impl IntoIterator<Item = Track>) {
        let items: Vec<_> = tracks.into_iter().collect();
        if items.is_empty() {
            return;
        }
        let insertion = self.index.map_or(0, |current| current.saturating_add(1).min(self.tracks.len()));
        self.tracks.splice(insertion..insertion, items);
    }
    pub fn remove(&mut self, index: usize) -> Option<Track> {
        if index >= self.tracks.len() || Some(index) == self.index {
            return None;
        }
        let removed = self.tracks.remove(index);
        if let Some(current) = self.index.as_mut() {
            if index < *current {
                *current -= 1;
            }
        }
        Some(removed)
    }
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tracks.len() || to >= self.tracks.len() || from == to {
            return false;
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        if let Some(current) = self.index.as_mut() {
            *current = if *current == from {
                to
            } else if from < *current && to >= *current {
                *current - 1
            } else if from > *current && to <= *current {
                *current + 1
            } else {
                *current
            };
        }
        true
    }
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.index = None;
        self.step = 0;
    }
    pub fn advance(&mut self, backwards: bool, automatic: bool) -> Option<Track> {
        let n = self.tracks.len();
        let current = self.index?;
        if n == 0 {
            return None;
        }
        let next = if automatic && self.repeat == "one" {
            current
        } else if backwards {
            if current == 0 {
                n - 1
            } else {
                current - 1
            }
        } else if self.shuffle && n > 1 {
            self.step = self.step.wrapping_add(1);
            (current + 1 + (now_ms() as usize ^ self.step) % (n - 1)) % n
        } else if current + 1 < n {
            current + 1
        } else if !automatic || self.repeat == "all" {
            0
        } else {
            return None;
        };
        self.index = Some(next);
        self.tracks.get(next).cloned()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batched_catalog_merge_matches_sequential_updates_and_serializes_identically() {
        let make = |path: &str, title: &str| Track {
            path: path.into(),
            title: title.into(),
            ..Default::default()
        };
        let initial = vec![
            make("a", "old"),
            make("b", "keep"),
            make("a", "legacy duplicate"),
        ];
        let updates = vec![make("c", "first"), make("a", "updated"), make("c", "last")];
        let mut sequential = Store {
            tracks: initial.clone(),
            ..Default::default()
        };
        for track in updates.clone() {
            sequential.remember(track);
        }
        let mut batched = Store {
            tracks: initial,
            ..Default::default()
        };
        batched.remember_all(updates);
        assert_eq!(
            serde_json::to_value(&batched).unwrap(),
            serde_json::to_value(&sequential).unwrap()
        );
    }
    #[test]
    fn indexed_playlist_preserves_order_repeats_and_first_catalog_match() {
        let make = |path: &str, title: &str| Track {
            path: path.into(),
            title: title.into(),
            ..Default::default()
        };
        let store = Store {
            tracks: vec![
                make("a", "first"),
                make("b", "second"),
                make("a", "duplicate"),
            ],
            playlists: vec![Playlist {
                id: "p".into(),
                name: "Test".into(),
                tracks: ["b", "missing", "a", "b"].map(str::to_string).to_vec(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let titles: Vec<_> = store
            .playlist_tracks("p")
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert_eq!(titles, ["second", "first", "second"]);
        assert!(store.playlist_tracks("missing").is_empty());
    }

    #[test]
    fn playlist_resolves_canonical_identity_to_local_track() {
        let local = Track {
            path: "C:/Music/song.flac".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Same title".into(),
            ..Default::default()
        };
        let store = Store {
            tracks: vec![local.clone()],
            playlists: vec![Playlist {
                id: "p".into(),
                tracks: vec!["yt://abcdefghijk".into()],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(store.playlist_tracks("p"), [local]);
    }

    #[test]
    fn library_view_filters_and_sorts_without_collapsing_distinct_files() {
        let tracks = vec![
            Track { path: "a-present.mp3".into(), title: "Same".into(), artist: "Zulu".into(), album: "Beta".into(), duration_secs: 240.0, ..Default::default() },
            Track { path: "b-missing.mp3".into(), title: "Same".into(), artist: "Alpha".into(), album: "Alpha".into(), duration_secs: 120.0, ..Default::default() },
            Track { path: "yt://abcdefghijk".into(), title: "Remote".into(), artist: "Zulu".into(), album: "Gamma".into(), duration_secs: 180.0, video_id: Some("abcdefghijk".into()), ..Default::default() },
        ];
        let mut store = Store { tracks: tracks.clone(), ..Default::default() };
        let view = LibraryViewState { sort: LibrarySort::Artist, ..Default::default() };
        let sorted = view.apply(&store, &tracks, |track| track.path.contains("present"));
        assert_eq!(sorted.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["b-missing.mp3", "yt://abcdefghijk", "a-present.mp3"]);

        let by_title = LibraryViewState { sort: LibrarySort::Title, ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(by_title.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["yt://abcdefghijk", "b-missing.mp3", "a-present.mp3"]);
        let by_album = LibraryViewState { sort: LibrarySort::Album, ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(by_album.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["b-missing.mp3", "a-present.mp3", "yt://abcdefghijk"]);
        let by_duration = LibraryViewState { sort: LibrarySort::Duration, ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(by_duration.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["b-missing.mp3", "yt://abcdefghijk", "a-present.mp3"]);
        let by_source = LibraryViewState { sort: LibrarySort::Source, ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(by_source.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["a-present.mp3", "b-missing.mp3", "yt://abcdefghijk"]);
        let matching_query = LibraryViewState { query: "same".into(), ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(matching_query.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["b-missing.mp3", "a-present.mp3"]);

        let missing = LibraryViewState { filter: LibraryFilter::Missing, ..Default::default() }
            .apply(&store, &tracks, |track| track.path.contains("present"));
        assert_eq!(missing.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["b-missing.mp3"]);
        let local = LibraryViewState { filter: LibraryFilter::Local, ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(local.len(), 2);
        store.likes.insert("yt://abcdefghijk".into());
        let liked = LibraryViewState { filter: LibraryFilter::Liked, ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(liked.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["yt://abcdefghijk"]);
        store.downloads.insert("yt://abcdefghijk".into(), "C:/download/remote.m4a".into());
        let downloaded = LibraryViewState { filter: LibraryFilter::Downloaded, ..Default::default() }
            .apply(&store, &tracks, |_| false);
        assert_eq!(downloaded.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["yt://abcdefghijk"]);

        store.set_library_excluded(&tracks[0], true);
        let excluded = store.excluded_library_tracks();
        assert_eq!(excluded.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["a-present.mp3"]);
        assert_eq!(store.library_tracks().len(), 2);
        assert!(LibraryViewState::default().apply(&store, &[], |_| false).is_empty());
    }

    #[test]
    fn library_collections_normalize_metadata_and_keep_ambiguous_tracks_in_songs() {
        let tracks = vec![
            Track { path: "one.flac".into(), title: "One".into(), artist: "The Artist".into(), album: "Blue Room".into(), duration_secs: 120.0, ..Default::default() },
            Track { path: "two.flac".into(), title: "Two".into(), artist: " the   ARTIST ".into(), album: " blue room ".into(), duration_secs: 180.0, ..Default::default() },
            Track { path: "three.flac".into(), title: "Three".into(), artist: "Other Artist".into(), album: "Blue Room".into(), duration_secs: 200.0, ..Default::default() },
            Track { path: "untagged.flac".into(), title: "Unknown".into(), artist: "".into(), album: "".into(), ..Default::default() },
        ];
        let store = Store { tracks: tracks.clone(), ..Default::default() };
        let albums = LibraryViewState { category: LibraryCategory::Albums, ..Default::default() }
            .collections(&store, &tracks, |_| false);
        assert_eq!(albums.len(), 2, "same album title by different artists stays separate");
        assert!(albums.iter().any(|album| album.tracks.len() == 2));
        assert!(albums.iter().any(|album| album.tracks.len() == 1));
        assert!(albums.iter().all(|album| album.kind == "album"));

        let artists = LibraryViewState { category: LibraryCategory::Artists, ..Default::default() }
            .collections(&store, &tracks, |_| false);
        assert_eq!(artists.len(), 2);
        assert!(artists.iter().any(|artist| artist.tracks.len() == 2));
        assert!(LibraryViewState { category: LibraryCategory::Albums, ..Default::default() }
            .collections(&store, &tracks, |_| false)
            .iter()
            .all(|album| !album.tracks.iter().any(|track| track.path == "untagged.flac")));
    }

    #[test]
    fn recently_played_sort_and_download_category_use_existing_store_data() {
        let tracks = vec![
            Track { path: "older.mp3".into(), title: "Older".into(), artist: "A".into(), ..Default::default() },
            Track { path: "newer.mp3".into(), title: "Newer".into(), artist: "B".into(), ..Default::default() },
        ];
        let store = Store {
            tracks: tracks.clone(),
            history: vec![
                History { track: tracks[0].clone(), played_at: 10, ..Default::default() },
                History { track: tracks[1].clone(), played_at: 20, ..Default::default() },
            ],
            ..Default::default()
        };
        let recently_played = LibraryViewState { sort: LibrarySort::RecentlyPlayed, ..Default::default() }
            .apply(&store, &tracks, |_| true);
        assert_eq!(recently_played[0].path, "newer.mp3");

        let downloads = LibraryViewState { category: LibraryCategory::Downloads, ..Default::default() }
            .apply(&store, &tracks, |track| track.path == "older.mp3");
        assert_eq!(downloads.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), ["older.mp3"]);
        assert!(LibraryViewState { category: LibraryCategory::Downloads, ..Default::default() }
            .apply(&store, &[], |_| true)
            .is_empty());
    }

    #[test]
    fn relinking_updates_saved_references_without_touching_the_file() {
        let old = Track { path: "C:/gone/old.mp3".into(), title: "Old title".into(), ..Default::default() };
        let replacement = Track { path: "D:/Music/new.mp3".into(), title: "New title".into(), ..Default::default() };
        let mut store = Store {
            tracks: vec![old.clone()],
            likes: [old.path.clone()].into_iter().collect(),
            playlists: vec![Playlist { id: "p".into(), tracks: vec![old.path.clone(), old.path.clone()], ..Default::default() }],
            history: vec![History { track: old.clone(), ..Default::default() }],
            downloads: [("yt://abcdefghijk".into(), old.path.clone())].into_iter().collect(),
            ..Default::default()
        };
        assert!(store.relink_local_path(&old.path, replacement.clone()));
        assert_eq!(store.tracks[0].path, replacement.path);
        assert!(store.likes.contains("yt://abcdefghijk"));
        assert_eq!(store.playlists[0].tracks, ["yt://abcdefghijk", "yt://abcdefghijk"]);
        assert_eq!(store.history[0].track.path, replacement.path);
        assert_eq!(store.downloads["yt://abcdefghijk"], replacement.path);
        assert!(store.is_liked(&replacement));
        assert_eq!(store.playlist_tracks("p"), [replacement.clone(), replacement]);
    }
    #[test]
    fn source_roots_and_dedup() {
        assert_eq!(normalize_path(" C:\\\\ ".trim()), "C:/");
        assert_eq!(normalize_path("//server//share/"), "//server/share");
        assert_eq!(
            normalize_sources(vec![
                Source {
                    id: "1".into(),
                    kind: "folder".into(),
                    path: "C:\\Music\\".into()
                },
                Source {
                    id: "2".into(),
                    kind: "folder".into(),
                    path: "c:/music".into()
                }
            ])
            .len(),
            1
        );
    }
    #[test]
    fn migration_never_copies_credentials_and_only_runs_once() {
        let mut s = Store::default();
        let n=s.migrate(r#"{"pvf:liked":"[\"a\"]","pvf:cfapi:jwt":"secret","SAPISID":"secret","pvf:lyrics:offset:a":"-250"}"#).unwrap();
        assert_eq!(n, 2);
        assert!(s.likes.contains("a"));
        assert_eq!(s.offsets["a"], -250);
        assert!(!serde_json::to_string(&s).unwrap().contains("secret"));
        assert!(s.migrate("{}").is_err());
    }
    #[test]
    fn track_clip_identity_is_optional_and_backward_compatible() {
        let legacy: Track =
            serde_json::from_str(r#"{"path":"yt://audio-id","title":"Song","videoId":"audio-id"}"#)
                .unwrap();
        assert_eq!(legacy.video_id.as_deref(), Some("audio-id"));
        assert_eq!(legacy.clip_video_id, None);

        let paired = Track {
            clip_video_id: Some("clip-id".into()),
            ..legacy
        };
        let restored: Track =
            serde_json::from_str(&serde_json::to_string(&paired).unwrap()).unwrap();
        assert_eq!(restored.video_id.as_deref(), Some("audio-id"));
        assert_eq!(restored.clip_video_id.as_deref(), Some("clip-id"));
    }

    #[test]
    fn library_keeps_local_files_and_saved_remote_songs_only() {
        let path = std::env::temp_dir().join(format!(
            "vey-library-{}-{}.mp3",
            std::process::id(),
            now_ms()
        ));
        std::fs::write(&path, b"audio").unwrap();
        let local_path = path.to_string_lossy().to_string();
        let remote = |id: &str, title: &str| Track {
            path: format!("yt://{id}"),
            video_id: Some(id.into()),
            title: title.into(),
            ..Default::default()
        };
        let mut store = Store {
            tracks: vec![
                remote("aaaaaaaaaaa", "Saved"),
                remote("bbbbbbbbbbb", "Search result"),
                remote("ccccccccccc", "Remote duplicate"),
                Track {
                    path: local_path.clone(),
                    video_id: Some("ccccccccccc".into()),
                    title: "Local file".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        store.likes.insert("yt://aaaaaaaaaaa".into());
        store
            .downloads
            .insert("yt://ccccccccccc".into(), local_path.clone());
        let titles: Vec<_> = store
            .library_tracks()
            .into_iter()
            .map(|track| track.title)
            .collect();
        assert_eq!(titles, ["Local file", "Saved"]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn identities_migrate_from_local_paths_to_video_keys() {
        let local = Track {
            path: "C:/Music/song.mp3".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Song".into(),
            ..Default::default()
        };
        let mut store = Store {
            tracks: vec![local.clone()],
            ..Default::default()
        };
        store.likes.insert(local.path.clone());
        store.playlists.push(Playlist {
            id: "p".into(),
            name: "Mix".into(),
            tracks: vec![local.path.clone(), local.path.clone()],
            ..Default::default()
        });
        store.offsets.insert(local.path.clone(), 250);
        store.lyrics_choices.insert(
            local.path.clone(),
            LyricsChoice {
                provider: "auto".into(),
                format: "lrc".into(),
                sync_type: crate::lyrics::LyricsSyncType::Synced,
                fingerprint: "x".into(),
            },
        );
        store.migrate_identities();
        assert_eq!(store.likes.iter().collect::<Vec<_>>(), ["yt://abcdefghijk"]);
        assert_eq!(store.playlists[0].tracks, ["yt://abcdefghijk", "yt://abcdefghijk"]);
        assert_eq!(store.offsets["yt://abcdefghijk"], 250);
        assert!(store.lyrics_choices.contains_key("yt://abcdefghijk"));
        assert!(store.is_liked(&local));
        store.toggle_like_track(&local);
        assert!(!store.is_liked(&local));
    }

    #[test]
    fn queue_repeat_and_end() {
        let mut q = Queue::default();
        let t = Track {
            path: "a".into(),
            ..Default::default()
        };
        assert!(q.select(vec![t], 2).is_none());
        q.select(vec![Track::default(), Track::default()], 0);
        assert!(q.advance(false, true).is_some());
        assert!(q.advance(false, true).is_none());
        q.repeat = "all".into();
        assert!(q.advance(false, true).is_some());
        assert_eq!(q.index, Some(0));
    }

    #[test]
    fn queue_insert_remove_and_reorder_keep_the_current_track_selected() {
        let track = |path: &str| Track { path: path.into(), ..Default::default() };
        let mut queue = Queue::default();
        queue.append([track("a"), track("b")]);
        assert_eq!(queue.index, None);
        queue.insert_next([track("c")]);
        assert_eq!(queue.tracks.iter().map(|item| item.path.as_str()).collect::<Vec<_>>(), ["c", "a", "b"]);
        assert_eq!(queue.select(queue.tracks.clone(), 1).unwrap().path, "a");
        assert!(queue.remove(1).is_none());
        assert_eq!(queue.remove(0).unwrap().path, "c");
        assert_eq!(queue.index, Some(0));
        assert!(queue.move_item(1, 0));
        assert_eq!(queue.index, Some(1));
        assert_eq!(queue.tracks[1].path, "a");
        queue.clear();
        assert!(queue.tracks.is_empty());
        assert_eq!(queue.index, None);
    }

    #[test]
    fn playlist_rename_and_reorder_preserve_repeated_entries() {
        let mut store = Store {
            playlists: vec![Playlist {
                id: "p".into(),
                name: "Old name".into(),
                tracks: ["a", "b", "a"].map(str::to_string).to_vec(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(store.rename_playlist("p", "  New name  "));
        assert_eq!(store.playlists[0].name, "New name");
        assert!(!store.rename_playlist("p", "  "));
        assert!(store.move_playlist_track("p", 2, 0));
        assert_eq!(store.playlists[0].tracks, ["a", "a", "b"]);
        assert!(!store.move_playlist_track("p", 3, 0));
    }

    #[test]
    fn store_v1_without_ephemeral_library_fields_still_loads() {
        let store: Store = serde_json::from_str(r#"{"version":1,"tracks":[],"likes":[],"playlists":[]}"#).unwrap();
        assert_eq!(store.version, 1);
        assert!(store.library_excluded.is_empty());
    }

    #[test]
    fn history_compacts_events_while_playlists_preserve_repeats() {
        let mut s = Store::default();
        let t = Track {
            path: "a".into(),
            title: "Known song".into(),
            artist: "Known artist".into(),
            duration_secs: 180.0,
            ..Default::default()
        };
        s.record(&t);
        s.record(&t);
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.history[0].play_count, 2);
        let id = s.create_playlist("Test").unwrap();
        s.playlist_add(&id, "a");
        s.playlist_add(&id, "a");
        assert_eq!(s.playlists[0].tracks.len(), 2);
    }

    #[test]
    fn canonical_track_dedup_prefers_id_and_local_path() {
        let local = Track {
            path: "C:/music/song.mp3".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Song".into(),
            artist: "Artist".into(),
            duration_secs: 180.0,
            ..Default::default()
        };
        let remote = Track {
            path: "yt://abcdefghijk".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Song (Official Audio)".into(),
            thumbnail_url: "https://example.test/cover.jpg".into(),
            ..Default::default()
        };
        let unique = deduplicate_tracks([local.clone(), remote]);
        assert_eq!(unique.len(), 1);
        assert_eq!(unique[0].path, local.path);
        assert_eq!(unique[0].thumbnail_url, "https://example.test/cover.jpg");
        assert_eq!(unique[0].video_id.as_deref(), Some("abcdefghijk"));
    }

    #[test]
    fn metadata_fallback_pairs_local_and_web_when_one_id_is_missing() {
        let local = Track {
            path: "C:/music/song.mp3".into(),
            title: "A Song!".into(),
            artist: "The Artist".into(),
            duration_secs: 180.0,
            ..Default::default()
        };
        let matching = Track {
            path: "search-result".into(),
            video_id: Some("abcdefghijk".into()),
            title: "a-song".into(),
            artist: "the artist".into(),
            duration_secs: 185.0,
            ..Default::default()
        };
        let merged = deduplicate_tracks([local.clone(), matching]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].path, local.path);
        assert_eq!(merged[0].video_id.as_deref(), Some("abcdefghijk"));
        assert_eq!(merged[0].duration_secs, 180.0);

        let no_id_copy = Track {
            path: "C:/music/song-copy.mp3".into(),
            ..local.clone()
        };
        assert_eq!(deduplicate_tracks([local.clone(), no_id_copy]).len(), 1);

        let outside_tolerance = Track {
            path: "search-result".into(),
            video_id: Some("abcdefghijk".into()),
            title: "a-song".into(),
            artist: "the artist".into(),
            duration_secs: 186.0,
            ..Default::default()
        };
        assert_eq!(deduplicate_tracks([local.clone(), outside_tolerance]).len(), 2);

        let identified_local = Track {
            video_id: Some("abcdefghijk".into()),
            ..local.clone()
        };
        let different_version = Track {
            path: "yt://lmnopqrstuv".into(),
            video_id: Some("lmnopqrstuv".into()),
            title: "A Song!".into(),
            artist: "The Artist".into(),
            duration_secs: 182.0,
            ..Default::default()
        };
        assert_eq!(deduplicate_tracks([identified_local, different_version]).len(), 2);
    }

    #[test]
    fn ambiguous_metadata_matches_are_not_collapsed() {
        let make = |path: &str, duration_secs| Track {
            path: path.into(),
            title: "Same song".into(),
            artist: "Same artist".into(),
            duration_secs,
            ..Default::default()
        };
        let local_a = make("C:/a.mp3", 180.0);
        let local_b = make("C:/b.mp3", 186.0);
        let web = make("yt://abcdefghijk", 183.0);
        assert_eq!(deduplicate_tracks([local_a, local_b, web]).len(), 3);
    }

    #[test]
    fn history_uses_canonical_identity_and_keeps_feedback_counters() {
        let mut store = Store::default();
        let local = Track {
            path: "C:/music/song.mp3".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Song".into(),
            artist: "Artist".into(),
            duration_secs: 180.0,
            ..Default::default()
        };
        let remote = Track {
            path: "yt://abcdefghijk".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Song".into(),
            artist: "Artist".into(),
            duration_secs: 180.0,
            ..Default::default()
        };
        store.record(&local);
        store.record_completion(&local);
        store.record_skip(&local);
        store.record(&remote);
        assert_eq!(store.history.len(), 1);
        assert_eq!(store.history[0].play_count, 2);
        assert_eq!(store.history[0].completion_count, 1);
        assert_eq!(store.history[0].skip_count, 1);
        assert!(store.recommendations.track_weights.contains_key("yt://abcdefghijk"));
    }
}
