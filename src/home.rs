//! Home composition selection shared by the native and browser shell.
use crate::{
    services::{Store, Track},
    MainWindow,
};
use slint::ComponentHandle;
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

fn parse_blueberry_candidates(data: &serde_json::Value) -> Vec<Track> {
    let mut seen = HashSet::new();
    data.get("items").and_then(serde_json::Value::as_array).into_iter().flatten()
        .take(200).filter_map(|row| {
            let id = row.get("youtube_id")?.as_str()?;
            let title = row.get("title")?.as_str()?.trim();
            let artist = row.get("artist")?.as_str()?.trim();
            let recording_type = row.get("recording_type").and_then(serde_json::Value::as_str).unwrap_or("unknown");
            if !crate::services::video_id(id) || title.is_empty() || artist.is_empty()
                || recording_type != "unknown" || crate::account::recording_type(title) != "unknown"
                || !seen.insert(id.to_owned()) { return None; }
            let art = row.get("cover_art").and_then(serde_json::Value::as_str).unwrap_or_default();
            let art = reqwest::Url::parse(art).ok().filter(|url| url.scheme() == "https" && url.username().is_empty() && url.password().is_none())
                .map(|url| url.to_string()).unwrap_or_default();
            let clip = row.get("clip_video_id").and_then(serde_json::Value::as_str)
                .filter(|id| crate::services::video_id(id)).map(str::to_owned);
            Some(Track { path: format!("yt://{id}"), video_id: Some(id.to_owned()), clip_video_id: clip,
                title: title.chars().take(512).collect(), artist: artist.chars().take(512).collect(),
                album: row.get("album").and_then(serde_json::Value::as_str).unwrap_or_default().chars().take(512).collect(),
                thumbnail_url: art, ..Default::default() })
        }).collect()
}

