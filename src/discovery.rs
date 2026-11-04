//! Bounded mixed-media Home feed. Account history stays local; public catalog
//! cards retain their real navigation targets instead of becoming fake songs.
use crate::{
    services::{Store, Track},
    HomeItemRow, HomeShelfRow, MainWindow,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Catalog {
    pub updated_at: u64,
    pub sections: Vec<Section>,
}
#[derive(Serialize, Deserialize)]
pub struct Selection {
    pub tracks: Vec<Track>,
    pub index: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Section {
    pub key: String,
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub items: Vec<Item>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Item {
    pub key: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub video_id: String,
    pub browse_id: String,
    pub thumbnail_url: String,
    pub duration_secs: f64,
}
impl Item {
    pub fn song(track: &Track) -> Self {
        Self {
            key: track.path.clone(),
            kind: "song".into(),
            title: track.title.clone(),
            subtitle: track.artist.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            path: track.path.clone(),
            video_id: crate::services::track_video_id(track)
                .unwrap_or_default()
                .into(),
            thumbnail_url: track.thumbnail_url.clone(),
            duration_secs: track.duration_secs,
            ..Self::default()
        }
    }
    pub fn track(&self) -> Option<Track> {
        ["song", "video"]
            .contains(&self.kind.as_str())
            .then(|| Track {
                path: self.path.clone(),
                title: self.title.clone(),
                artist: self.artist.clone(),
                album: self.album.clone(),
                video_id: crate::services::video_id(&self.video_id).then(|| self.video_id.clone()),
                clip_video_id: (self.kind == "video").then(|| self.video_id.clone()),
                thumbnail_url: self.thumbnail_url.clone(),
                duration_secs: self.duration_secs,
            })
    }
    fn artwork_track(&self) -> Track {
        Track {
            path: self.path.clone(),
            thumbnail_url: self.thumbnail_url.clone(),
            video_id: crate::services::video_id(&self.video_id).then(|| self.video_id.clone()),
            ..Track::default()
        }
    }
    pub fn collection_tracks(&self, mut tracks: Vec<Track>) -> Vec<Track> {
        if self.kind == "album" {
            for track in &mut tracks {
                if track.album.trim().is_empty() {
                    track.album = self.title.clone();
                }
                if track.artist.trim().is_empty() {
                    track.artist = self.artist.clone();
                }
            }
        }
        tracks
    }
    pub fn local_collection(&self, store: &Store) -> Option<Vec<Track>> {
        let tracks = store.library_tracks();
        if self.key.starts_with("local-album:") {
            return Some(tracks.into_iter().filter(|track| {
                crate::services::normalize_music_text(&track.album)
                    == crate::services::normalize_music_text(&self.album)
                    && crate::services::normalize_music_text(&track.artist)
                        == crate::services::normalize_music_text(&self.artist)
            }).collect());
        }
        if let Some(key) = self.key.strip_prefix("local-artist:") {
            return Some(tracks.into_iter().filter(|track| {
                crate::services::normalize_music_text(&track.artist) == key
            }).collect());
        }
        None
    }
}
fn text(value: &Value) -> String {
    value
        .get("simpleText")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            value
                .get("runs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|run| run.get("text").and_then(Value::as_str))
                .collect()
        })
}
fn image(value: &Value) -> (String, f64) {
    fn visit(value: &Value, best: &mut (u64, String, f64)) {
        if let Some(thumbnails) = value.get("thumbnails").and_then(Value::as_array) {
            for thumb in thumbnails {
                let url = thumb.get("url").and_then(Value::as_str).unwrap_or_default();
                let width = thumb.get("width").and_then(Value::as_u64).unwrap_or(1);
                let height = thumb
                    .get("height")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .max(1);
                if url.starts_with("https://") && width.saturating_mul(height) > best.0 {
                    *best = (
                        width.saturating_mul(height),
                        url.into(),
                        width as f64 / height as f64,
                    );
                }
            }
        }
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if key != "thumbnails" {
                        visit(child, best);
                    }
                }
            }
            Value::Array(values) => {
                for child in values {
                    visit(child, best);
                }
            }
            _ => {}
        }
    }
    let mut best = (0, String::new(), 1.0);
    visit(value, &mut best);
    (best.1, best.2)
}
fn endpoint<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    if let Some(found) = value.get(name) {
        return Some(found);
    }
    match value {
        Value::Object(map) => map.values().find_map(|child| endpoint(child, name)),
        Value::Array(values) => values.iter().find_map(|child| endpoint(child, name)),
        _ => None,
    }
}
fn duration(value: &str) -> f64 {
    value
        .split(['•', '·'])
        .map(str::trim)
        .find_map(|part| {
            let fields: Vec<_> = part.split(':').collect();
            if !(2..=3).contains(&fields.len()) {
                return None;
            }
            fields.into_iter().try_fold(0u64, |total, field| {
                field.parse::<u64>().ok().map(|n| total * 60 + n)
            })
        })
        .unwrap_or(0) as f64
}
fn item(value: &Value) -> Option<Item> {
    let row = value
        .get("musicTwoRowItemRenderer")
        .or_else(|| value.get("musicResponsiveListItemRenderer"))?;
    let title = row.get("title").map(text).or_else(|| {
        row.pointer("/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text")
            .map(text)
    })?;
    if title.trim().is_empty() {
        return None;
    }
    let subtitle_value = row
        .get("subtitle")
        .or_else(|| row.pointer("/flexColumns/1/musicResponsiveListItemFlexColumnRenderer/text"));
    let subtitle = subtitle_value.map(text).unwrap_or_default();
    let artist = subtitle_value
        .and_then(|v| v.get("runs"))
        .and_then(Value::as_array)
        .map(|runs| {
            runs.iter()
                .filter(|run| {
                    run.pointer("/navigationEndpoint/browseEndpoint/browseId")
                        .and_then(Value::as_str)
                        .is_some_and(|id| id.starts_with("UC"))
                })
                .filter_map(|run| run.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|artist| !artist.is_empty())
        .unwrap_or_else(|| {
            subtitle
                .split(['•', '·'])
                .next()
                .unwrap_or_default()
                .trim()
                .into()
        });
    let (thumbnail_url, ratio) = image(
        row.get("thumbnailRenderer")
            .or_else(|| row.get("thumbnail"))
            .unwrap_or(row),
    );
    let main_endpoint = row.get("navigationEndpoint").unwrap_or(&Value::Null);
    let browse = endpoint(main_endpoint, "browseEndpoint");
    let mut browse_id = browse
        .and_then(|v| v.get("browseId"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if browse_id.is_empty() {
        if let Some(id) = endpoint(main_endpoint, "watchPlaylistEndpoint")
            .and_then(|v| v.get("playlistId"))
            .and_then(Value::as_str)
        {
            browse_id = format!("VL{id}");
        }
    }
    let page_type = browse
        .and_then(|v| {
            v.pointer(
                "/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType",
            )
        })
        .and_then(Value::as_str)
        .unwrap_or_default();
    let video_id = endpoint(main_endpoint, "watchEndpoint")
        .or_else(|| endpoint(row.get("overlay").unwrap_or(&Value::Null), "watchEndpoint"))
        .and_then(|v| v.get("videoId"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let kind = if !browse_id.is_empty() {
        if browse_id.starts_with("MPRE") || page_type.contains("ALBUM") {
            "album"
        } else if browse_id.starts_with("UC") || page_type.contains("ARTIST") {
            "artist"
        } else if browse_id.starts_with("VL") || page_type.contains("PLAYLIST") {
            "playlist"
        } else {
            return None;
        }
    } else if crate::services::video_id(video_id) {
        if ratio > 1.3 {
            "video"
        } else {
            "song"
        }
    } else {
        return None;
    };
    let key = if browse_id.is_empty() {
        format!("yt://{video_id}")
    } else {
        format!("catalog://{browse_id}")
    };
    let duration_secs = duration(&subtitle).max(
        row.get("fixedColumns")
            .and_then(|v| v.pointer("/0/musicResponsiveListItemFixedColumnRenderer/text"))
            .map(text)
            .map(|v| duration(&v))
            .unwrap_or(0.0),
    );
    Some(Item {
        path: key.clone(),
        key,
        kind: kind.into(),
        title,
        subtitle,
        artist,
        video_id: video_id.into(),
        browse_id: browse_id.into(),
        thumbnail_url,
        duration_secs,
        ..Item::default()
    })
}
/// Preserve browse targets and media shape from the actual Home carousels.
pub fn parse(value: &Value) -> Catalog {
    fn visit(value: &Value, out: &mut Vec<Section>) {
        if out.len() >= 8 {
            return;
        }
        if let Some(shelf) = value
            .get("musicCarouselShelfRenderer")
            .or_else(|| value.get("musicShelfRenderer"))
        {
            let title = shelf
                .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")
                .or_else(|| shelf.get("title"))
                .map(text)
                .unwrap_or_default();
            let subtitle = shelf
                .pointer("/header/musicCarouselShelfBasicHeaderRenderer/strapline")
                .map(text)
                .unwrap_or_default();
            if let Some(rows) = shelf.get("contents").and_then(Value::as_array) {
                let mut groups: Vec<(String, Vec<Item>)> = Vec::new();
                let mut seen = HashSet::new();
                for entry in rows.iter().filter_map(item) {
                    if !seen.insert(entry.key.clone()) {
                        continue;
                    }
                    let index = groups
                        .iter()
                        .position(|(kind, _)| kind == &entry.kind)
                        .unwrap_or_else(|| {
                            groups.push((entry.kind.clone(), vec![]));
                            groups.len() - 1
                        });
                    if groups[index].1.len() < 10 {
                        groups[index].1.push(entry);
                    }
                }
                for (kind, items) in groups {
                    if out.len() < 8 && !items.is_empty() {
                        out.push(Section {
                            key: format!("public-{}-{kind}", out.len()),
                            title: title.clone(),
                            subtitle: subtitle.clone(),
                            kind,
                            items,
                        });
                    }
                }
            }
            return;
        }
        match value {
            Value::Object(map) => {
                for child in map.values() {
                    visit(child, out);
                }
            }
            Value::Array(values) => {
                for child in values {
                    visit(child, out);
                }
            }
            _ => {}
        }
    }
    let mut sections = vec![];
    visit(value, &mut sections);
    Catalog {
        updated_at: crate::services::now_ms(),
        sections,
    }
}
// Home combines history and cached recommendations. Resolve known download
// identities first, then only merge incomplete local/remote metadata when
// exactly one candidate has matching duration or artwork.
fn home_tracks(store: &Store, tracks: impl IntoIterator<Item = Track>) -> Vec<Track> {
    let tracks = tracks.into_iter().map(|mut track| {
        if crate::services::track_video_id(&track).is_none() {
            if let Some(id) = store.identity_key(&track).strip_prefix("yt://") {
                if crate::services::video_id(id) { track.video_id = Some(id.to_owned()); }
            }
        }
        track
    });
    let mut tracks = crate::services::deduplicate_tracks(tracks);
    let matches = |a: &Track, b: &Track| {
        let title = crate::services::normalize_music_text(&a.title);
        let artist_missing = |t: &Track| t.artist.trim().is_empty() || t.artist.eq_ignore_ascii_case("unknown artist");
        let same_id = match (crate::services::track_video_id(a), crate::services::track_video_id(b)) {
            (Some(left), Some(right)) => left == right,
            _ => true,
        };
        let duration = a.duration_secs > 0.0 && b.duration_secs > 0.0
            && (a.duration_secs - b.duration_secs).abs() <= 5.0;
        let art = !a.thumbnail_url.is_empty() && a.thumbnail_url == b.thumbnail_url;
        !title.is_empty() && title == crate::services::normalize_music_text(&b.title)
            && artist_missing(a) != artist_missing(b)
            && crate::services::key_is_remote(&a.path) != crate::services::key_is_remote(&b.path)
            && same_id && (duration || art)
    };
    let mut removed = vec![false; tracks.len()];
    for i in 0..tracks.len() {
        if removed[i] { continue; }
        let candidates: Vec<_> = (0..tracks.len()).filter(|&j| j != i && !removed[j] && matches(&tracks[i], &tracks[j])).collect();
        if candidates.len() != 1 { continue; }
        let j = candidates[0];
        if (0..tracks.len()).filter(|&k| k != j && !removed[k] && matches(&tracks[j], &tracks[k])).count() != 1 { continue; }
        let (first, second) = (i.min(j), i.max(j));
        let mut merged = if tracks[first].artist.trim().is_empty() || tracks[first].artist.eq_ignore_ascii_case("unknown artist") { tracks[second].clone() } else { tracks[first].clone() };
        if merged.thumbnail_url.is_empty() {
            merged.thumbnail_url = if tracks[first].thumbnail_url.is_empty() { tracks[second].thumbnail_url.clone() } else { tracks[first].thumbnail_url.clone() };
        }
        if !crate::services::key_is_remote(&tracks[first].path) { merged.path = tracks[first].path.clone(); }
        else if !crate::services::key_is_remote(&tracks[second].path) { merged.path = tracks[second].path.clone(); }
        if crate::services::track_video_id(&merged).is_none() {
            merged.video_id = crate::services::track_video_id(&tracks[first])
                .or_else(|| crate::services::track_video_id(&tracks[second]))
                .map(str::to_owned);
        }
        tracks[first] = merged;
        removed[second] = true;
    }
    tracks.into_iter().zip(removed).filter_map(|(track, removed)| (!removed).then_some(track)).collect()
}

pub fn compose(store: &Store) -> Vec<Section> {
    let mut sections = vec![];
    let recent = if store.history.is_empty() {
        store.library_tracks()
    } else {
        home_tracks(store, store.history.iter().map(|h| h.track.clone()))
    };
    let mut again = vec![];
    for playlist in store.playlists.iter().rev().take(3) {
        let first = store.tracks.iter().find(|t| {
            playlist
                .tracks
                .iter()
                .any(|key| key == &t.path || key == &store.identity_key(t))
        });
        again.push(Item {
            key: format!("local-playlist:{}", playlist.id),
            kind: "playlist".into(),
            title: playlist.name.clone(),
            subtitle: format!("{} · {}", playlist.author, playlist.tracks.len())
                .trim_start_matches(" · ")
                .into(),
            browse_id: playlist.id.clone(),
            path: first
                .map(|t| t.path.clone())
                .unwrap_or_else(|| format!("playlist-art:{}", playlist.id)),
            video_id: first
                .and_then(crate::services::track_video_id)
                .unwrap_or_default()
                .into(),
            thumbnail_url: first.map(|t| t.thumbnail_url.clone()).unwrap_or_default(),
            ..Item::default()
        });
    }
    again.extend(recent.iter().take(10).map(Item::song));
    again.truncate(12);
    if !again.is_empty() {
        sections.push(Section {
            key: "again".into(),
            title: "Послухати ще раз".into(),
            subtitle: "ВАША МУЗИКА".into(),
            kind: "mixed".into(),
            items: again,
        });
    }
    if !store.recommendations.tracks.is_empty() {
        sections.push(Section {
            key: "recommendations".into(),
            title: "Щоденні рекомендації".into(),
            subtitle: "ДЛЯ ВАС".into(),
            kind: "song".into(),
            items: home_tracks(store, store
                .recommendations
                .tracks
                .iter()
                .cloned())
                .iter()
                .take(12)
                .map(Item::song)
                .collect(),
        });
    }
    let mut public = store.home_catalog.sections.clone();
    let library = store.library_tracks();
    if !public.iter().any(|s| s.kind == "album") {
        let mut seen = HashSet::new();
        let items: Vec<_> = library
            .iter()
            .filter(|track| {
                !track.album.trim().is_empty()
                    && !track.artist.trim().is_empty()
                    && !track.artist.eq_ignore_ascii_case("unknown artist")
                    && seen.insert((
                        crate::services::normalize_music_text(&track.artist),
                        crate::services::normalize_music_text(&track.album),
                    ))
            })
            .take(10)
            .map(|track| {
                let mut item = Item::song(track);
                item.kind = "album".into();
                item.key = format!(
                    "local-album:{}",
                    serde_json::to_string(&(
                        crate::services::normalize_music_text(&track.artist),
                        crate::services::normalize_music_text(&track.album),
                    ))
                    .unwrap_or_default()
                );
                item.title = track.album.clone();
                item.subtitle = track.artist.clone();
                item
            })
            .collect();
        if !items.is_empty() {
            public.push(Section {
                key: "library-albums".into(),
                title: "Альбоми".into(),
                subtitle: "ВАША МУЗИКА".into(),
                kind: "album".into(),
                items,
                ..Section::default()
            });
        }
    }
    if !public.iter().any(|s| s.kind == "artist") {
        let mut seen = HashSet::new();
        let items: Vec<_> = library
            .iter()
            .filter(|track| {
                !track.artist.trim().is_empty()
                    && !track.artist.eq_ignore_ascii_case("unknown artist")
                    && seen.insert(crate::services::normalize_music_text(&track.artist))
            })
            .take(10)
            .map(|track| {
                let mut item = Item::song(track);
                item.kind = "artist".into();
                item.key = format!("local-artist:{}", crate::services::normalize_music_text(&track.artist));
                item.title = track.artist.clone();
                item.subtitle = String::new();
                item
            })
            .collect();
        if !items.is_empty() {
            public.push(Section {
                key: "library-artists".into(),
                title: "Виконавці".into(),
                kind: "artist".into(),
                items,
                ..Section::default()
            });
        }
    }
    if !public.iter().any(|section| section.kind == "video") {
        let mut seen_videos = HashSet::new();
        let videos: Vec<_> = store
            .recommendations
            .tracks
            .iter()
            .chain(recent.iter())
            .filter_map(|track| {
                let id = track
                    .clip_video_id
                    .as_deref()
                    .filter(|id| crate::services::video_id(id))?;
                if !seen_videos.insert(id.to_owned()) {
                    return None;
                }
                let mut video = Item::song(track);
                video.key = format!("yt://{id}");
                video.path = video.key.clone();
                video.video_id = id.into();
                video.kind = "video".into();
                video.thumbnail_url = format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg");
                Some(video)
            })
            .take(10)
            .collect();
        if !videos.is_empty() {
            public.insert(
                0,
                Section {
                    key: "videos".into(),
                    title: "Музичні відео для вас".into(),
                    kind: "video".into(),
                    items: videos,
                    ..Section::default()
                },
            );
        }
    }
    sections.extend(public.into_iter().filter(|s| !s.items.is_empty()).take(11));
    let quick = home_tracks(store,
        recent
            .into_iter()
            .chain(store.recommendations.tracks.iter().cloned()),
    );
    if !quick.is_empty() {
        sections.push(Section {
            key: "quick".into(),
            title: "Швидкий вибір".into(),
            kind: "quick".into(),
            items: quick.iter().take(12).map(Item::song).collect(),
            ..Section::default()
        });
    }
    match store.settings.home_layout {
        2 => sections.sort_by_key(|s| s.kind != "quick"),
        3 => sections.sort_by_key(|s| s.key == "again" || s.key == "recommendations"),
        4 => sections.sort_by_key(|s| s.kind != "playlist"),
        5 => sections.sort_by_key(|s| s.kind != "artist"),
        _ => {}
    }
    sections
}
pub fn update_art(w: &MainWindow, path: &str, art: &slint::Image) {
    for shelf in w.get_home_shelves().iter() {
        if let Some(items) = shelf.items.as_any().downcast_ref::<VecModel<HomeItemRow>>() {
            for index in 0..items.row_count() {
                let Some(mut row) = items.row_data(index) else {
                    continue;
                };
                if row.art_key == path && row.art != *art {
                    row.art = art.clone();
                    items.set_row_data(index, row);
                }
            }
        }
    }
}
fn sync(w: &MainWindow, store: &Store, signature: &RefCell<String>) {
    let sections = compose(store);
    let next = serde_json::to_string(&sections).unwrap_or_default();
    if *signature.borrow() == next {
        request_art(w, &sections);
        return;
    }
    *signature.borrow_mut() = next;
    let previous: HashMap<String, slint::Image> = w
        .get_home_shelves()
        .iter()
        .flat_map(|s| {
            s.items
                .iter()
                .map(|item| (item.art_key.to_string(), item.art))
                .collect::<Vec<_>>()
        })
        .collect();
    let rows: Vec<_> = sections
        .iter()
        .map(|s| HomeShelfRow {
            key: s.key.clone().into(),
            title: s.title.clone().into(),
            subtitle: s.subtitle.clone().into(),
            kind: s.kind.clone().into(),
            items: Rc::new(VecModel::from(
                s.items
                    .iter()
                    .map(|item| HomeItemRow {
                        key: item.key.clone().into(),
                        art_key: item.path.clone().into(),
                        title: item.title.clone().into(),
                        subtitle: item.subtitle.clone().into(),
                        kind: item.kind.clone().into(),
                        duration: if item.duration_secs > 0.0 {
                            format!(
                                "{}:{:02}",
                                item.duration_secs as u64 / 60,
                                item.duration_secs as u64 % 60
                            )
                            .into()
                        } else {
                            "".into()
                        },
                        art: previous.get(&item.path).cloned().unwrap_or_default(),
                    })
                    .collect::<Vec<_>>(),
            ))
            .into(),
        })
        .collect();
    w.set_home_shelves(Rc::new(VecModel::from(rows)).into());
    let filters: Vec<slint::SharedString> = std::iter::once("all")
        .chain(
            ["video", "album", "artist", "playlist"]
                .into_iter()
                .filter(|kind| sections.iter().any(|s| s.kind == *kind)),
        )
        .map(Into::into)
        .collect();
    w.set_home_filters(Rc::new(VecModel::from(filters)).into());
    request_art(w, &sections);
}
fn request_art(w: &MainWindow, sections: &[Section]) {
    if w.get_active_view() != "home" {
        return;
    }
    let mut requested = HashSet::new();
    for section in sections {
        for item in &section.items {
            // The same recording may appear on multiple shelves. Send one
            // request to the shared cache/queue per identity during this pass.
            if !requested.insert(item.path.as_str()) {
                continue;
            }
            if let Ok(track) = serde_json::to_string(&item.artwork_track()) {
                w.invoke_request_discovery_art(track.into());
            }
        }
    }
}
fn fetch(query: Option<String>, tx: mpsc::Sender<Result<Catalog, String>>) {
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())
            .and_then(|rt| rt.block_on(crate::native::ytm_home_catalog(query)));
        let _ = tx.send(result);
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let origin = web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default();
        let result = async {
            let response = reqwest::Client::new()
                .get(format!("{origin}/api/home"))
                .query(&[("q", query.unwrap_or_default())])
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !response.status().is_success() {
                return Err("Home catalog unavailable".into());
            }
            response.json::<Catalog>().await.map_err(|e| e.to_string())
        }
        .await;
        let _ = tx.send(result);
    });
}
pub fn install(w: &MainWindow, get: Rc<dyn Fn() -> Store>, apply: Rc<dyn Fn(Store)>) {
    let signature = Rc::new(RefCell::new(String::new()));
    let weak = w.as_weak();
    let current = get.clone();
    w.on_sync_home(move || {
        if let Some(w) = weak.upgrade() {
            sync(&w, &current(), &signature);
        }
    });
    let weak = w.as_weak();
    let current = get.clone();
    w.on_open_home_item(move |shelf_key, key| {
        let Some(w) = weak.upgrade() else {
            return;
        };
        let store = current();
        let Some(section) = compose(&store)
            .into_iter()
            .find(|s| s.key == shelf_key.as_str())
        else {
            return;
        };
        let Some(item) = section.items.iter().find(|item| item.key == key.as_str()) else {
            return;
        };
        if item.track().is_some() {
            let tracks: Vec<_> = section.items.iter().filter_map(Item::track).collect();
            let index = tracks
                .iter()
                .position(|track| track.path == item.path)
                .unwrap_or(0);
            w.invoke_play_discovery_items(
                serde_json::to_string(&Selection { tracks, index })
                    .unwrap_or_default()
                    .into(),
                item.kind == "video",
            );
        } else if let Some(id) = item.key.strip_prefix("local-playlist:") {
            w.invoke_open_playlist(id.into());
        } else if item.key.starts_with("local-artist:") {
            w.invoke_open_discovery_collection(
                "".into(),
                serde_json::to_string(item).unwrap_or_default().into(),
            );
        } else if item.kind == "artist" {
            w.invoke_search(item.title.clone().into());
        } else {
            w.invoke_open_discovery_collection(
                item.browse_id.clone().into(),
                serde_json::to_string(item).unwrap_or_default().into(),
            );
        }
    });
    let weak = w.as_weak();
    let current = get.clone();
    w.on_play_home_shelf(move |key| {
        let Some(w) = weak.upgrade() else {
            return;
        };
        if let Some(section) = compose(&current())
            .into_iter()
            .find(|s| s.key == key.as_str())
        {
            let tracks: Vec<_> = section.items.iter().filter_map(Item::track).collect();
            if !tracks.is_empty() {
                w.invoke_play_discovery_items(
                    serde_json::to_string(&Selection { tracks, index: 0 })
                        .unwrap_or_default()
                        .into(),
                    section.kind == "video",
                );
            }
        }
    });
    let (tx, rx) = mpsc::channel();
    let timer = Rc::new(slint::Timer::default());
    let keep_timer = timer.clone();
    let weak = w.as_weak();
    let current = get.clone();
    w.on_request_home_catalog(move || {
        let _ = &keep_timer;
        let Some(w) = weak.upgrade() else {
            return;
        };
        if w.get_home_catalog_loading() {
            return;
        }
        w.set_home_catalog_loading(true);
        w.set_home_catalog_failed(false);
        let store = current();
        let query = store
            .history
            .first()
            .map(|h| h.track.artist.clone())
            .filter(|q| !q.is_empty());
        fetch(query, tx.clone());
    });
    let weak = w.as_weak();
    let current = get.clone();
    let last_check = Rc::new(RefCell::new(web_time::Instant::now()));
    let revision = Rc::new(RefCell::new(-1));
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
            if let Ok(result) = rx.try_recv() {
                w.set_home_catalog_loading(false);
                match result {
                    Ok(catalog) if !catalog.sections.is_empty() => {
                        let mut store = current();
                        store.remember_all(
                            catalog
                                .sections
                                .iter()
                                .flat_map(|s| s.items.iter())
                                .filter_map(Item::track),
                        );
                        store.home_catalog = catalog;
                        apply(store);
                    }
                    _ => w.set_home_catalog_failed(true),
                }
            }
            if *revision.borrow() != w.get_home_revision() {
                *revision.borrow_mut() = w.get_home_revision();
                w.invoke_sync_home();
            }
            if last_check.borrow().elapsed() > Duration::from_secs(60) {
                *last_check.borrow_mut() = web_time::Instant::now();
                if w.get_active_view() == "home"
                    && current().home_catalog.updated_at.saturating_add(3_600_000)
                        < crate::services::now_ms()
                {
                    w.invoke_request_home_catalog();
                }
            }
        },
    );
    w.invoke_sync_home();
    if get().home_catalog.updated_at.saturating_add(3_600_000) < crate::services::now_ms() {
        w.invoke_request_home_catalog();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_album_fallback_opens_only_its_tracks_and_keeps_existing_metadata() {
        let mut store = Store::default();
        store.tracks = vec![
            Track {
                path: "a.mp3".into(),
                title: "First".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                ..Default::default()
            },
            Track {
                path: "b.mp3".into(),
                title: "Second".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                ..Default::default()
            },
            Track {
                path: "c.mp3".into(),
                title: "Other".into(),
                artist: "Other artist".into(),
                album: "Other album".into(),
                ..Default::default()
            },
        ];
        let sections = compose(&store);
        let album = sections.iter().find(|s| s.kind == "album").unwrap().items[0].clone();
        assert_eq!(
            album
                .local_collection(&store)
                .unwrap()
                .iter()
                .map(|t| t.path.as_str())
                .collect::<Vec<_>>(),
            ["a.mp3", "b.mp3"]
        );
        let tracks = album.collection_tracks(vec![Track::default(), store.tracks[2].clone()]);
        assert_eq!(tracks[0].album, "Album");
        assert_eq!(tracks[0].artist, "Artist");
        assert_eq!(tracks[1].album, "Other album");
        assert_eq!(tracks[1].artist, "Other artist");
    }
    #[test]
    fn mixed_home_preserves_album_artist_playlist_and_wide_video_targets() {
        let tile = |id: &str| serde_json::json!({"musicTwoRowItemRenderer":{"title":{"runs":[{"text":id}]},"subtitle":{"runs":[{"text":"Artist"}]},"navigationEndpoint":{"browseEndpoint":{"browseId":id}},"thumbnailRenderer":{"musicThumbnailRenderer":{"thumbnail":{"thumbnails":[{"url":"https://example.test/cover.jpg","width":200,"height":200}]}}}}});
        let video = serde_json::json!({"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"Video"}]},"navigationEndpoint":{"watchEndpoint":{"videoId":"abcdefghijk"}},"thumbnailRenderer":{"musicThumbnailRenderer":{"thumbnail":{"thumbnails":[{"url":"https://example.test/video.jpg","width":480,"height":270}]}}}}});
        let catalog = parse(
            &serde_json::json!({"musicCarouselShelfRenderer":{"header":{"musicCarouselShelfBasicHeaderRenderer":{"title":{"runs":[{"text":"For you"}]}}},"contents":[tile("MPREalbum"),tile("UCartist"),tile("VLplaylist"),video]}}),
        );
        assert_eq!(
            catalog
                .sections
                .iter()
                .map(|s| s.kind.as_str())
                .collect::<Vec<_>>(),
            ["album", "artist", "playlist", "video"]
        );
        assert_eq!(catalog.sections[0].items[0].browse_id, "MPREalbum");
        assert!(catalog.sections[0].items[0].track().is_none());
        assert_eq!(
            catalog.sections[3].items[0]
                .track()
                .unwrap()
                .clip_video_id
                .as_deref(),
            Some("abcdefghijk")
        );
    }
    #[test]
    fn empty_catalog_never_creates_empty_shelves() {
        assert!(parse(&serde_json::json!({"contents":[]}))
            .sections
            .is_empty());
        assert!(compose(&Store::default()).is_empty());
    }

    #[test]
    fn home_deduplicates_incomplete_local_tracks_without_merging_distinct_versions() {
        let local = Track { path: "local.mp3".into(), title: "Same song".into(), duration_secs: 104.0, ..Default::default() };
        let remote = Track { path: "yt://abcdefghijk".into(), title: "Same song".into(), artist: "Artist".into(), duration_secs: 104.0, ..Default::default() };
        let result = home_tracks(&Store::default(), [local.clone(), remote.clone()]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "local.mp3");
        assert_eq!(result[0].artist, "Artist");
        assert_eq!(result[0].video_id.as_deref(), Some("abcdefghijk"));
        let local_full = Track { artist: "Artist".into(), ..local.clone() };
        let remote_incomplete = Track { artist: "".into(), thumbnail_url: "https://example.test/art".into(), ..remote.clone() };
        let result = home_tracks(&Store::default(), [remote_incomplete, local_full]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "local.mp3");
        assert_eq!(result[0].video_id.as_deref(), Some("abcdefghijk"));
        assert_eq!(result[0].thumbnail_url, "https://example.test/art");
        let another = Track { path: "yt://lmnopqrstuv".into(), artist: "Other artist".into(), ..remote.clone() };
        assert_eq!(home_tracks(&Store::default(), [local.clone(), remote.clone(), another]).len(), 3);
        let different_id = Track { path: "yt://lmnopqrstuv".into(), video_id: Some("lmnopqrstuv".into()), artist: "".into(), ..local };
        assert_eq!(home_tracks(&Store::default(), [different_id, remote]).len(), 2);
    }
}
