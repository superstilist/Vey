//! Rust/WASM controller with the original Braccato rendering widget.
use crate::{lyrics::*, services::*, *};
use futures_util::future::join_all;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    time::Duration,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
#[wasm_bindgen(module = "/web/bridge.js")]
extern "C" {
    #[wasm_bindgen(js_name=pickFiles)]
    fn pick_files() -> js_sys::Promise;
    #[wasm_bindgen(js_name=pickSingleFile)]
    fn pick_single_file() -> js_sys::Promise;
    #[wasm_bindgen(js_name=readJsonFile)]
    fn read_json() -> js_sys::Promise;
    #[wasm_bindgen(js_name=downloadJson)]
    fn download_json(text: &str);
    #[wasm_bindgen(js_name=fileUrl)]
    fn file_url(path: &str) -> String;
    #[wasm_bindgen(js_name=unlinkFile)]
    fn unlink_file(path: &str);
    #[wasm_bindgen(js_name=cancelDownload)]
    fn cancel_download(id: &str);
    #[wasm_bindgen(js_name=downloadTrack)]
    fn download_track(id: &str, filename: &str, on_progress: &js_sys::Function) -> js_sys::Promise;
    fn challenge() -> js_sys::Promise;
    #[wasm_bindgen(js_name=viewportInsets)]
    fn viewport_insets() -> String;
    #[wasm_bindgen(js_name=onStoreFlush)]
    fn on_store_flush(callback: &js_sys::Function);
    #[wasm_bindgen(js_name=installMediaSession)]
    fn install_media_session(callback: &js_sys::Function) -> bool;
    #[wasm_bindgen(js_name=setMediaMetadata)]
    fn set_media_metadata(title: &str, artist: &str, album: &str, artwork: &str, duration: f64);
    #[wasm_bindgen(js_name=setMediaPlayback)]
    fn set_media_playback(playing: bool, position: f64, duration: f64, rate: f64);
}
#[wasm_bindgen(module = "/web/artwork-fetch.js")]
extern "C" {
    #[wasm_bindgen(js_name=fetchArtwork)]
    fn fetch_artwork(url: &str, max_bytes: u32) -> js_sys::Promise;
    #[wasm_bindgen(js_name=decodeArtwork)]
    fn decode_artwork(bytes: &js_sys::Uint8Array, max_edge: u32) -> js_sys::Promise;
}
struct WebApp {
    store: Store,
    page_results: HashMap<String, Vec<Track>>,
    library: Vec<Track>,
    library_view: LibraryViewState,
    visible: Vec<Track>,
    queue: Queue,
    download_pending: HashSet<String>,
    download_progress: HashMap<String, i32>,
    download_ids: HashMap<String, String>,
    download_callbacks: HashMap<String, Closure<dyn FnMut(f64)>>,
    current: Option<Track>,
    playlist: String,
    audio: web_sys::HtmlAudioElement,
    alternatives: Vec<LyricsAlternative>,
    enrichment_attempts: RefCell<HashSet<String>>,
    kugou_supplement_attempts: HashSet<String>,
    self_ref: std::rc::Weak<RefCell<WebApp>>,
    artwork: HashMap<String, slint::Image>,
    artwork_pending: HashSet<String>,
    artwork_queue: VecDeque<Track>,
    artwork_failed: HashMap<String, web_time::Instant>,
    generation: u64,
    home_recommendation_generation: u64,
    home_recommendation_loading: bool,
    home_recommendation_loaded: bool,
    home_recommendations: Vec<Track>,
    search_index: crate::search_index::SearchIndex,
    search_cache: crate::search_index::SearchCache,
    pending_search: Option<(web_time::Instant, String)>,
    lyric_generation: u64,
    playback_generation: u64,
    playback_recorded: bool,
    ended: bool,
    save_due: Cell<Option<web_time::Instant>>,
    media_handler: Option<Closure<dyn FnMut(String, f64)>>,
}
fn apply_viewport_insets(w: &MainWindow) {
    if let Ok([top, bottom, left, right]) = serde_json::from_str::<[f32; 4]>(&viewport_insets()) {
        w.set_safe_top(top);
        w.set_safe_bottom(bottom);
        w.set_safe_left(left);
        w.set_safe_right(right);
    }
}
fn api(path: &str) -> String {
    format!(
        "{}{}",
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default(),
        path
    )
}
fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::from(Rc::new(VecModel::from(v)))
}
fn save(a: &WebApp) {
    if a.save_due.get().is_none() {
        a.save_due
            .set(Some(web_time::Instant::now() + Duration::from_millis(200)));
    }
}
fn flush_save(a: &WebApp) {
    if a.save_due.take().is_none() {
        return;
    }
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(&a.store) {
            let _ = storage.set_item("playervf.rust.v1", &json);
        }
    }
}
fn playlist_detail(p: &Playlist) -> String {
    let author = if p.author.trim().is_empty() { "Ви" } else { p.author.trim() };
    let days = crate::services::now_ms().saturating_sub(p.updated_at) / 86_400_000;
    let modified = if p.updated_at == 0 { "ще не змінювався".to_string() } else { match days { 0 => "змінено сьогодні".to_string(), 1 => "змінено учора".to_string(), n => format!("змінено {n} дн. тому") } };
    format!("{} композицій · {} · {}", p.tracks.len(), author, modified)
}
fn refresh_playlist_rows(w: &MainWindow, a: &WebApp) {
    if let Some(p) = a.store.playlists.iter().find(|p| p.id == a.playlist) {
        w.set_playlist_title(p.name.clone().into());
        w.set_playlist_detail(playlist_detail(p).into());
        w.set_playlist_public(p.is_public);
        w.set_playlist_art(p.tracks.first().and_then(|path| a.artwork.get(path)).cloned().unwrap_or_default());
    } else {
        w.set_playlist_detail("".into());
        w.set_playlist_public(false);
        w.set_playlist_art(slint::Image::default());
    }
    w.set_playlists(model(a.store.playlists.iter().map(|p| PlaylistRow {
        id: p.id.clone().into(),
        name: p.name.clone().into(),
        detail: playlist_detail(p).into(),
        art: p.tracks.first().and_then(|path| a.artwork.get(path)).cloned().unwrap_or_default(),
    }).collect()));
    w.set_playlist_destinations(w.get_playlists());
}
fn refresh_queue_view(w: &MainWindow, a: &WebApp) {
    w.set_queue_tracks(model(a.queue.tracks.iter().map(|track| TrackRow {
        title: track.title.clone().into(),
        artist: track.artist.clone().into(),
        album: track.album.clone().into(),
        path: track.path.clone().into(),
        duration: format_time(track.duration_secs).into(),
        art: a.artwork.get(&track.path).cloned().unwrap_or_default(),
        liked: a.store.is_liked(track),
        downloaded: !file_url(&track.path).is_empty()
            || a.store.downloads.contains_key(&track.path)
            || a.store.downloads.contains_key(&a.store.identity_key(track)),
        ..Default::default()
    }).collect()));
    w.set_queue_current_index(a.queue.index.map_or(-1, |index| index as i32));
}
fn refresh(w: &MainWindow, a: &mut WebApp) {
    w.set_home_revision(w.get_home_revision().wrapping_add(1));
    a.visible = match w.get_active_view().as_str() {
        "home" => {
            if a.store.history.is_empty() {
                a.library.clone()
            } else {
                a.store.history.iter().map(|h| h.track.clone()).collect()
            }
        }
        "library" => {
            if matches!(a.library_view.category, LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists) {
                Vec::new()
            } else {
                let source = if a.library_view.filter == LibraryFilter::Excluded {
                    a.store.excluded_library_tracks()
                } else {
                    a.library.clone()
                };
                let local_urls: HashSet<String> = source
                    .iter()
                    .filter(|track| !crate::services::key_is_remote(&track.path))
                    .filter(|track| !file_url(&track.path).is_empty())
                    .map(|track| track.path.clone())
                    .collect();
                a.library_view.apply(&a.store, &source, |track| {
                    local_urls.contains(&track.path)
                        || a.store.downloads.contains_key(&track.path)
                        || a.store.downloads.contains_key(&a.store.identity_key(track))
                })
            }
        }
        "liked" => a
            .store
            .library_tracks()
            .into_iter()
            .filter(|t| a.store.is_liked(t))
            .collect(),
        "queue" => a.queue.tracks.clone(),
        "playlist" => a.store.playlist_tracks(&a.playlist),
        _ => a.visible.clone(),
    };
    let library_count = if a.library_view.filter == LibraryFilter::Excluded {
        a.store.excluded_library_tracks().len()
    } else {
        a.library.len()
    };
    w.set_library_category_index(match a.library_view.category {
        LibraryCategory::Songs => 0,
        LibraryCategory::Albums => 1,
        LibraryCategory::Artists => 2,
        LibraryCategory::Playlists => 3,
        LibraryCategory::Downloads => 4,
    });
    w.set_library_total_count(library_count as i32);
    w.set_library_selected_count(a.library_view.selected.len() as i32);
    w.set_library_scanning(false);
    refresh_queue_view(w, a);
    let selected_tracks: Vec<_> = if a.library_view.selected.is_empty() {
        Vec::new()
    } else {
        a.library.iter().filter(|track| a.library_view.selected.contains(&a.store.identity_key(track))).collect()
    };
    w.set_library_selection_all_liked(!selected_tracks.is_empty() && selected_tracks.iter().all(|track| a.store.is_liked(track)));
    if w.get_active_view() == "library" && matches!(a.library_view.category, LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists) {
        let collection_source = if a.library_view.filter == LibraryFilter::Excluded {
            a.store.excluded_library_tracks()
        } else {
            a.library.clone()
        };
        let collections = a.library_view.collections(&a.store, &collection_source, |track| {
            !file_url(&track.path).is_empty()
                || a.store.downloads.contains_key(&track.path)
                || a.store.downloads.contains_key(&a.store.identity_key(track))
        });
        w.set_library_collections(model(collections.iter().map(|collection| LibraryCollectionRow {
            kind: collection.kind.clone().into(),
            key: collection.key.clone().into(),
            title: collection.title.clone().into(),
            subtitle: if collection.kind == "playlist" { String::new() } else { collection.subtitle.clone() }.into(),
            count: collection.track_count.to_string().into(),
            art: a.artwork.get(&collection.representative.path).cloned().unwrap_or_default(),
        }).collect()));
    } else if w.get_active_view() == "library" {
        w.set_library_collections(model(Vec::new()));
    }
    if matches!(w.get_active_view().as_str(), "search" | "explore") {
        a.page_results
            .insert(w.get_active_view().to_string(), a.visible.clone());
    }
    crate::track_model::set_tracks(
        w,
        a.visible
            .iter()
            .map(|t| TrackRow {
                title: t.title.clone().into(),
                artist: t.artist.clone().into(),
                album: t.album.clone().into(),
                path: t.path.clone().into(),
                duration: format_time(t.duration_secs).into(),
                art: a.artwork.get(&t.path).cloned().unwrap_or_default(),
                liked: a.store.is_liked(t),
                downloading: a.download_pending.contains(&t.path),
                download_progress: a.download_progress.get(&t.path).copied().unwrap_or(-1),
                downloaded: !file_url(&t.path).is_empty()
                    || a.store.downloads.contains_key(&t.path)
                    || a.store.downloads.contains_key(&a.store.identity_key(t)),
                selected: a.library_view.selected.contains(&a.store.identity_key(t)),
                missing: !crate::services::key_is_remote(&t.path) && file_url(&t.path).is_empty(),
                ..Default::default()
            })
            .collect(),
    );
    crate::track_model::set_home_recommendations(
        w,
        a.home_recommendations
            .iter()
            .map(|track| TrackRow {
                title: track.title.clone().into(),
                artist: track.artist.clone().into(),
                album: track.album.clone().into(),
                path: track.path.clone().into(),
                duration: format_time(track.duration_secs).into(),
                art: a.artwork.get(&track.path).cloned().unwrap_or_default(),
                liked: a.store.is_liked(track),
                downloaded: !file_url(&track.path).is_empty(),
                ..Default::default()
            })
            .collect(),
    );
    refresh_playlist_rows(w, a);
    if let Some(t) = &a.current {
        w.set_current_liked(a.store.is_liked(t));
        w.set_current_disliked(a.store.is_disliked(t));
        let available = !file_url(&t.path).is_empty();
        w.set_current_downloaded(available);
        w.set_current_downloading(a.download_pending.contains(&t.path));
        w.set_current_download_progress(a.download_progress.get(&t.path).copied().unwrap_or(-1));
        w.set_current_local(available);
    }
}
fn format_time(s: f64) -> String {
    let s = if s.is_finite() { s.max(0.) as u64 } else { 0 };
    format!("{}:{:02}", s / 60, s % 60)
}
fn selected(a: &WebApp, index: i32) -> Option<Track> {
    if index < 0 {
        a.current.clone()
    } else {
        a.visible.get(index as usize).cloned()
    }
}
fn selected_library_tracks(a: &WebApp) -> Vec<Track> {
    let tracks = a.library.iter().cloned().chain(a.store.excluded_library_tracks());
    deduplicate_tracks(tracks)
        .into_iter()
        .filter(|track| a.library_view.selected.contains(&a.store.identity_key(track)))
        .collect()
}
fn apply_artwork(w: &MainWindow, image: slint::Image) {
    w.set_current_art(image);
}
fn load_artwork(w: &MainWindow, a: &Rc<RefCell<WebApp>>, track: &Track) {
    if crate::artwork::remote_urls(track).is_empty() {
        return;
    }
    let path = track.path.clone();
    let mut app = a.borrow_mut();
    let priority = app
        .current
        .as_ref()
        .is_some_and(|current| current.path == path);
    if let Some(image) = app.artwork.get(&path).cloned() {
        drop(app);
        if priority {
            apply_artwork(w, image.clone());
        }
        crate::track_model::update_path(w, &path, |row| row.art = image.clone());
        crate::discovery::update_art(w, &path, &image);
        return;
    }
    if app.artwork_pending.contains(&path)
        || app
            .artwork_failed
            .get(&path)
            .is_some_and(|failed| failed.elapsed() < Duration::from_secs(30))
    {
        return;
    }
    if priority {
        // Keep the current recommendations, but discard older selected songs
        // that have not started downloading when the listener skips rapidly.
        let recommended: HashSet<_> = app
            .home_recommendations
            .iter()
            .map(|item| item.path.clone())
            .collect();
        app.artwork_queue
            .retain(|item| item.path == path || recommended.contains(&item.path));
    }
    if let Some(index) = app.artwork_queue.iter().position(|item| item.path == path) {
        if priority {
            app.artwork_queue.remove(index);
        } else {
            return;
        }
    }
    if priority {
        app.artwork_queue.push_front(track.clone());
    } else {
        app.artwork_queue.push_back(track.clone());
    }
    drop(app);
    pump_artwork(w, a);
}