/// Read public, verified Blueberry recordings without sending private signals.
/// Vey applies the listener's profile locally in `rank_recommendations`.
pub(crate) async fn blueberry_candidates() -> Vec<Track> {
    let Some(base) = crate::account::recommendation_catalog_base() else { return Vec::new() };
    let fetch = async move {
        let response = reqwest::get(format!("{base}/vey/catalog/recordings?sort=release&limit=100")).await.ok()?;
        if !response.status().is_success() || response.content_length().is_some_and(|len| len > 2_000_000) { return None; }
        let bytes = response.bytes().await.ok()?;
        if bytes.len() > 2_000_000 { return None; }
        let data = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
        Some(parse_blueberry_candidates(&data))
    };
    #[cfg(not(target_arch = "wasm32"))]
    { tokio::time::timeout(std::time::Duration::from_secs(8), fetch).await.ok().flatten().unwrap_or_default() }
    #[cfg(target_arch = "wasm32")]
    {
        let delay = js_sys::Promise::new(&mut |resolve, _| {
            if let Some(window) = web_sys::window() {
                if window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 8_000).is_err() {
                    let _ = resolve.call0(&wasm_bindgen::JsValue::UNDEFINED);
                }
            } else {
                let _ = resolve.call0(&wasm_bindgen::JsValue::UNDEFINED);
            }
        });
        match futures_util::future::select(Box::pin(fetch), Box::pin(wasm_bindgen_futures::JsFuture::from(delay))).await {
            futures_util::future::Either::Left((result, _)) => result.unwrap_or_default(),
            futures_util::future::Either::Right(_) => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RecommendationProfile {
    pub seeds: Vec<Track>,
    excluded_paths: HashSet<String>,
    excluded_video_ids: HashSet<String>,
    pub track_weights: HashMap<String, i16>,
    pub artist_weights: HashMap<String, i16>,
    disliked_tracks: HashSet<String>,
}

/// A local listening seed is usable only when one public recording matches
/// its title, artist and duration; never seed radio with the first search hit.
pub fn select_public_seed(source: &Track, candidates: Vec<Track>) -> Option<String> {
    let mut ids = HashSet::new();
    for candidate in candidates {
        if crate::services::same_track_identity(source, &candidate) {
            if let Some(id) = crate::services::track_video_id(&candidate) { ids.insert(id.to_owned()); }
        }
    }
    (ids.len() == 1).then(|| ids.into_iter().next()).flatten()
}

/// Blend the current song, recent listening and liked songs into a small
/// public-radio profile. Keep the network request count bounded to three seeds.
pub fn recommendation_profile(store: &Store, current: Option<&Track>) -> RecommendationProfile {
    const SEED_LIMIT: usize = 3;
    const RECENT_EXCLUSION_LIMIT: usize = 12;
    let mut profile = RecommendationProfile::default();
    let mut seen_seeds = HashSet::new();
    profile.track_weights = store.recommendations.track_weights.clone().into_iter().collect();
    profile.artist_weights = store.recommendations.artist_weights.clone().into_iter().collect();
    profile.disliked_tracks = store.recommendations.disliked_tracks.iter().cloned().collect();

    if let Some(track) = current {
        push_recommendation_seed(&mut profile.seeds, &mut seen_seeds, track);
        profile.excluded_paths.insert(track.path.clone());
        if let Some(id) = crate::services::track_video_id(track) {
            profile.excluded_video_ids.insert(id.to_string());
        }
    }

    let recent_seed_limit = 1;
    let mut recent_seeds = 0;
    for entry in &store.history {
        if recent_seeds < recent_seed_limit
            && current.is_none_or(|current| entry.track.path != current.path)
        {
            if push_recommendation_seed(&mut profile.seeds, &mut seen_seeds, &entry.track) {
                recent_seeds += 1;
            }
        }
        if recent_seeds >= recent_seed_limit || profile.seeds.len() >= SEED_LIMIT {
            break;
        }
    }

    for track in &store.tracks {
        if profile.seeds.len() >= SEED_LIMIT {
            break;
        }
        if store.is_liked(track) {
            if push_recommendation_seed(&mut profile.seeds, &mut seen_seeds, track) { break; }
        }
    }
    for playlist in store.playlists.iter().rev() {
        for path in &playlist.tracks {
            if profile.seeds.len() >= SEED_LIMIT { break; }
            if let Some(track) = store.tracks.iter().find(|t| &t.path == path) {
                if push_recommendation_seed(&mut profile.seeds, &mut seen_seeds, track) { break; }
            }
        }
        if profile.seeds.len() >= SEED_LIMIT { break; }
    }
    if profile.seeds.len() < SEED_LIMIT {
        for entry in &store.history {
            if profile.seeds.len() >= SEED_LIMIT {
                break;
            }
            push_recommendation_seed(&mut profile.seeds, &mut seen_seeds, &entry.track);
        }
    }
    if profile.seeds.len() < SEED_LIMIT {
        for track in store.library_tracks() {
            if profile.seeds.len() >= SEED_LIMIT {
                break;
            }
            push_recommendation_seed(&mut profile.seeds, &mut seen_seeds, &track);
        }
    }

    for entry in store.history.iter().take(RECENT_EXCLUSION_LIMIT) {
        profile.excluded_paths.insert(entry.track.path.clone());
        if let Some(id) = crate::services::track_video_id(&entry.track) {
            profile.excluded_video_ids.insert(id.to_string());
        }
    }
    for track in &store.tracks {
        if store.is_liked(track) {
            profile.excluded_paths.insert(track.path.clone());
            if let Some(id) = crate::services::track_video_id(track) {
                profile.excluded_video_ids.insert(id.to_string());
            }
        }
    }
    profile
}

fn push_recommendation_seed(
    seeds: &mut Vec<Track>,
    seen: &mut HashSet<String>,
    track: &Track,
) -> bool {
    if crate::services::track_video_id(track).is_none() && recommendation_query(track).is_none() {
        return false;
    }
    let key = crate::services::track_video_id(track)
        .map(|id| format!("video:{id}"))
        .unwrap_or_else(|| format!("path:{}", track.path));
    if !seen.insert(key) {
        return false;
    }
    seeds.push(track.clone());
    true
}

/// Combine radio batches by consensus, preserve each source's rank, and avoid
/// suggesting very recently played or already-liked songs. A light artist cap
/// keeps the first screen from becoming the same act twelve times in a row.
pub fn rank_recommendations(
    batches: Vec<Vec<Track>>,
    profile: &RecommendationProfile,
    limit: usize,
) -> Vec<Track> {
    struct Candidate {
        track: Track,
        score: f64,
        source_hits: HashSet<usize>,
        first_order: usize,
    }

    let mut candidates = Vec::<Candidate>::new();
    let mut indices = HashMap::<String, usize>::new();
    let mut order = 0;
    for (source, batch) in batches.into_iter().enumerate() {
        let source_weight = match source {
            0 => 2.2,
            1 => 1.7,
            2 => 1.3,
            _ => 0.9,
        };
        for (rank, track) in batch.into_iter().enumerate() {
            let Some(id) = crate::services::track_video_id(&track) else {
                continue;
            };
            let canonical = format!("yt://{id}");
            if profile.excluded_video_ids.contains(id)
                || profile.excluded_paths.contains(&track.path)
                || profile.disliked_tracks.contains(&canonical)
            {
                continue;
            }
            let artist = normalize_artist(&track.artist);
            let personal_track = profile.track_weights.get(&canonical).copied().unwrap_or(0);
            let personal_artist = profile.artist_weights.get(&artist).copied().unwrap_or(0);
            let score = source_weight / (1.0 + rank as f64 * 0.18)
                + f64::from(personal_track.clamp(-20, 30)) * 0.18
                + f64::from(personal_artist.clamp(-30, 40)) * 0.08;
            let index = if let Some(index) = indices.get(id) {
                *index
            } else {
                let index = candidates.len();
                indices.insert(id.to_string(), index);
                candidates.push(Candidate {
                    track,
                    score: 0.0,
                    source_hits: HashSet::new(),
                    first_order: order,
                });
                index
            };
            let candidate = &mut candidates[index];
            if candidate.source_hits.insert(source) {
                candidate.score += score
                    + if candidate.source_hits.len() > 1 {
                        0.55
                    } else {
                        0.0
                    };
            }
            order += 1;
        }
    }

    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.source_hits.len().cmp(&a.source_hits.len()))
            .then_with(|| a.first_order.cmp(&b.first_order))
    });
    let mut selected = Vec::new();
    let mut deferred = Vec::new();
    let mut artist_counts = HashMap::<String, usize>::new();
    for candidate in candidates {
        let artist = candidate.track.artist.trim().to_lowercase();
        let key = if artist.is_empty() {
            candidate.track.video_id.clone().unwrap_or_default()
        } else {
            artist
        };
        if artist_counts.get(&key).copied().unwrap_or(0) >= 2 {
            deferred.push((key, candidate.track));
        } else {
            *artist_counts.entry(key).or_default() += 1;
            selected.push(candidate.track);
        }
        if selected.len() >= limit {
            return selected;
        }
    }
    for (artist, track) in deferred {
        if selected.len() >= limit {
            break;
        }
        *artist_counts.entry(artist).or_default() += 1;
        selected.push(track);
    }
    selected
}

