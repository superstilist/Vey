//! Portable application data. No window, audio device, Tauri, or account state.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Track {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub video_id: Option<String>,
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
#[serde(default)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct History {
    pub track: Track,
    pub played_at: u64,
    pub play_count: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub volume: f32,
    pub rate: f32,
    pub art_mode: String,
    pub lyrics_provider: String,
    pub lyrics_stage: bool,
    pub lyrics_reduced_motion: bool,
    pub lyrics_passive_scroll: bool,
    pub shuffle: bool,
    pub repeat: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 1.,
            rate: 1.,
            art_mode: "cover".into(),
            lyrics_provider: "auto".into(),
            lyrics_stage: false,
            lyrics_reduced_motion: false,
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
    pub offsets: BTreeMap<String, i64>,
    pub legacy: BTreeMap<String, Value>,
    pub lyrics_cache: BTreeMap<String, CachedLyrics>,
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
            offsets: BTreeMap::new(),
            legacy: BTreeMap::new(),
            lyrics_cache: BTreeMap::new(),
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
    pub fn remember(&mut self, track: Track) {
        if let Some(t) = self.tracks.iter_mut().find(|t| t.path == track.path) {
            *t = track;
        } else {
            self.tracks.push(track);
        }
    }
    pub fn record(&mut self, track: &Track) {
        self.remember(track.clone());
        let count = self
            .history
            .iter()
            .find(|h| h.track.path == track.path)
            .map(|h| h.play_count)
            .unwrap_or(0);
        self.history.retain(|h| h.track.path != track.path);
        self.history.insert(
            0,
            History {
                track: track.clone(),
                played_at: now_ms(),
                play_count: count.saturating_add(1),
            },
        );
        self.history.truncate(100);
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
            CachedLyrics {
                parser_version: 4,
                saved_at: now_ms(),
                alternatives,
            },
        );
    }
    pub fn offset_for(&self, track: &Track) -> i64 {
        let video = track.video_id.as_ref().map(|id| format!("vid:{id}"));
        let tag = format!(
            "tag:{}|{}",
            track.artist.trim().to_lowercase(),
            track.title.trim().to_lowercase()
        );
        self.offsets
            .get(&track.path)
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
        });
        Some(id)
    }
    pub fn playlist_add(&mut self, id: &str, path: &str) {
        if let Some(p) = self.playlists.iter_mut().find(|p| p.id == id) {
            if !p.tracks.iter().any(|t| t == path) {
                p.tracks.push(path.into());
            }
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
        let history = self.history.clone();
        for h in history {
            self.remember(h.track);
        }
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
    fn history_and_playlists_deduplicate() {
        let mut s = Store::default();
        let t = Track {
            path: "a".into(),
            ..Default::default()
        };
        s.record(&t);
        s.record(&t);
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.history[0].play_count, 2);
        let id = s.create_playlist("Test").unwrap();
        s.playlist_add(&id, "a");
        s.playlist_add(&id, "a");
        assert_eq!(s.playlists[0].tracks.len(), 1);
    }
}