fn pump_artwork(w: &MainWindow, state: &Rc<RefCell<WebApp>>) {
    // Match the native loader: two background covers plus one reserved slot
    // for the selected track. Browser canvas readback and Slint upload are
    // main-thread work, so a larger burst can hitch startup on modest devices.
    const MAX_IN_FLIGHT: usize = 2;
    loop {
        let track = {
            let mut app = state.borrow_mut();
            let current_waiting = app.artwork_queue.front().is_some_and(|queued| {
                app.current
                    .as_ref()
                    .is_some_and(|current| current.path == queued.path)
            });
            if app.artwork_pending.len() >= MAX_IN_FLIGHT
                && !(app.artwork_pending.len() == MAX_IN_FLIGHT && current_waiting)
            {
                return;
            }
            let Some(track) = app.artwork_queue.pop_front() else {
                return;
            };
            if app.artwork.contains_key(&track.path)
                || !app.artwork_pending.insert(track.path.clone())
            {
                continue;
            }
            track
        };
        let urls = crate::artwork::remote_urls(&track);
        let path = track.path;
        let weak = w.as_weak();
        let state = state.clone();
        spawn_local(async move {
            let result: Result<slint::Image, String> = async {
                let mut decoded = None;
                for url in &urls {
                    if let Ok(value) = JsFuture::from(fetch_artwork(url, crate::artwork::MAX_BYTES as u32)).await {
                        let bytes = js_sys::Uint8Array::new(&value);
                        let Ok(value) = JsFuture::from(decode_artwork(&bytes, 768)).await else { continue; };
                        let width = js_sys::Reflect::get(&value, &"width".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
                        let height = js_sys::Reflect::get(&value, &"height".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as u32;
                        let data = js_sys::Reflect::get(&value, &"data".into()).ok();
                        if width == 0 || height == 0 || width > 768 || height > 768 { continue; }
                        if let Some(data) = data {
                            let pixels = js_sys::Uint8ClampedArray::new(&data).to_vec();
                            if pixels.len() == width as usize * height as usize * 4 {
                                decoded = Some((pixels, width, height));
                                break;
                            }
                        }
                    }
                }
                let (pixels, width, height) = decoded.ok_or_else(|| "Artwork request failed".to_string())?;
                let image = slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&pixels, width, height));
                Ok(image)
            }
            .await;
            let Some(w) = weak.upgrade() else {
                return;
            };
            let mut app = state.borrow_mut();
            app.artwork_pending.remove(&path);
            let is_current = app
                .current
                .as_ref()
                .is_some_and(|current| current.path == path);
            if let Ok(image) = result {
                app.artwork_failed.remove(&path);
                app.artwork.insert(path.clone(), image.clone());
                crate::track_model::update_path(&w, &path, |row| row.art = image.clone());
                crate::discovery::update_art(&w, &path, &image);
                refresh_playlist_rows(&w, &app);
                refresh_queue_view(&w, &app);
                if w.get_catalog_kind() != ""
                    && app.visible.first().is_some_and(|track| track.path == path)
                    && w.get_active_view() == "explore"
                {
                    w.set_catalog_art(image.clone());
                }
                if w.get_active_view() == "library"
                    && matches!(app.library_view.category, LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists)
                {
                    refresh(&w, &mut app);
                }
                if is_current {
                    apply_artwork(&w, image);
                }
            } else {
                app.artwork_failed
                    .retain(|_, failed| failed.elapsed() < Duration::from_secs(30));
                if app.artwork_failed.len() >= 256 {
                    app.artwork_failed.clear();
                }
                app.artwork_failed.insert(path, web_time::Instant::now());
                if is_current {
                    w.set_current_art(slint::Image::default());
                }
            }
            drop(app);
            pump_artwork(&w, &state);
        });
    }
}
fn show_preferred_lyrics(w: &MainWindow, a: &WebApp) {
    let index = a
        .current
        .as_ref()
        .map(|t| a.store.preferred_lyrics_index(t, &a.alternatives))
        .unwrap_or(0);
    show_lyrics(w, a, index);
}
fn show_lyrics(w: &MainWindow, a: &WebApp, index: usize) {
    if let Some(alt) = a.alternatives.get(index) {
        let mut enriched = alt.clone();
        w.set_lyrics_enrichment_status("".into());
        if let Some(request) = crate::lyrics_translation::Request::for_song(
            alt,
            &a.store.settings,
            a.current.as_ref().and_then(|t| t.video_id.clone()),
        ) {
            request.apply(
                &crate::lyrics_translation::Enrichment {
                    lines: vec![Default::default(); request.lines.len()],
                },
                &mut enriched,
            );
            let key = request.key();
            if let Some(value) = a.store.lyrics_enrichment.get(&key) {
                request.apply(value, &mut enriched);
            } else if request.needs_network(alt)
                && a.enrichment_attempts.borrow_mut().insert(key.clone())
            {
                w.set_lyrics_enrichment_status(
                    if request.translation {
                        "Перекладаємо слова…"
                    } else {
                        "Романізуємо слова…"
                    }
                    .into(),
                );
                let weak = w.as_weak();
                let state = a.self_ref.clone();
                spawn_local(async move {
                    let result = async {
                        reqwest::Client::new()
                            .post(api("/api/lyrics/enrich"))
                            .json(&request)
                            .send()
                            .await
                            .map_err(|e| e.to_string())?
                            .error_for_status()
                            .map_err(|e| e.to_string())?
                            .json::<crate::lyrics_translation::Enrichment>()
                            .await
                            .map_err(|e| e.to_string())
                    }
                    .await;
                    let (Some(w), Some(state)) = (weak.upgrade(), state.upgrade()) else {
                        return;
                    };
                    let mut a = state.borrow_mut();
                    let index = w.get_provider_index().max(0) as usize;
                    let current_key = a.alternatives.get(index).and_then(|alt| {
                        crate::lyrics_translation::Request::for_song(
                            alt,
                            &a.store.settings,
                            a.current.as_ref().and_then(|t| t.video_id.clone()),
                        )
                        .map(|r| r.key())
                    });
                    match result {
                        Ok(value) => {
                            while a.store.lyrics_enrichment.len() >= 128 {
                                if let Some(key) = a.store.lyrics_enrichment.keys().next().cloned()
                                {
                                    a.store.lyrics_enrichment.remove(&key);
                                }
                            }
                            a.store.lyrics_enrichment.insert(key.clone(), value);
                            a.enrichment_attempts.borrow_mut().remove(&key);
                            save(&a);
                            if current_key.as_ref() == Some(&key) {
                                show_lyrics(&w, &a, index);
                            }
                        }
                        Err(_) => {
                            if current_key.as_ref() == Some(&key) {
                                w.set_lyrics_enrichment_status(
                                    if request.translation {
                                        "Переклад недоступний. Спробуйте оновити слова."
                                    } else {
                                        "Романізація недоступна. Спробуйте оновити слова."
                                    }
                                    .into(),
                                );
                            }
                        }
                    }
                });
            }
        }
        let display = crate::braccato::visible_layers(
            &enriched,
            a.store.settings.lyrics_romanization,
            a.store.settings.lyrics_translation,
        );
        let alt = &display;
        w.set_lyrics_credits(alt.lyrics.songwriters.join(" · ").into());
        w.set_lyrics_timed(crate::braccato::effective_sync_type(alt) != LyricsSyncType::Plain);
        w.set_lyrics_word_timed(matches!(
            crate::braccato::effective_sync_type(alt),
            LyricsSyncType::Word | LyricsSyncType::RichSync
        ));
        w.set_provider_index(index as i32);
        w.set_providers(model(
            a.alternatives
                .iter()
                .map(|a| a.provider.clone().into())
                .collect(),
        ));
        w.set_lyrics_status(alt.provider.clone().into());
        let duration_ms = (a
            .current
            .as_ref()
            .map(|t| t.duration_secs)
            .unwrap_or(0.)
            .max(0.)
            * 1000.) as i64;
        w.set_lyrics_document(crate::braccato::web_document(alt, duration_ms).into());
        w.set_lyric_lines(model(crate::braccato::rows(alt, duration_ms)));
    }
}
fn request_lyrics(w: &MainWindow, a: &Rc<RefCell<WebApp>>) {
    request_lyrics_with_refresh(w, a, false);
}
fn request_lyrics_with_refresh(w: &MainWindow, a: &Rc<RefCell<WebApp>>, refresh: bool) {
    let Some(track) = a.borrow().current.clone() else {
        return;
    };
    let generation = {
        let mut state = a.borrow_mut();
        state.lyric_generation += 1;
        state.lyric_generation
    };
    let mut provider = a.borrow().store.lyrics_request_provider(&track);
    let mut supplement = false;
    let cached = { a.borrow().store.cached_lyrics(&track.path) };
    if let Some(alts) = cached.filter(|_| !refresh && !matches!(provider.as_str(), "betterlyrics" | "kugou")) {
        let should_supplement = provider == "auto" && crate::braccato::providers::should_supplement_kugou(&alts);
        let mut state = a.borrow_mut();
        state.alternatives = alts;
        if should_supplement {
            if state.kugou_supplement_attempts.len() >= 1024 {
                state.kugou_supplement_attempts.clear();
            }
            let identity = state.store.identity_key(&track);
            supplement = state.kugou_supplement_attempts.insert(identity);
        }
        drop(state);
        show_preferred_lyrics(w, &a.borrow());
        if !supplement { return; }
        provider = "kugou".into();
    }
    let weak = w.as_weak();
    let state = a.clone();
    spawn_local(async move {
        let result = reqwest::Client::new()
            .post(api("/api/lyrics"))
            .query(&[("provider", provider), ("refresh", (refresh || supplement).to_string())])
            .json(&track)
            .send()
            .await;
        if let Some(w) = weak.upgrade() {
            let a = state.borrow();
            if a.current.as_ref().map(|t| &t.path) != Some(&track.path)
                || a.lyric_generation != generation
            {
                return;
            }
            match result {
                Ok(r) if r.status().is_success() => {
                    drop(a);
                    match r.json::<Vec<LyricsAlternative>>().await {
                        Ok(alts) => {
                            let mut a = state.borrow_mut();
                            if a.current.as_ref().map(|t| &t.path) == Some(&track.path)
                                && a.lyric_generation == generation
                            {
                                let alts = crate::braccato::providers::merge_refreshed_alternatives(alts, &a.alternatives);
                                a.store.cache_lyrics(track.path.clone(), alts.clone());
                                a.alternatives = alts;
                                save(&a);
                                show_preferred_lyrics(&w, &a);
                            }
                        }
                        Err(_) if !supplement => w.set_lyrics_status("Помилка формату слів".into()),
                        Err(_) => {},
                    }
                }
                _ if !supplement => w.set_lyrics_status("Слова недоступні. Можна вставити власні.".into()),
                _ => {},
            }
        }
    });
}
fn start(w: &MainWindow, a: &Rc<RefCell<WebApp>>, track: Track) {
    let mut state = a.borrow_mut();
    state.playback_generation = state.playback_generation.wrapping_add(1);
    state.playback_recorded = false;
    let generation = state.playback_generation;
    let src = track
        .video_id
        .as_ref()
        .map(|id| format!("/api/audio/{id}"))
        .unwrap_or_else(|| {
            if track.path.starts_with("browser://") || track.path.starts_with("cloud://") {
                file_url(&track.path)
            } else {
                track.path.clone()
            }
        });
    state.audio.set_src(&src);
    state.audio.set_volume(state.store.settings.volume as f64);
    state
        .audio
        .set_playback_rate(state.store.settings.rate as f64);
    let play = state.audio.play();
    state.ended = false;
    state.current = Some(track.clone());
    state.alternatives.clear();
    w.set_status("".into());
    w.set_current_track_key(track.path.clone().into());
    refresh_queue_view(w, &state);
    w.set_current_video_id(track.video_id.clone().unwrap_or_default().into());
    w.set_current_clip_video_id(track.clip_video_id.clone().unwrap_or_default().into());
    w.set_current_title(track.title.clone().into());
    w.set_current_artist(track.artist.clone().into());
    w.set_current_album(track.album.clone().into());
    w.set_current_liked(state.store.is_liked(&track));
    w.set_current_disliked(state.store.is_disliked(&track));
    w.set_current_downloaded(!file_url(&track.path).is_empty());
    w.set_current_local(!file_url(&track.path).is_empty());
    w.set_current_art(state.artwork.get(&track.path).cloned().unwrap_or_default());
    set_media_metadata(
        &track.title,
        &track.artist,
        &track.album,
        &track.thumbnail_url,
        track.duration_secs,
    );
    w.set_lyric_lines(model(vec![]));
    w.set_lyrics_document("".into());
    w.set_lyrics_enrichment_status("".into());
    w.set_lyrics_offset(*state.store.offsets.get(&track.path).unwrap_or(&0) as i32);
    drop(state);
    load_artwork(w, a, &track);
    let weak = w.as_weak();
    let shared = Rc::downgrade(a);
    if let Ok(promise) = play {
        spawn_local(async move {
            if let Err(error) = JsFuture::from(promise).await {
                let aborted = js_sys::Reflect::get(&error, &JsValue::from_str("name"))
                    .ok()
                    .and_then(|name| name.as_string())
                    .as_deref()
                    == Some("AbortError");
                if aborted
                    || !shared
                        .upgrade()
                        .is_some_and(|a| a.borrow().playback_generation == generation)
                {
                    return;
                }
                if let Some(w) = weak.upgrade() {
                    w.set_status(
                        "Не вдалося почати відтворення. Натисніть кнопку відтворення.".into(),
                    );
                }
            }
        });
    }
    request_lyrics(w, a);
}
fn search(w: &MainWindow, a: &Rc<RefCell<WebApp>>, query: Option<String>, radio: bool) {
    let mut state = a.borrow_mut();
    state.generation += 1;
    let generation = state.generation;
    let is_search = query.is_some();
    let weak = w.as_weak();
    let shared = a.clone();
    let cache_key = query.clone();
    let cached = query.as_ref().and_then(|q| state.search_cache.get(q));
    w.set_status("Завантаження з YouTube Music…".into());
    let url = if let Some(q) = query {
        format!("/api/search?q={}", js_sys::encode_uri_component(&q))
    } else if radio {
        format!(
            "/api/browse?radio=1&v={}",
            state
                .current
                .as_ref()
                .and_then(|t| t.video_id.as_deref())
                .unwrap_or("")
        )
    } else {
        "/api/browse".into()
    };
    drop(state);
    spawn_local(async move {
        let result = if let Some(rows) = cached {
            Ok(rows)
        } else {
            match reqwest::get(api(&url)).await {
                Ok(r) if r.status().is_success() => {
                    r.json::<Vec<Track>>().await.map_err(|e| e.to_string())
                }
                Ok(r) => Err(format!("HTTP {}", r.status())),
                Err(e) => Err(e.to_string()),
            }
        };
        if let Some(w) = weak.upgrade() {
            let mut a = shared.borrow_mut();
            if a.generation != generation {
                return;
            }
            match result {
                Ok(rows) => {
                    if let Some(key) = cache_key {
                        a.search_cache.put(&key, rows.clone());
                    }
                    if is_search {
                        a.visible = crate::services::deduplicate_tracks(
                            a.visible.clone().into_iter().chain(rows.iter().cloned()),
                        );
                    } else {
                        a.visible = crate::services::deduplicate_tracks(rows.clone());
                    }
                    refresh(&w, &mut a);
                    w.set_status("".into());
                }
                Err(_) => w.set_status(
                    "YouTube Music недоступний. Запустіть Vey --web-server для мережевих функцій."
                        .into(),
                ),
            }
        }
    });
}