fn normalize_artist(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

/// Offline fallback for an empty cache: prefer local songs with positive
/// history/playlist/like signals, while keeping first-seen order on ties.
pub fn local_recommendations(store: &Store, limit: usize) -> Vec<Track> {
    let mut tracks: Vec<_> = store
        .library_tracks()
        .into_iter()
        .enumerate()
        .filter(|(_, track)| !store.is_disliked(track))
        .filter(|(_, track)| !store.is_liked(track))
        .filter(|(_, track)| {
            !store
                .history
                .iter()
                .take(12)
                .any(|entry| crate::services::same_track_identity(&entry.track, track))
        })
        .map(|(order, track)| {
            let track_key = store.identity_key(&track);
            let artist_key = normalize_artist(&track.artist);
            let score = store
                .recommendations
                .track_weights
                .get(&track_key)
                .copied()
                .unwrap_or(0) as i32
                + store
                    .recommendations
                    .artist_weights
                    .get(&artist_key)
                    .copied()
                    .unwrap_or(0) as i32;
            (score, order, track)
        })
        .collect();
    tracks.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    tracks.into_iter().take(limit).map(|(_, _, track)| track).collect()
}

/// Local files do not have YouTube video IDs, so use their real metadata to
/// find a catalog match before requesting a radio station.
pub fn recommendation_query(track: &Track) -> Option<String> {
    let title = track.title.trim();
    if title.is_empty() {
        return None;
    }
    let artist = track.artist.trim();
    Some(if artist.is_empty() {
        title.to_string()
    } else {
        format!("{artist} {title}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blueberry_candidates_reject_invalid_ids_and_duplicate_rows() {
        let data = serde_json::json!({"items":[
            {"youtube_id":"abcdefghijk","title":"Song","artist":"Artist","album":"Album","cover_art":"https://example.test/art","clip_video_id":"clip0000001"},
            {"youtube_id":"abcdefghijk","title":"Duplicate","artist":"Artist"},
            {"youtube_id":"invalid","title":"Wrong","artist":"Artist"},
            {"youtube_id":"12345678901","title":" ","artist":"Artist"},
            {"youtube_id":"22222222222","title":"Song (Remix)","artist":"Artist","recording_type":"unknown"},
            {"youtube_id":"33333333333","title":"Song","artist":"Artist","recording_type":"live"},
            {"youtube_id":"lmnopqrstuv","title":"Other","artist":"Artist","cover_art":"http://example.test/not-https"}
        ]});
        let tracks = parse_blueberry_candidates(&data);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].path, "yt://abcdefghijk");
        assert_eq!(tracks[0].clip_video_id.as_deref(), Some("clip0000001"));
        assert!(tracks[1].thumbnail_url.is_empty());
    }
    #[test]
    fn local_radio_seed_requires_a_unique_metadata_and_duration_match() {
        let local = Track { path: "C:/music/song.flac".into(), title: "Song".into(), artist: "Artist".into(),
            duration_secs: 180.0, ..Default::default() };
        let good = Track { path: "yt://abcdefghijk".into(), video_id: Some("abcdefghijk".into()), ..local.clone() };
        let unrelated = Track { title: "Other".into(), ..good.clone() };
        assert_eq!(select_public_seed(&local, vec![unrelated, good.clone()]).as_deref(), Some("abcdefghijk"));
        let ambiguous = Track { path: "yt://12345678901".into(), video_id: Some("12345678901".into()), ..good.clone() };
        assert!(select_public_seed(&local, vec![good, ambiguous]).is_none());
    }
    use crate::services::{History, Playlist};

    fn remote(id: &str, artist: &str) -> Track {
        Track {
            path: format!("yt://{id}"),
            video_id: Some(id.into()),
            title: format!("Track {id}"),
            artist: artist.into(),
            duration_secs: 180.0,
            ..Default::default()
        }
    }

    #[test]
    fn cold_start_profile_uses_library_and_local_fallback() {
        let mut store = Store::default();
        store.tracks = vec![
            Track { path: "a.mp3".into(), title: "A".into(), artist: "Artist".into(), duration_secs: 90.0, ..Default::default() },
            Track { path: "b.mp3".into(), title: "B".into(), artist: "Artist".into(), duration_secs: 91.0, ..Default::default() },
        ];
        let profile = recommendation_profile(&store, None);
        assert!(!profile.seeds.is_empty());
        assert_eq!(local_recommendations(&store, 4).len(), 2);
    }

    #[test]
    fn learned_feedback_ranks_matching_artist_and_excludes_dislikes() {
        let favorite = remote("abcdefghijk", "Favorite Artist");
        let disliked = remote("lmnopqrstuv", "Other Artist");
        let liked = remote("zzzzzzzzzzz", "Favorite Artist");
        let mut store = Store {
            tracks: vec![favorite.clone(), disliked.clone(), liked.clone()],
            history: vec![History { track: favorite.clone(), play_count: 7, played_at: crate::services::now_ms(), completion_count: 3, ..Default::default() }],
            playlists: vec![Playlist { id: "p".into(), name: "P".into(), tracks: vec![favorite.path.clone()], ..Default::default() }],
            ..Default::default()
        };
        store.recommendations.disliked_tracks.insert(disliked.path.clone());
        store.refresh_recommendation_signals();
        let profile = recommendation_profile(&store, None);
        let ranked = rank_recommendations(vec![vec![disliked, liked.clone()]], &profile, 12);
        assert_eq!(ranked.iter().map(|track| track.path.as_str()).collect::<Vec<_>>(), [liked.path.as_str()]);
        assert!(profile.artist_weights.get("favoriteartist").copied().unwrap_or_default() > 0);
    }
}

pub fn install(w: &MainWindow, get: Rc<dyn Fn() -> Store>, apply: Rc<dyn Fn(Store)>) {
    let weak = w.as_weak();
    let home_get = get.clone();
    let home_apply = apply.clone();
    w.on_home_choose(move |choice| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut store = home_get();
        if choice == -2 {
            store.settings.home_rotate = !store.settings.home_rotate;
        } else {
            let selected = if choice < 0 {
                // Each visit chooses another complete composition; never shuffles mid-song.
                (ui.get_home_layout() + 1 + (crate::services::now_ms() % 5) as i32) % 6
            } else {
                choice.clamp(0, 5)
            };
            store.settings.home_layout = selected as u8;
            if choice >= 0 {
                store.settings.home_rotate = false;
            }
        }
        home_apply(store);
    });
    if w.get_home_rotate() {
        w.invoke_home_choose(-1);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let weak = w.as_weak();
        let generation = Rc::new(std::cell::Cell::new(0u64));
        w.on_music_command(move |command| {
            let Some(ui)=weak.upgrade() else{return;};
            if command=="select" {ui.set_music_status("".into());return;}
            if command=="cancel" {generation.set(generation.get()+1);ui.set_music_busy(false);ui.set_music_status("Скасовано".into());return;}
            if command!="import" {ui.set_music_status("Підключення приватної бібліотеки YouTube Music доступне в настільній Vey. Публічні плейлісти YouTube Music і Spotify можна імпортувати за посиланням.".into());return;}
            if ui.get_music_busy(){return;}
            let link=ui.get_music_link().to_string();
            if let Err(error)=crate::library_import::playlist_link(&link){ui.set_music_status(error.into());return;}
            generation.set(generation.get()+1);let revision=generation.get();
            let epoch=generation.clone();let weak=weak.clone();let get=get.clone();let apply=apply.clone();
            ui.set_music_busy(true);ui.set_music_status("Імпорт плейліста…".into());
            wasm_bindgen_futures::spawn_local(async move {
                let origin=web_sys::window().and_then(|w|w.location().origin().ok()).unwrap_or_default();
                let result=async {
                    let response=reqwest::Client::new().post(format!("{origin}/api/playlists/import")).json(&link).send().await.map_err(|_|"Немає зв’язку із сервером Vey")?;
                    if !response.status().is_success(){return Err("Імпорт недоступний. Перевірте посилання й запустіть Vey --web-server.");}
                    response.json::<crate::library_import::ImportedPlaylist>().await.map_err(|_|"Некоректна відповідь імпорту")
                }.await;
                if epoch.get()!=revision{return;}
                let Some(ui)=weak.upgrade() else{return;};ui.set_music_busy(false);
                match result {
                    Ok(mut playlist)=>{
                        let mut store=get();
                        crate::library_import::reuse_legacy_spotify_identity(&store,&mut playlist);
                        let key=format!("vey:import:{}",playlist.id);
                        let storage=web_sys::window().and_then(|w|w.local_storage().ok().flatten());
                        let baseline=storage.as_ref().and_then(|s|s.get_item(&key).ok().flatten());
                        match crate::library_import::apply(&mut store,&playlist,baseline.as_deref()) {
                            Ok(hash)=>{if let Some(storage)=storage {let _=storage.set_item(&key,&hash);}apply(store);ui.set_music_status(format!("Імпортовано: {} · {} композицій",playlist.name,playlist.tracks.len()).into());}
                            Err(error)=>ui.set_music_status(error.into()),
                        }
                    }
                    Err(error)=>ui.set_music_status(error.into()),
                }
            });
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (get, apply);
}
