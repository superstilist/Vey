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
    pub offsets: BTreeMap<String, i64>,
    pub legacy: BTreeMap<String, Value>,
    pub lyrics_cache: BTreeMap<String, std::sync::Arc<CachedLyrics>>,
    /// Public track key -> complete permanent local file, never the stream cache.
    pub downloads: BTreeMap<String, String>,
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
            offsets: BTreeMap::new(),
            legacy: BTreeMap::new(),
            lyrics_cache: BTreeMap::new(),
            downloads: BTreeMap::new(),
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
            .filter(|track| !key_is_remote(&track.path))
            .cloned();
        let remote = self
            .tracks
            .iter()
            .filter(|track| {
                track.path.strip_prefix("yt://").is_some_and(|id| {
                    saved.contains(track.path.as_str()) && !downloaded.contains(id)
                })
            })
            .cloned();
        deduplicate_tracks(local.chain(remote))
    }
    /// Resolve in playlist order, including repeats; ignore missing references.
    pub fn playlist_tracks(&self, id: &str) -> Vec<Track> {
        let Some(playlist) = self.playlists.iter().find(|p| p.id == id) else {
            return vec![];
        };
        let mut tracks = std::collections::HashMap::with_capacity(self.tracks.len());
        for track in &self.tracks {
            tracks.entry(track.path.as_str()).or_insert(track);
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