fn request_home_recommendations(w: &MainWindow, state: &Rc<RefCell<WebApp>>) {
    let (generation, profile) = {
        let mut app = state.borrow_mut();
        if app.home_recommendation_loading {
            return;
        }
        app.home_recommendation_generation = app.home_recommendation_generation.wrapping_add(1);
        app.home_recommendation_loading = true;
        (
            app.home_recommendation_generation,
            crate::home::recommendation_profile(&app.store, app.current.as_ref()),
        )
    };
    w.set_home_recommendation_loading(true);
    w.set_home_recommendation_failed(false);
    let weak = w.as_weak();
    let shared = state.clone();
    spawn_local(async move {
        let result = async {
            let blueberry = crate::home::blueberry_candidates().await;
            let resolved = join_all(profile.seeds.iter().cloned().map(|track| async move {
                if let Some(id) = crate::services::track_video_id(&track) {
                    return Some(id.to_string());
                }
                let query = crate::home::recommendation_query(&track)?;
                let search_url = format!("/api/match?q={}", js_sys::encode_uri_component(&query));
                let response = reqwest::get(api(&search_url)).await.ok()?;
                let response = response.error_for_status().ok()?;
                let candidates = response.json::<Vec<Track>>().await.ok()?;
                crate::home::select_public_seed(&track, candidates)
            }))
            .await;
            let mut seen_seeds = HashSet::new();
            let seed_ids: Vec<_> = resolved
                .into_iter()
                .flatten()
                .filter(|id| seen_seeds.insert(id.clone()))
                .collect();

            let mut batches = join_all(seed_ids.into_iter().map(|id| async move {
                let response = reqwest::get(api(&format!("/api/browse?radio=1&v={id}")))
                    .await
                    .ok()?
                    .error_for_status()
                    .ok()?;
                response.json::<Vec<Track>>().await.ok()
            }))
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

            if !blueberry.is_empty() { batches.insert(0, blueberry); }
            let mut tracks = crate::home::rank_recommendations(batches, &profile, 12);
            let personalized = !profile.seeds.is_empty() && !tracks.is_empty();
            if tracks.is_empty() {
                let response = reqwest::get(api("/api/browse?radio=1"))
                    .await
                    .map_err(|error| error.to_string())?
                    .error_for_status()
                    .map_err(|error| error.to_string())?;
                let general = response
                    .json::<Vec<Track>>()
                    .await
                    .map_err(|error| error.to_string())?;
                tracks = crate::home::rank_recommendations(vec![general], &profile, 12);
            }
            Ok::<_, String>((tracks, personalized))
        }
        .await;
        let Some(w) = weak.upgrade() else {
            return;
        };
        let mut app = shared.borrow_mut();
        if app.home_recommendation_generation != generation {
            return;
        }
        app.home_recommendation_loading = false;
        app.home_recommendation_loaded = true;
        w.set_home_recommendation_loading(false);
        let tracks = match result {
            Ok((tracks, personalized)) if !tracks.is_empty() => {
                app.store.recommendations.updated_at = crate::services::now_ms();
                app.store.recommendations.personalized = personalized;
                app.store.recommendations.tracks = tracks.clone();
                save(&app);
                app.home_recommendations = tracks;
                let current = app.current.as_ref().map(|track| track.path.clone());
                app.artwork_queue
                    .retain(|track| current.as_ref() == Some(&track.path));
                w.set_home_recommendation_failed(false);
                w.set_home_recommendation_personalized(personalized);
                app.home_recommendations.clone()
            }
            Ok(_) | Err(_) => {
                if app.home_recommendations.is_empty() {
                    let tracks = crate::home::local_recommendations(&app.store, 12);
                    if tracks.is_empty() {
                        w.set_home_recommendation_failed(true);
                        vec![]
                    } else {
                        app.home_recommendations = tracks;
                        app.store.recommendations.updated_at = crate::services::now_ms();
                        app.store.recommendations.personalized = false;
                        app.store.recommendations.tracks = app.home_recommendations.clone();
                        save(&app);
                        w.set_home_recommendation_personalized(false);
                        w.set_home_recommendation_failed(false);
                        app.home_recommendations.clone()
                    }
                } else {
                    w.set_home_recommendation_failed(false);
                    app.home_recommendations.clone()
                }
            }
        };
        refresh(&w, &mut app);
        drop(app);
        for track in tracks {
            load_artwork(&w, &shared, &track);
        }
    });
}

