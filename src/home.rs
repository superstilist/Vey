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

#[derive(Clone, Debug, Default)]
pub struct RecommendationProfile {
    pub seeds: Vec<Track>,
    excluded_paths: HashSet<String>,
    excluded_video_ids: HashSet<String>,
}

/// Blend the current song, recent listening and liked songs into a small
/// public-radio profile. Keep the network request count bounded to three seeds.
pub fn recommendation_profile(store: &Store, current: Option<&Track>) -> RecommendationProfile {
    const SEED_LIMIT: usize = 3;
    const RECENT_EXCLUSION_LIMIT: usize = 12;
    let mut profile = RecommendationProfile::default();
    let mut seen_seeds = HashSet::new();

    if let Some(track) = current {
        push_recommendation_seed(&mut profile.seeds, &mut seen_seeds, track);
        profile.excluded_paths.insert(track.path.clone());
        if let Some(id) = track.video_id.as_deref().filter(|id| !id.is_empty()) {
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
        if store.likes.contains(&track.path) {
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

    for entry in store.history.iter().take(RECENT_EXCLUSION_LIMIT) {
        profile.excluded_paths.insert(entry.track.path.clone());
        if let Some(id) = entry.track.video_id.as_deref().filter(|id| !id.is_empty()) {
            profile.excluded_video_ids.insert(id.to_string());
        }
    }
    for track in &store.tracks {
        if store.likes.contains(&track.path) {
            profile.excluded_paths.insert(track.path.clone());
            if let Some(id) = track.video_id.as_deref().filter(|id| !id.is_empty()) {
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
    if track
        .video_id
        .as_deref()
        .filter(|id| !id.is_empty())
        .is_none()
        && recommendation_query(track).is_none()
    {
        return false;
    }
    let key = track
        .video_id
        .as_deref()
        .filter(|id| !id.is_empty())
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
            let Some(id) = track.video_id.as_deref().filter(|id| !id.is_empty()) else {
                continue;
            };
            if profile.excluded_video_ids.contains(id)
                || profile.excluded_paths.contains(&track.path)
            {
                continue;
            }
            let score = source_weight / (1.0 + rank as f64 * 0.18);
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
            if command!="import" {ui.set_music_status("Підключення YouTube Music і Spotify доступне в настільній Vey. Публічний плейліст можна імпортувати тут за посиланням.".into());return;}
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
                    Ok(playlist)=>{
                        let mut store=get();
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