#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let w = MainWindow::new().map_err(|e| JsValue::from_str(&e.to_string()))?;
    w.set_app_version(env!("CARGO_PKG_VERSION").into());
    crate::i18n::install(&w);
    crate::player_themes::install(&w);
    crate::renderer::install(&w);
    crate::navigation_fx::install(&w);
    let _lyrics_widget = crate::lyrics_widget::install(&w);
    let video_surface = crate::video::install(&w);
    std::mem::forget(video_surface);
    apply_viewport_insets(&w);
    if let Some(browser) = web_sys::window() {
        let width = browser.inner_width()?.as_f64().unwrap_or(1024.);
        let height = browser.inner_height()?.as_f64().unwrap_or(768.);
        w.window()
            .set_size(slint::LogicalSize::new(width as f32, height as f32));
        w.set_mobile(width < 800. || height < 500.);
    }

    let store = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("playervf.rust.v1").ok().flatten())
        .and_then(|s| serde_json::from_str::<Store>(&s).ok())
        .unwrap_or_default();
    let saved_recommendations = store.recommendations.tracks.clone();
    let saved_library = store.library_tracks();
    let a = Rc::new(RefCell::new(WebApp {
        library: saved_library,
        store,
        library_view: LibraryViewState::default(),
        page_results: HashMap::new(),
        visible: vec![],
        queue: Queue::default(),
        download_pending: HashSet::new(),
        download_progress: HashMap::new(),
        download_ids: HashMap::new(),
        download_callbacks: HashMap::new(),
        current: None,
        playlist: String::new(),
        audio: web_sys::HtmlAudioElement::new()?,
        alternatives: vec![],
        enrichment_attempts: RefCell::default(),
        kugou_supplement_attempts: HashSet::new(),
        self_ref: Default::default(),
        artwork: HashMap::new(),
        artwork_pending: HashSet::new(),
        artwork_queue: VecDeque::new(),
        artwork_failed: HashMap::new(),
        generation: 0,
        home_recommendation_generation: 0,
        home_recommendation_loading: false,
        home_recommendation_loaded: false,
        home_recommendations: saved_recommendations,
        search_index: Default::default(),
        search_cache: Default::default(),
        pending_search: None,
        lyric_generation: 0,
        playback_generation: 0,
        playback_recorded: false,
        ended: false,
        save_due: Cell::new(None),
        media_handler: None,
    }));
    a.borrow_mut().self_ref = Rc::downgrade(&a);
    let media_state = Rc::downgrade(&a);
    let media_window = w.as_weak();
    let media_handler = Closure::<dyn FnMut(String, f64)>::new(move |action: String, value: f64| {
        let (Some(state), Some(ui)) = (media_state.upgrade(), media_window.upgrade()) else { return; };
        match action.as_str() {
            "play" => {
                if state.borrow().current.is_none() { ui.invoke_select_track(0); }
                else { let _ = state.borrow().audio.play(); }
            }
            "pause" => { let _ = state.borrow().audio.pause(); }
            "nexttrack" | "previoustrack" => {
                let next = {
                    let mut app = state.borrow_mut();
                    let previous = action == "previoustrack";
                    let next = app.queue.advance(previous, false);
                    if !previous && next.is_some() {
                        if let Some(current) = app.current.clone() {
                            let position = app.audio.current_time();
                            let duration = app.audio.duration();
                            app.store.record_skip_if_early(&current, position, duration);
                            save(&app);
                        }
                    }
                    next
                };
                if let Some(track) = next { start(&ui, &state, track); }
            }
            "seekto" => state.borrow().audio.set_current_time(value.max(0.)),
            "seekbackward" | "seekforward" => {
                let audio = state.borrow().audio.clone();
                audio.set_current_time((audio.current_time() + if action == "seekforward" { value.max(10.) } else { -value.max(10.) }).max(0.));
            }
            _ => {}
        }
    });
    install_media_session(media_handler.as_ref().unchecked_ref());
    a.borrow_mut().media_handler = Some(media_handler);
    let flush_state = Rc::downgrade(&a);
    let flush = Closure::<dyn FnMut()>::new(move || {
        if let Some(state) = flush_state.upgrade() {
            flush_save(&state.borrow());
        }
    });
    on_store_flush(flush.as_ref().unchecked_ref());
    flush.forget();
    {
        let a = a.borrow();
        crate::preferences::apply(&w, &a.store.settings);
        w.set_volume(a.store.settings.volume);
        w.set_color_mode(a.store.settings.color_mode.as_str().into());
        w.set_color_hue(a.store.settings.color_hue as f32);
        w.set_art_mode(a.store.settings.art_mode.clone().into());
        w.set_rate(a.store.settings.rate);
        w.set_lyrics_provider(a.store.settings.lyrics_provider.clone().into());
        w.set_lyrics_stage(a.store.settings.lyrics_stage);
    }
    macro_rules! bind{($method:ident,|$ui:ident,$state:ident $(,$arg:ident)*|$body:block)=>{{let weak=w.as_weak();let shared=a.clone();w.$method(move|$($arg),*|{if let Some($ui)=weak.upgrade(){let $state=&shared;$body}});}};}
    bind!(on_search_edited, |ui, state, query| {
        ui.set_status("".into());
        let mut a = state.borrow_mut();
        a.generation += 1;
        a.pending_search = None;
        if ui.get_active_view() == "search" {
            let WebApp {
                search_index,
                library,
                ..
            } = &mut *a;
            let rows = search_index.find(library, &query);
            a.visible = rows;
            refresh(&ui, &mut a);
            if query.trim().chars().count() >= 2 {
                a.pending_search = Some((web_time::Instant::now(), query.to_string()));
            }
        }
    });
    bind!(on_navigate, |ui, state, view| {
        if view != "explore" {
            ui.set_catalog_kind("".into());
            ui.set_catalog_subtitle("".into());
            ui.set_catalog_art(slint::Image::default());
            ui.set_catalog_track_count(0);
        }
        if view == "explore" && !ui.get_catalog_title().is_empty() {
            ui.set_catalog_title("".into());
            ui.set_catalog_kind("".into());
            ui.set_catalog_subtitle("".into());
            ui.set_catalog_art(slint::Image::default());
            ui.set_catalog_track_count(0);
            state.borrow_mut().page_results.remove("explore");
        }
        state.borrow_mut().generation += 1;
        state.borrow_mut().pending_search = None;
        if view == "home" && ui.get_home_rotate() {
            ui.invoke_home_choose(-1);
        }
        let cached = state.borrow().page_results.get(view.as_str()).cloned();
        if matches!(view.as_str(), "search" | "explore") {
            state.borrow_mut().visible = cached.clone().unwrap_or_default();
        }
        ui.set_active_view(view.clone());
        if view == "home" && !state.borrow().home_recommendation_loaded {
            request_home_recommendations(&ui, state);
        }
        if view == "explore" && cached.is_none() {
            search(&ui, state, None, false);
        } else {
            refresh(&ui, &mut state.borrow_mut());
        }
    });
    bind!(on_refresh_explore, |ui, state| {
        ui.set_catalog_title("".into());
        search(&ui, state, None, false);
    });
    bind!(on_request_discovery_art, |ui, state, json| {
        if let Ok(track) = serde_json::from_str::<Track>(&json) {
            load_artwork(&ui, state, &track);
        }
    });
    bind!(on_play_discovery_items, |ui, state, json, video| {
        let Ok(selection) = serde_json::from_str::<crate::discovery::Selection>(&json) else {
            return;
        };
        let track = {
            let mut app = state.borrow_mut();
            app.queue.select(selection.tracks, selection.index)
        };
        if let Some(track) = track {
            if video && ui.get_art_mode() != "clip" {
                ui.invoke_change_art_mode();
            }
            start(&ui, state, track);
            if video {
                ui.set_active_view("lyrics".into());
            }
        }
    });
    bind!(on_open_discovery_collection, |ui, state, id, json| {
        let Ok(context) = serde_json::from_str::<crate::discovery::Item>(&json) else {
            return;
        };
        if ui.get_active_view() == "explore" && ui.get_catalog_kind() == "artist" && !ui.get_catalog_current_id().is_empty() {
            ui.set_catalog_parent_id(ui.get_catalog_current_id());
            ui.set_catalog_parent_json(ui.get_catalog_current_json());
        } else {
            ui.set_catalog_parent_id("".into());
            ui.set_catalog_parent_json("".into());
        }
        ui.set_catalog_current_id(id.clone());
        ui.set_catalog_current_json(json.clone());
        ui.set_catalog_from_home(true);
        ui.set_catalog_shelves(slint::ModelRc::default());
        state.borrow_mut().store.artist_page_catalog = crate::discovery::Catalog::default();
        if id.is_empty() {
            let mut app = state.borrow_mut();
            if let Some(tracks) = context.local_collection(&app.store) {
                app.generation = app.generation.wrapping_add(1);
                app.visible = tracks;
                ui.set_catalog_title(context.title.clone().into());
                ui.set_catalog_kind(context.kind.clone().into());
                ui.set_catalog_subtitle(if context.kind == "album" { context.artist.clone() } else { String::new() }.into());
                ui.set_catalog_art(app.visible.first().and_then(|track| app.artwork.get(&track.path)).cloned().unwrap_or_default());
                ui.set_catalog_track_count(app.visible.len() as i32);
                ui.set_active_view("explore".into());
                ui.set_status("".into());
                refresh(&ui, &mut app);
            }
            return;
        }
        ui.set_catalog_title(context.title.clone().into());
        ui.set_catalog_kind(if context.kind == "artist" || context.kind == "album" { context.kind.clone() } else { String::new() }.into());
        ui.set_catalog_subtitle(if context.kind == "album" { context.artist.clone() } else { String::new() }.into());
        ui.set_catalog_track_count(0);
        ui.set_catalog_art(state.borrow().artwork.get(&context.path).cloned().unwrap_or_default());
        ui.set_active_view("explore".into());
        let generation = {
            let mut app = state.borrow_mut();
            app.generation += 1;
            app.visible.clear();
            refresh(&ui, &mut app);
            app.generation
        };
        ui.set_status("Завантаження з YouTube Music…".into());
        let weak = ui.as_weak();
        let shared = state.clone();
        spawn_local(async move {
            let result = async {
                let artist = context.kind == "artist" && id.starts_with("UC");
                let response = reqwest::get(api(&format!(
                    "{}?id={}", if artist { "/api/artist" } else { "/api/browse" },
                    js_sys::encode_uri_component(&id)
                )))
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
                let mut page = if artist {
                    response.json::<crate::discovery::ArtistPage>().await.map_err(|e| e.to_string())?
                } else {
                    let tracks = response.json::<Vec<Track>>().await.map_err(|e| e.to_string())?;
                    crate::discovery::ArtistPage { tracks, ..Default::default() }
                };
                page.tracks = context.collection_tracks(page.tracks);
                Ok::<_, String>(page)
            }
            .await;
            if let Some(ui) = weak.upgrade() {
                let mut app = shared.borrow_mut();
                if app.generation != generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        let catalog = page.catalog;
                        app.visible = page.tracks;
                        ui.set_catalog_track_count(app.visible.len() as i32);
                        app.store.artist_page_catalog = catalog.clone();
                        crate::discovery::sync_artist_page(&ui, &catalog);
                        refresh(&ui, &mut app);
                        ui.set_status("".into());
                        drop(app);
                        crate::discovery::request_artist_art(&ui, &catalog);
                    }
                    Err(_) => {
                        ui.set_status("Не вдалося завантажити добірки. Спробуйте ще раз.".into())
                    }
                }
            }
        });
    });
    bind!(on_request_home_recommendations, |ui, state| {
        request_home_recommendations(&ui, state);
    });
    bind!(on_select_home_recommendation, |ui, state, index| {
        let track = {
            let mut app = state.borrow_mut();
            app.queue.shuffle = app.store.settings.shuffle;
            app.queue.repeat = app.store.settings.repeat.clone();
            let recommendations = app.home_recommendations.clone();
            app.queue.select(recommendations, index.max(0) as usize)
        };
        if let Some(track) = track {
            start(&ui, state, track);
        }
    });
    bind!(on_play_home_recommendations, |ui, _state| {
        ui.invoke_select_home_recommendation(0);
    });
    bind!(on_search, |ui, state, q| {
        ui.set_active_view("search".into());
        let query = q.trim().to_lowercase();
        {
            let mut app = state.borrow_mut();
            app.generation += 1;
            app.pending_search = None;
            let WebApp {
                search_index,
                library,
                ..
            } = &mut *app;
            let rows = search_index.find(library, &query);
            app.visible = rows;
            refresh(&ui, &mut app);
        }
        if query.is_empty() {
            ui.set_status("".into());
        } else {
            search(&ui, state, Some(q.to_string()), false);
        }
    });
    bind!(on_select_track, |ui, state, index| {
        let track = {
            let mut a = state.borrow_mut();
            let tracks = a.visible.clone();
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            a.queue.select(tracks, index.max(0) as usize)
        };
        if let Some(t) = track {
            start(&ui, state, t);
        }
    });
    bind!(on_select_queue_track, |ui, state, index| {
        let track = {
            let mut a = state.borrow_mut();
            let queue = a.queue.tracks.clone();
            a.queue.select(queue, index.max(0) as usize)
        };
        if let Some(track) = track { start(&ui, state, track); }
    });
    bind!(on_play_toggle, |ui, state| {
        let a = state.borrow();
        if a.current.is_none() {
            drop(a);
            ui.invoke_select_track(0);
        } else if a.audio.paused() {
            let _ = a.audio.play();
        } else {
            let _ = a.audio.pause();
        }
    });
    bind!(on_previous_track, |ui, state| {
        if state.borrow().audio.current_time() > 3. {
            state.borrow().audio.set_current_time(0.);
        } else {
            let t = state.borrow_mut().queue.advance(true, false);
            if let Some(t) = t {
                start(&ui, state, t);
            }
        }
    });
    bind!(on_next_track, |ui, state| {
        let t = {
            let mut app = state.borrow_mut();
            let next = app.queue.advance(false, false);
            if next.is_some() {
                if let Some(current) = app.current.clone() {
                    let position = app.audio.current_time();
                    let duration = app.audio.duration();
                    app.store.record_skip_if_early(&current, position, duration);
                    save(&app);
                }
            }
            next
        };
        if let Some(t) = t {
            start(&ui, state, t);
        }
    });
    bind!(on_seek_to, |_ui, state, millis| {
        state
            .borrow()
            .audio
            .set_current_time((millis as f64 / 1000.).max(0.));
    });
    bind!(on_volume_changed, |_ui, state, value| {
        let mut a = state.borrow_mut();
        a.store.settings.volume = value.clamp(0., 1.);
        a.audio.set_volume(a.store.settings.volume as f64);
        save(&a);
    });
    bind!(on_preferences_changed, |ui, state| {
        let mut a = state.borrow_mut();
        let old_language = a.store.settings.language.clone();
        let layers = (
            a.store.settings.lyrics_romanization,
            a.store.settings.lyrics_translation,
            a.store.settings.lyrics_translation_language.clone(),
            a.store.settings.language.clone(),
        );
        crate::preferences::capture(&ui, &mut a.store.settings);
        if layers
            != (
                a.store.settings.lyrics_romanization,
                a.store.settings.lyrics_translation,
                a.store.settings.lyrics_translation_language.clone(),
                a.store.settings.language.clone(),
            )
        {
            show_lyrics(&ui, &a, ui.get_provider_index().max(0) as usize);
        }
        if old_language != a.store.settings.language {
            refresh(&ui, &mut a);
        }
        save(&a);
    });
    bind!(on_appearance_changed, |ui, state, mode, hue| {
        let mut a = state.borrow_mut();
        a.store.settings.color_mode = ColorMode::from_name(&mode);
        a.store.settings.color_hue = (hue.round() as u16) % 360;
        ui.set_color_mode(a.store.settings.color_mode.as_str().into());
        ui.set_color_hue(a.store.settings.color_hue as f32);
        save(&a);
    });
    bind!(on_toggle_like, |ui, state, index| {
        let mut a = state.borrow_mut();
        let t = if index < 0 {
            a.current.clone()
        } else {
            a.visible.get(index as usize).cloned()
        };
        if let Some(t) = t {
            a.store.remember(t.clone());
            a.store.toggle_like_track(&t);
            let liked = a.store.is_liked(&t);
            crate::track_model::update_path(&ui, &t.path, |row| row.liked = liked);
            if a.current
                .as_ref()
                .is_some_and(|current| current.path == t.path)
            {
                ui.set_current_liked(liked);
                ui.set_current_disliked(a.store.is_disliked(&t));
            }
            save(&a);
            if ui.get_active_view() == "liked" {
                refresh(&ui, &mut a);
            }
        }
    });
    bind!(on_toggle_dislike, |ui, state, index| {
        let mut app = state.borrow_mut();
        let track = if index < 0 {
            app.current.clone()
        } else {
            app.visible.get(index as usize).cloned()
        };
        if let Some(track) = track {
            app.store.remember(track.clone());
            app.store.toggle_dislike_track(&track);
            let liked = app.store.is_liked(&track);
            let disliked = app.store.is_disliked(&track);
            crate::track_model::update_path(&ui, &track.path, |row| row.liked = liked);
            if app.current.as_ref().is_some_and(|current| current.path == track.path) {
                ui.set_current_disliked(disliked);
                ui.set_current_liked(liked);
            }
            let refresh_home = ui.get_active_view() == "home";
            if refresh_home { app.home_recommendation_loaded = false; }
            save(&app);
            drop(app);
            if refresh_home { request_home_recommendations(&ui, &state); }
        }
    });
    bind!(on_toggle_shuffle, |ui, state| {
        let mut a = state.borrow_mut();
        a.store.settings.shuffle = !a.store.settings.shuffle;
        a.queue.shuffle = a.store.settings.shuffle;
        ui.set_shuffle(a.store.settings.shuffle);
        save(&a);
    });
    bind!(on_toggle_repeat, |ui, state| {
        let mut a = state.borrow_mut();
        a.store.settings.repeat = match a.store.settings.repeat.as_str() {
            "off" => "all",
            "all" => "one",
            _ => "off",
        }
        .into();
        a.queue.repeat = a.store.settings.repeat.clone();
        ui.set_repeat_mode(a.store.settings.repeat.clone().into());
        save(&a);
    });
    bind!(on_add_source, |ui, state, _kind| {
        let weak = ui.as_weak();
        let shared = state.clone();
        spawn_local(async move {
            if let Ok(json) = JsFuture::from(pick_files()).await {
                if let Some(w) = weak.upgrade() {
                    if let Ok(tracks) =
                        serde_json::from_str::<Vec<Track>>(&json.as_string().unwrap_or_default())
                    {
                        let mut a = shared.borrow_mut();
                        a.store.remember_all(tracks);
                        a.library = a.store.library_tracks();
                        w.set_active_view("library".into());
                        refresh(&w, &mut a);
                        save(&a);
                    }
                }
            }
        });
    });
    bind!(on_scan, |ui, state| {
        refresh(&ui, &mut state.borrow_mut());
    });
    bind!(on_library_view_changed, |ui, state| {
        let mut a = state.borrow_mut();
        a.library_view.query = ui.get_library_query().to_string();
        a.library_view.filter = LibraryFilter::from_index(ui.get_library_filter_index());
        a.library_view.sort = LibrarySort::from_index(ui.get_library_sort_index());
        a.library_view.category = LibraryCategory::from_index(ui.get_library_category_index());
        refresh(&ui, &mut a);
    });
    bind!(on_open_library_collection, |ui, state, kind, key| {
        let mut a = state.borrow_mut();
        if kind == "playlist" {
            a.playlist = key.to_string();
            if let Some(playlist) = a.store.playlists.iter().find(|playlist| playlist.id == key.as_str()) {
                ui.set_playlist_title(playlist.name.clone().into());
                ui.set_active_view("playlist".into());
                refresh(&ui, &mut a);
            }
            return;
        }
        let source = if a.library_view.filter == LibraryFilter::Excluded {
            a.store.excluded_library_tracks()
        } else {
            a.library.clone()
        };
        let Some(collection) = a.library_view.collections(&a.store, &source, |track| {
            !file_url(&track.path).is_empty()
                || a.store.downloads.contains_key(&track.path)
                || a.store.downloads.contains_key(&a.store.identity_key(track))
        })
            .into_iter()
            .find(|collection| collection.kind == kind.as_str() && collection.key == key.as_str())
        else { return; };
        a.visible = collection.tracks.clone();
        ui.set_catalog_from_home(false);
        ui.set_catalog_parent_id("".into());
        ui.set_catalog_parent_json("".into());
        ui.set_catalog_title(collection.title.clone().into());
        ui.set_catalog_kind(collection.kind.clone().into());
        ui.set_catalog_subtitle(collection.subtitle.clone().into());
        ui.set_catalog_art(a.artwork.get(&collection.representative.path).cloned().unwrap_or_default());
        ui.set_catalog_track_count(collection.track_count as i32);
        ui.set_active_view("explore".into());
        ui.set_status("".into());
        refresh(&ui, &mut a);
    });
    bind!(on_library_collection_action, |ui, state, action| {
        let mut a = state.borrow_mut();
        if a.visible.is_empty() { return; }
        let mut tracks = a.visible.clone();
        if action == "queue" {
            a.queue.append(tracks);
            refresh(&ui, &mut a);
            ui.set_status("Добірку додано до черги".into());
        } else {
            if action == "shuffle" { tracks = shuffle_tracks(tracks); }
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            let first = a.queue.select(tracks, 0);
            drop(a);
            if let Some(track) = first { start(&ui, &state, track); }
        }
    });
    bind!(on_library_track_action, |ui, state, index, action| {
        let mut a = state.borrow_mut();
        let Some(track) = a.visible.get(index.max(0) as usize).cloned() else { return; };
        match action.as_str() {
            "like" => {
                a.store.toggle_like_track(&track);
                save(&a);
                refresh(&ui, &mut a);
            }
            "queue" | "next" => {
                if action == "next" {
                    if a.current.is_none() {
                        a.queue.insert_next([track]);
                        a.queue.index = Some(0);
                        let first = a.queue.tracks.first().cloned();
                        drop(a);
                        if let Some(first) = first { start(&ui, &state, first); }
                        return;
                    }
                    a.queue.insert_next([track]);
                } else {
                    a.queue.append([track]);
                }
                refresh(&ui, &mut a);
                ui.set_status(if action == "next" { "Композицію поставлено наступною" } else { "Композицію додано до черги" }.into());
            }
            "add" => ui.set_dialog("add".into()),
            "download" => { drop(a); ui.invoke_download(index); }
            "relink" => { drop(a); ui.invoke_relink_track(index); }
            "remove" => {
                a.library_view.selected.clear();
                let key = a.store.identity_key(&track);
                a.library_view.selected.insert(key);
                refresh(&ui, &mut a);
                ui.set_dialog("confirm-library-remove".into());
            }
            _ => {}
        }
    });
    bind!(on_toggle_library_selection, |ui, state, index| {
        let mut a = state.borrow_mut();
        if let Some(track) = selected(&a, index) {
            let key = a.store.identity_key(&track);
            if !a.library_view.selected.remove(&key) { a.library_view.selected.insert(key); }
            refresh(&ui, &mut a);
        }
    });
    bind!(on_library_bulk_action, |ui, state, action| {
        let mut a = state.borrow_mut();
        let selected = selected_library_tracks(&a);
        if action == "select-all" {
            let keys: Vec<_> = a.visible.iter().map(|track| a.store.identity_key(track)).collect();
            a.library_view.selected.extend(keys);
        } else if action == "clear-selection" {
            a.library_view.selected.clear();
        } else if action == "playlist" {
            ui.set_dialog("bulk-add".into());
        } else if action == "remove" {
            ui.set_dialog("confirm-library-remove".into());
        } else if action == "confirm-remove" || action == "restore" {
            let excluded = action == "confirm-remove";
            for track in &selected {
                if !crate::services::key_is_remote(&track.path) { a.store.set_library_excluded(track, excluded); }
            }
            a.library_view.selected.clear();
            a.library = a.store.library_tracks();
            save(&a);
            ui.set_status(if excluded { "Композиції прибрано з Vey. Файли залишилися на пристрої" } else { "Композиції повернуто до бібліотеки" }.into());
            refresh(&ui, &mut a);
        } else if action == "like" {
            let all_liked = !selected.is_empty() && selected.iter().all(|track| a.store.is_liked(track));
            for track in &selected {
                if a.store.is_liked(track) != !all_liked { a.store.toggle_like_track(track); }
            }
            save(&a);
            refresh(&ui, &mut a);
        } else if action == "queue" || action == "next" {
            let start_next = action == "next" && a.current.is_none() && !selected.is_empty();
            if action == "next" { a.queue.insert_next(selected); } else { a.queue.append(selected); }
            let next = if start_next {
                a.queue.index = Some(0);
                a.queue.tracks.first().cloned()
            } else { None };
            if let Some(track) = next {
                drop(a);
                start(&ui, &state, track);
                return;
            }
            refresh(&ui, &mut a);
            ui.set_status("Композиції додано до черги".into());
        } else if action == "play" && !selected.is_empty() {
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            let track = a.queue.select(selected, 0);
            drop(a);
            if let Some(track) = track { start(&ui, &state, track); }
            return;
        }
    });
    bind!(on_add_selected_to_playlist, |ui, state, id| {
        let mut a = state.borrow_mut();
        let selected = selected_library_tracks(&a);
        for track in selected { a.store.playlist_add_track(&id, &track); }
        a.library_view.selected.clear();
        save(&a);
        ui.set_dialog("".into());
        refresh(&ui, &mut a);
    });
    bind!(on_queue_track, |ui, state, index, play_next| {
        let mut a = state.borrow_mut();
        if let Some(track) = selected(&a, index) {
            if play_next {
                if a.current.is_none() {
                    a.queue.insert_next([track]);
                    a.queue.index = Some(0);
                    let first = a.queue.tracks.first().cloned();
                    drop(a);
                    if let Some(first) = first { start(&ui, &state, first); }
                    return;
                }
                a.queue.insert_next([track]);
            } else {
                a.queue.append([track]);
            }
            refresh(&ui, &mut a);
            ui.set_status(if play_next { "Додано наступною композицією" } else { "Додано до черги" }.into());
        }
    });
    bind!(on_move_list_item, |ui, state, index, delta| {
        let mut a = state.borrow_mut();
        let from = index.max(0) as usize;
        let to = (from as i32 + delta).max(0) as usize;
        let moved = match ui.get_active_view().as_str() {
            "queue" => a.queue.move_item(from, to),
            "playlist" => {
                let id = a.playlist.clone();
                let moved = a.store.move_playlist_track(&id, from, to);
                if moved {
                    if let Some(playlist) = a.store.playlists.iter().find(|item| item.id == id) {
                        let keys: Vec<_> = a.queue.tracks.iter().map(|track| a.store.identity_key(track)).collect();
                        if keys == playlist.tracks { a.queue.move_item(from, to); }
                    }
                }
                moved
            }
            _ => false,
        };
        if moved { save(&a); refresh(&ui, &mut a); }
    });
    bind!(on_remove_queue_item, |ui, state, index| {
        let mut a = state.borrow_mut();
        if a.queue.remove(index.max(0) as usize).is_some() { refresh(&ui, &mut a); }
    });
    bind!(on_clear_queue, |ui, state| {
        let mut a = state.borrow_mut();
        a.queue.clear();
        refresh(&ui, &mut a);
        ui.set_status("Чергу очищено. Поточне відтворення триває.".into());
    });
    bind!(on_create_playlist, |ui, state, name| {
        let mut a = state.borrow_mut();
        if let Some(id) = a.store.create_playlist(&name) {
            if ui.get_bulk_adding_to_new_playlist() {
                let selected = selected_library_tracks(&a);
                for track in selected { a.store.playlist_add_track(&id, &track); }
                a.library_view.selected.clear();
            } else if ui.get_adding_to_new_playlist() {
                let pending_index = ui.get_library_action_index();
                let selected_track = if pending_index >= 0 {
                    a.visible.get(pending_index as usize).cloned()
                } else {
                    a.current.clone()
                };
                if let Some(track) = selected_track {
                    a.store.remember(track.clone());
                    a.store.playlist_add_track(&id, &track);
                }
            }
        }
        ui.set_library_action_index(-1);
        save(&a);
        refresh(&ui, &mut a);
    });
    bind!(on_rename_playlist, |ui, state, name| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        if a.store.rename_playlist(&id, &name) {
            ui.set_playlist_title(name.trim().into());
            save(&a);
            if ui.get_account_signed_in() { ui.invoke_account_command("push".into()); }
            refresh(&ui, &mut a);
        } else {
            ui.set_status("Введіть назву плейлиста".into());
        }
    });
    bind!(on_open_playlist, |ui, state, id| {
        let mut a = state.borrow_mut();
        a.playlist = id.to_string();
        ui.set_active_view("playlist".into());
        if let Some(p) = a.store.playlists.iter().find(|p| p.id == a.playlist) {
            ui.set_playlist_title(p.name.clone().into());
        }
        refresh(&ui, &mut a);
    });
    bind!(on_play_playlist, |ui, state, id| {
        let track = {
            let mut a = state.borrow_mut();
            a.playlist = id.to_string();
            ui.set_active_view("playlist".into());
            refresh(&ui, &mut a);
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            let visible = a.visible.clone();
            a.queue.select(visible, 0)
        };
        if let Some(track) = track { start(&ui, &state, track); }
    });
    bind!(on_add_to_playlist, |ui, state, id| {
        let mut a = state.borrow_mut();
        let pending_index = ui.get_library_action_index();
        let selected_track = if pending_index >= 0 {
            a.visible.get(pending_index as usize).cloned()
        } else {
            a.current.clone()
        };
        if let Some(t) = selected_track {
            a.store.playlist_add_track(&id, &t);
            save(&a);
            refresh(&ui, &mut a);
        }
        ui.set_library_action_index(-1);
    });
    bind!(on_remove_from_playlist, |ui, state, index| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        if let Some(t) = a.visible.get(index.max(0) as usize).cloned() {
            let identity = a.store.identity_key(&t);
            if let Some(p) = a.store.playlists.iter_mut().find(|p| p.id == id) {
                let position = index.max(0) as usize;
                if p.tracks.get(position).is_some_and(|path| path == &t.path || path == &identity) {
                    p.tracks.remove(position);
                    p.updated_at = crate::services::now_ms();
                }
            }
            save(&a);
            refresh(&ui, &mut a);
        }
    });
    bind!(on_delete_playlist, |ui, state| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        a.store.playlists.retain(|p| p.id != id);
        save(&a);
        ui.set_active_view("library".into());
        refresh(&ui, &mut a);
    });
    bind!(on_set_playlist_public, |ui, state, is_public| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        if let Some(p) = a.store.playlists.iter_mut().find(|p| p.id == id) {
            p.is_public = is_public;
            p.updated_at = crate::services::now_ms();
            save(&a);
            refresh(&ui, &mut a);
            if ui.get_account_signed_in() {
                if ui.get_account_busy() {
                    ui.set_status("Saved. Sync the library after the current account operation finishes".into());
                } else {
                    ui.invoke_account_command("push".into());
                    ui.set_status(if is_public { "Playlist published; library sync started".into() } else { "Playlist hidden; library sync started".into() });
                }
            } else {
                ui.set_status(if is_public { "Playlist marked public. Connect your Vey account to show it on your profile".into() } else { "Playlist hidden from your profile".into() });
            }
        }
    });
    bind!(on_request_lyrics, |ui, state| {
        request_lyrics(&ui, state);
    });
    bind!(on_refresh_lyrics, |ui, state| {
        {
            let mut a = state.borrow_mut();
            a.enrichment_attempts.borrow_mut().clear();
            if let Some(alt) = a.alternatives.get(ui.get_provider_index().max(0) as usize) {
                if let Some(request) = crate::lyrics_translation::Request::for_song(
                    alt,
                    &a.store.settings,
                    a.current.as_ref().and_then(|t| t.video_id.clone()),
                ) {
                    a.store.lyrics_enrichment.remove(&request.key());
                }
            }
            show_lyrics(&ui, &a, ui.get_provider_index().max(0) as usize);
        }
        let current = { state.borrow().current.clone() };
        if let Some(t) = current {
            state.borrow_mut().store.lyrics_cache.remove(&t.path);
        }
        request_lyrics_with_refresh(&ui, state, true);
    });
    bind!(on_choose_provider, |ui, state, index| {
        let mut a = state.borrow_mut();
        let index = index.max(0) as usize;
        if let (Some(track), Some(alt)) = (a.current.clone(), a.alternatives.get(index).cloned()) {
            a.store.remember_lyrics_choice(&track, &alt);
            save(&a);
            show_lyrics(&ui, &a, index);
        }
    });
    bind!(on_adjust_offset, |ui, state, delta| {
        let mut a = state.borrow_mut();
        if let Some(t) = a.current.clone() {
            let v = a.store.offsets.entry(t.path).or_default();
            *v = (*v + delta as i64).clamp(-60000, 60000);
            ui.set_lyrics_offset(*v as i32);
            save(&a);
        }
    });
    bind!(on_import_lyrics, |ui, state, raw| {
        let mut a = state.borrow_mut();
        let Some((mut lyrics, format, sync_type)) = parse_payload(&raw) else {
            ui.set_status("Не вдалося розібрати слова пісні".into());
            return;
        };
        if sync_type != LyricsSyncType::Plain {
            complete_timing(
                &mut lyrics,
                (a.current
                    .as_ref()
                    .map(|t| t.duration_secs)
                    .unwrap_or(0.)
                    .max(0.)
                    * 1000.) as u64,
            );
        }
        a.lyric_generation += 1;
        a.alternatives = vec![LyricsAlternative {
            provider: "Власні слова".into(),
            sync_type,
            format: format.into(),
            lyrics,
        }];
        if let Some(track) = a.current.clone() {
            let alts = a.alternatives.clone();
            a.store.remember_lyrics_choice(&track, &alts[0]);
            a.store.cache_lyrics(track.path, alts);
            save(&a);
        }
        show_lyrics(&ui, &a, 0);
    });
    bind!(on_copy_lyrics, |_ui, state| {
        let a = state.borrow();
        if let Some(alt) = a.alternatives.get(_ui.get_provider_index().max(0) as usize) {
            let text = crate::lyrics::serialize_lrc(&alt.lyrics);
            if let Some(w) = web_sys::window() {
                let _ = w.navigator().clipboard().write_text(&text);
            }
        }
    });
    bind!(
        on_lyric_mode_changed,
        |_ui, state, stage, reduced, passive| {
            let mut a = state.borrow_mut();
            a.store.settings.lyrics_stage = stage;
            a.store.settings.lyrics_reduced_motion = reduced;
            a.store.settings.lyrics_passive_scroll = passive;
            save(&a);
        }
    );

    crate::lyric_appearance::install(&w);
    bind!(on_apply_lyric_style, |ui, state| {
        if let Err(error) = crate::lyric_appearance::draft(&ui) {
            ui.set_lyric_style_status(error.into());
            ui.set_lyric_draft_valid(false);
            return;
        }
        let mut a = state.borrow_mut();
        a.store.settings.lyrics_theme_css = ui.get_lyric_theme_source().to_string();
        a.store.settings.lyrics_font_family = ui.get_lyric_font_family().to_string();
        a.store.settings.theme_entire_player = ui.get_theme_entire_player_draft();
        crate::themes::apply_settings(&ui, &a.store.settings);
        save(&a);
        ui.set_dialog("".into());
    });
    bind!(on_provider_changed, |ui, state, value| {
        state.borrow_mut().store.settings.lyrics_provider = value.to_string();
        save(&state.borrow());
        request_lyrics_with_refresh(&ui, state, true);
    });
    bind!(on_radio, |ui, state| {
        ui.set_active_view("explore".into());
        search(&ui, state, None, true);
    });
    bind!(on_open_login, |_ui, _state| {
        if let Some(w) = web_sys::window() {
            let _ = w.open_with_url_and_target("https://music.youtube.com/", "_blank");
        }
    });
    bind!(on_backup, |_ui, state| {
        if let Ok(json) = serde_json::to_string_pretty(&state.borrow().store) {
            download_json(&json);
        }
    });
    bind!(on_migrate, |ui, state| {
        let weak = ui.as_weak();
        let shared = state.clone();
        spawn_local(async move {
            if let Ok(json) = JsFuture::from(read_json()).await {
                if let Some(w) = weak.upgrade() {
                    let mut a = shared.borrow_mut();
                    match a.store.migrate(&json.as_string().unwrap_or_default()) {
                        Ok(n) => {
                            save(&a);
                            refresh(&w, &mut a);
                            w.set_status(format!("Перенесено {n} записів").into());
                        }
                        Err(e) => w.set_status(e.into()),
                    }
                }
            }
        });
    });
    bind!(on_restore, |ui, state| {
        let weak = ui.as_weak();
        let shared = state.clone();
        spawn_local(async move {
            if let Ok(json) = JsFuture::from(read_json()).await {
                if let Some(w) = weak.upgrade() {
                    match serde_json::from_str::<Store>(&json.as_string().unwrap_or_default()) {
                        Ok(s) if s.version == 1 => {
                            let mut a = shared.borrow_mut();
                            a.store = s;
                            crate::preferences::apply(&w, &a.store.settings);
                            w.set_color_mode(a.store.settings.color_mode.as_str().into());
                            w.set_color_hue(a.store.settings.color_hue as f32);
                            w.set_lyrics_reduced_motion(a.store.settings.lyrics_reduced_motion);
                            save(&a);
                            refresh(&w, &mut a);
                        }
                        _ => w.set_status("Некоректний файл резервної копії".into()),
                    }
                }
            }
        });
    });
    bind!(on_clear_history, |ui, state| {
        let mut a = state.borrow_mut();
        a.store.history.clear();
        save(&a);
        refresh(&ui, &mut a);
    });
    bind!(on_submit_auth, |ui, state, token| {
        let weak = ui.as_weak();
        let state = state.clone();
        spawn_local(async move {
            let ok = reqwest::Client::new()
                .post(api("/api/verify"))
                .json(&serde_json::json!({"token": token.to_string()}))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success());
            if let Some(w) = weak.upgrade() {
                w.set_jwt_status(
                    if ok {
                        "JWT чинний"
                    } else {
                        "Не вдалося перевірити token"
                    }
                    .into(),
                );
                if ok {
                    request_lyrics_with_refresh(&w, &state, true);
                }
            }
        });
    });
    bind!(on_open_challenge, |ui, state| {
        let weak = ui.as_weak();
        let state = state.clone();
        spawn_local(async move {
            let result = JsFuture::from(challenge()).await;
            let success = if let Ok(token) = result {
                reqwest::Client::new()
                    .post(api("/api/verify"))
                    .json(&serde_json::json!({"token":token.as_string().unwrap_or_default()}))
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
            } else {
                false
            };
            if let Some(w) = weak.upgrade() {
                if success {
                    request_lyrics_with_refresh(&w, &state, true);
                }
                w.set_jwt_status(
                    if success {
                        "JWT чинний"
                    } else {
                        "Не вдалося пройти перевірку"
                    }
                    .into(),
                );
            }
        });
    });
    bind!(on_change_art_mode, |ui, state| {
        let mut a = state.borrow_mut();
        a.store.settings.art_mode = if ui.get_art_mode() == "cover" {
            "clip"
        } else {
            "cover"
        }
        .into();
        ui.set_art_mode(a.store.settings.art_mode.clone().into());
        save(&a);
    });
    bind!(on_relink_track, |ui, state, index| {
        let old = selected(&state.borrow(), index);
        let Some(old) = old.filter(|track| !crate::services::key_is_remote(&track.path)) else { return; };
        let old_path = old.path.clone();
        let weak = ui.as_weak();
        let shared = state.clone();
        spawn_local(async move {
            let picked = JsFuture::from(pick_single_file()).await;
            let json = picked.ok().and_then(|value| value.as_string()).unwrap_or_default();
            if json.is_empty() { return; }
            let replacement = match serde_json::from_str::<Track>(&json) {
                Ok(track) => track,
                Err(error) => {
                    if let Some(ui) = weak.upgrade() { ui.set_status(format!("Не вдалося прочитати вибраний файл: {error}").into()); }
                    return;
                }
            };
            if let Some(ui) = weak.upgrade() {
                let mut app = shared.borrow_mut();
                let old_key = app.store.identity_key(&old);
                if app.store.relink_local_path(&old_path, replacement.clone()) {
                    if old_path != replacement.path { unlink_file(&old_path); }
                    let new_key = app.store.identity_key(&replacement);
                    if app.library_view.selected.remove(&old_key) {
                        app.library_view.selected.insert(new_key);
                    }
                    for queued in &mut app.queue.tracks {
                        if queued.path == old_path { *queued = replacement.clone(); }
                    }
                    app.library = app.store.library_tracks();
                    save(&app);
                    ui.set_status("Файл повторно прив’язано; збережені списки й уподобання залишилися".into());
                    refresh(&ui, &mut app);
                    drop(app);
                    load_artwork(&ui, &shared, &replacement);
                } else {
                    ui.set_status("Не вдалося знайти цей запис у бібліотеці".into());
                }
            }
        });
        ui.set_status("Виберіть файл для відновлення композиції".into());
    });
    bind!(on_download, |ui, state, index| {
        let track = selected(&state.borrow(), index);
        let Some(track) = track else { return; };
        let path = track.path.clone();
        let active_download = { state.borrow().download_ids.get(&path).cloned() };
        if let Some(id) = active_download {
            cancel_download(&id);
            let mut app = state.borrow_mut();
            app.download_pending.remove(&path);
            app.download_progress.remove(&path);
            app.download_ids.remove(&path);
            app.download_callbacks.remove(&path);
            crate::track_model::update_path(&ui, &path, |row| { row.downloading = false; row.download_progress = -1; });
            if app.current.as_ref().is_some_and(|current| current.path == path) {
                ui.set_current_downloading(false);
                ui.set_current_download_progress(-1);
            }
            ui.set_status("Завантаження скасовано".into());
            return;
        }
        let Some(id) = track.video_id.clone() else {
            ui.set_status("Файл уже доступний локально".into());
            return;
        };
        let filename = format!("{} - {}.m4a", track.artist, track.title);
        let weak = ui.as_weak();
        let shared = state.clone();
        let progress_path = path.clone();
        let progress_weak = ui.as_weak();
        let progress_state = state.clone();
        let callback = Closure::<dyn FnMut(f64)>::new(move |value| {
            let Some(ui) = progress_weak.upgrade() else { return; };
            let mut app = progress_state.borrow_mut();
            if !app.download_pending.contains(&progress_path) { return; }
            let progress = if value >= 0.0 { (value as i32).clamp(0, 100) } else { -1 };
            if app.download_progress.get(&progress_path).copied().unwrap_or(-1) != progress {
                app.download_progress.insert(progress_path.clone(), progress);
                crate::track_model::update_path(&ui, &progress_path, |row| { row.downloading = true; row.download_progress = progress; });
                if app.current.as_ref().is_some_and(|current| current.path == progress_path) {
                    ui.set_current_downloading(true);
                    ui.set_current_download_progress(progress);
                }
            }
        });
        let promise = download_track(&id, &filename, callback.as_ref().unchecked_ref());
        {
            let mut app = state.borrow_mut();
            app.download_pending.insert(path.clone());
            app.download_progress.insert(path.clone(), -1);
            app.download_ids.insert(path.clone(), id);
            app.download_callbacks.insert(path.clone(), callback);
        }
        crate::track_model::update_path(&ui, &path, |row| { row.downloading = true; row.download_progress = -1; });
        if state.borrow().current.as_ref().is_some_and(|current| current.path == path) {
            ui.set_current_downloading(true);
            ui.set_current_download_progress(-1);
        }
        ui.set_status("Підготовка завантаження…".into());
        spawn_local(async move {
            let result = JsFuture::from(promise).await;
            if let Some(ui) = weak.upgrade() {
                let mut app = shared.borrow_mut();
                let was_pending = app.download_pending.remove(&path);
                app.download_progress.remove(&path);
                app.download_ids.remove(&path);
                app.download_callbacks.remove(&path);
                crate::track_model::update_path(&ui, &path, |row| { row.downloading = false; row.download_progress = -1; });
                if app.current.as_ref().is_some_and(|current| current.path == path) {
                    ui.set_current_downloading(false);
                    ui.set_current_download_progress(-1);
                }
                if was_pending {
                    if result.is_ok() {
                        ui.set_status("Файл передано браузеру для збереження".into());
                    } else {
                        let error = result.err().and_then(|error| error.as_string()).unwrap_or_else(|| "невідома помилка".into());
                        ui.set_status(format!("Завантаження не вдалося: {error}").into());
                    }
                }
            }
        });
    });

    bind!(on_add_path, |ui, _state, _kind, _path| {
        ui.set_status("У браузері виберіть файли кнопкою «Додати файли»".into());
    });
    bind!(on_sync_tags, |ui, _state| {
        ui.set_status("Редагування тегів доступне у нативній версії".into());
    });
    bind!(on_restore_tags, |ui, _state| {
        ui.set_status("Відновлення тегів доступне у нативній версії".into());
    });
    let account_get = {
        let state = a.clone();
        Rc::new(move || crate::account::snapshot(&state.borrow().store))
    };
    let account_apply = {
        let state = a.clone();
        let weak = w.as_weak();
        Rc::new(move |cloud: Store| {
            if let Some(ui) = weak.upgrade() {
                let mut state = state.borrow_mut();
                state.store.tracks = cloud.tracks;
                state.store.likes = cloud.likes;
                state.store.playlists = cloud.playlists;
                state.store.history = cloud.history;
                let newer_recommendations = cloud.recommendations.updated_at > state.store.recommendations.updated_at;
                state.store.recommendations = cloud.recommendations;
                if newer_recommendations {
                    state.home_recommendation_generation = state.home_recommendation_generation.wrapping_add(1);
                    state.home_recommendation_loading = false;
                    state.home_recommendation_loaded = !state.store.recommendations.tracks.is_empty();
                    ui.set_home_recommendation_loading(false);
                }
                state.home_recommendations = state.store.recommendations.tracks.clone();
                ui.set_home_recommendation_personalized(state.store.recommendations.personalized);
                state.store.offsets = cloud.offsets;
                state.store.lyrics_choices = cloud.lyrics_choices;
                state.store.settings = cloud.settings;
                state.store.downloads = cloud.downloads;
                state.store.migrate_identities();
                state.library = state.store.library_tracks();
                crate::preferences::apply(&ui, &state.store.settings);
                ui.set_volume(state.store.settings.volume);
                ui.set_color_mode(state.store.settings.color_mode.as_str().into());
                ui.set_color_hue(state.store.settings.color_hue as f32);
                ui.set_art_mode(state.store.settings.art_mode.clone().into());
                ui.set_shuffle(state.store.settings.shuffle);
                ui.set_repeat_mode(state.store.settings.repeat.clone().into());
                ui.set_lyrics_provider(state.store.settings.lyrics_provider.clone().into());
                ui.set_lyrics_stage(state.store.settings.lyrics_stage);
                ui.set_lyrics_reduced_motion(state.store.settings.lyrics_reduced_motion);
                ui.set_lyrics_passive_scroll(state.store.settings.lyrics_passive_scroll);
                ui.invoke_artwork_theme();
                state.audio.set_volume(state.store.settings.volume as f64);
                refresh(&ui, &mut state);
                save(&state);
            }
        })
    };
    crate::home::install(&w, account_get.clone(), account_apply.clone());
    let discovery_get = {
        let state = a.clone();
        Rc::new(move || state.borrow().store.clone())
    };
    let discovery_apply = {
        let state = a.clone();
        let weak = w.as_weak();
        Rc::new(move |next: Store| {
            if let Some(ui) = weak.upgrade() {
                let mut app = state.borrow_mut();
                app.store.home_catalog = next.home_catalog;
                save(&app);
                refresh(&ui, &mut app);
            }
        })
    };
    crate::discovery::install(&w, discovery_get, discovery_apply);
    let account_timer = crate::account::install(&w, account_get, account_apply);
    std::mem::forget(account_timer);
    let search_timer = slint::Timer::default();
    let weak_search = w.as_weak();
    let search_state = a.clone();
    search_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let Some(ui) = weak_search.upgrade() else {
                return;
            };
            let query = {
                let mut a = search_state.borrow_mut();
                if a.pending_search
                    .as_ref()
                    .is_some_and(|(at, _)| at.elapsed() >= Duration::from_millis(280))
                {
                    a.pending_search.take().map(|(_, q)| q)
                } else {
                    None
                }
            };
            if let Some(query) = query {
                if ui.get_active_view() == "search" {
                    search(&ui, &search_state, Some(query), false);
                }
            }
        },
    );
    std::mem::forget(search_timer);
    refresh(&w, &mut a.borrow_mut());
    let saved = a.borrow().home_recommendations.clone();
    w.set_home_recommendation_personalized(a.borrow().store.recommendations.personalized);
    for track in &saved { load_artwork(&w, &a, track); }
    a.borrow_mut().home_recommendation_loaded = !saved.is_empty();
    if a.borrow().store.recommendations.updated_at.saturating_add(3_600_000) <= crate::services::now_ms() || saved.is_empty() {
        request_home_recommendations(&w, &a);
    }
    let recommendation_timer = slint::Timer::default();
    let recommendation_state = a.clone();
    let recommendation_window = w.as_weak();
    recommendation_timer.start(slint::TimerMode::Repeated, Duration::from_secs(60), move || {
        if let Some(ui) = recommendation_window.upgrade() {
            if recommendation_state.borrow().store.recommendations.updated_at.saturating_add(3_600_000) <= crate::services::now_ms() {
                request_home_recommendations(&ui, &recommendation_state);
            }
        }
    });
    std::mem::forget(recommendation_timer);
    let weak = w.as_weak();
    let shared = a.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            if let Some(w) = weak.upgrade() {
                if let Some(browser) = web_sys::window() {
                    let width = browser
                        .inner_width()
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(1024.);
                    let height = browser
                        .inner_height()
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(768.);
                    let size = w.window().size().to_logical(w.window().scale_factor());
                    if (size.width as f64 - width).abs() > 1.
                        || (size.height as f64 - height).abs() > 1.
                    {
                        w.window()
                            .set_size(slint::LogicalSize::new(width as f32, height as f32));
                        w.set_mobile(width < 800. || height < 500.);
                        apply_viewport_insets(&w);
                    }
                }

                let mut a = shared.borrow_mut();
                if a.save_due
                    .get()
                    .is_some_and(|due| web_time::Instant::now() >= due)
                {
                    flush_save(&a);
                }
                if let Some(error) = a.audio.error() {
                    w.set_status(
                        format!("Помилка аудіо ({}): {}", error.code(), error.message()).into(),
                    );
                }
                let pos = a.audio.current_time();
                let duration = a.audio.duration();
                if !a.playback_recorded && pos > 0.0 && a.audio.error().is_none()
                    && (!a.audio.paused() || a.audio.ended())
                {
                    if let Some(track) = a.current.clone() {
                        a.playback_recorded = true;
                        a.store.record(&track);
                        save(&a);
                    }
                }
                w.set_position(pos as f32);
                if duration.is_finite() {
                    w.set_duration(duration as f32);
                }
                w.set_time_label(
                    format!("{} / {}", format_time(pos), format_time(duration)).into(),
                );
                w.set_player_state(
                    if a.audio.paused() {
                        "paused"
                    } else {
                        "playing"
                    }
                    .into(),
                );
                let duration_value = if duration.is_finite() { duration } else { 0. };
                set_media_playback(!a.audio.paused(), pos, duration_value, a.audio.playback_rate());
                if a.audio.ended() && !a.ended {
                    a.ended = true;
                    if let Some(track) = a.current.clone() {
                        a.store.record_completion(&track);
                        save(&a);
                    }
                    let next = a.queue.advance(false, true);
                    drop(a);
                    if let Some(t) = next {
                        start(&w, &shared, t);
                    }
                }
            }
        },
    );
    // The event loop retains these values on the web.
    std::mem::forget(timer);
    w.run().map_err(|e| JsValue::from_str(&e.to_string()))
}
