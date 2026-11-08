//! Slint callbacks are thin controllers over Rust services.
use crate::audio::AudioEngine;
use crate::{local_sync, lyrics::*, lyrics_service as ls, native as core, services::*, storage, *};
use futures_util::future::join_all;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;
type Shared = Rc<RefCell<App>>;

#[cfg(target_os = "windows")]
struct EmbeddedChallenge {
    webview: wry::WebView,
    _page: crate::challenge::EmbeddedPageServer,
    bounds: wry::Rect,
    deadline: Instant,
}

struct App {
    store: Store,
    visible: Vec<Track>,
    page_results: std::collections::HashMap<String, Vec<Track>>,
    library: Vec<Track>,
    library_view: LibraryViewState,
    collection_jobs: CollectionJobs,
    collection_representatives: Vec<Track>,
    local_paths: std::collections::HashSet<String>,
    liked_sidebar_art_keys: Vec<String>,
    history_sidebar_art_keys: Vec<String>,
    queue: Queue,
    current: Option<Track>,
    playlist: String,
    alternatives: Vec<LyricsAlternative>,
    enrichment_attempts: RefCell<std::collections::HashSet<String>>,
    kugou_supplement_attempts: std::collections::HashSet<String>,
    audio: Arc<core::CoreState>,
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    media_session: Option<crate::media_session::MediaSession>,
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    media_session_retry_at: Option<Instant>,
    rt: Arc<Runtime>,
    tx: mpsc::Sender<Event>,
    loading: bool,
    scanning: bool,
    scan_generation: u64,
    scan_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    source_status: String,
    #[cfg(target_os = "android")]
    permission_wait_started: Option<Instant>,
    search_generation: u64,
    home_recommendation_generation: u64,
    home_recommendation_loading: bool,
    home_recommendation_loaded: bool,
    home_recommendation_task: Option<tokio::task::JoinHandle<()>>,
    home_recommendations: Vec<Track>,
    search_index: crate::search_index::SearchIndex,
    search_cache: crate::search_index::SearchCache,
    search_task: Option<tokio::task::JoinHandle<()>>,
    pending_search: Option<(Instant, String)>,
    lyric_generation: u64,
    lyric_task: Option<tokio::task::JoinHandle<()>>,
    loaded_path: String,
    ended: bool,
    load_generation: Arc<std::sync::atomic::AtomicU64>,
    load_task: Option<tokio::task::JoinHandle<()>>,
    download_pending: std::collections::HashSet<String>,
    download_serial: u64,
    download_generations: std::collections::HashMap<String, u64>,
    download_progress: std::collections::HashMap<String, i32>,
    download_tasks: std::collections::HashMap<String, tokio::task::AbortHandle>,
    load_gate: Arc<std::sync::Mutex<()>>,
    last_playing: Option<(Track, String)>,
    pending_tag_path: Option<String>,
    pending_tag_video_id: Option<String>,
    art: std::collections::HashMap<String, slint::Image>,
    art_pending: std::collections::HashSet<String>,
    art_failed: std::collections::HashMap<String, Instant>,
    art_attempts: std::collections::HashMap<String, u8>,
    art_queue: ArtworkQueue,
    jwt: ls::JwtCache,
    challenge_in_flight: bool,
    #[cfg(target_os = "windows")]
    embedded_challenge: Option<EmbeddedChallenge>,
    snapshot: bool,
    store_revision: Cell<u64>,
    save_due: Cell<Option<Instant>>,
    saver: storage::SaveWorker,
}

/// Keep pending artwork as lightweight data rather than one task per row.
#[derive(Default)]
struct ArtworkQueue {
    entries: std::collections::VecDeque<Track>,
    paths: std::collections::HashSet<String>,
}

/// At most one collection build runs, with one coalesced request behind it.
#[derive(Default)]
struct CollectionJobs {
    generation: u64,
    running: bool,
    pending: bool,
}
impl CollectionJobs {
    fn request(&mut self) -> Option<u64> {
        self.generation = self.generation.wrapping_add(1);
        self.pending = true;
        self.start_pending()
    }
    fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
    }
    fn start_pending(&mut self) -> Option<u64> {
        if self.running || !self.pending {
            return None;
        }
        self.running = true;
        self.pending = false;
        Some(self.generation)
    }
    fn finish(&mut self, generation: u64) -> bool {
        self.running = false;
        generation == self.generation
    }
}
impl ArtworkQueue {
    fn push(&mut self, track: &Track, priority: bool) {
        if self.paths.contains(&track.path) {
            if priority {
                if let Some(index) = self.entries.iter().position(|item| item.path == track.path) {
                    self.entries.remove(index);
                    self.entries.push_front(track.clone());
                }
            }
            return;
        }
        self.paths.insert(track.path.clone());
        if priority {
            self.entries.push_front(track.clone());
        } else {
            self.entries.push_back(track.clone());
        }
    }
    fn pop(&mut self) -> Option<Track> {
        let track = self.entries.pop_front()?;
        self.paths.remove(&track.path);
        Some(track)
    }
    fn retain_paths(&mut self, paths: &std::collections::HashSet<String>) {
        self.entries.retain(|track| paths.contains(&track.path));
        self.paths.retain(|path| paths.contains(path));
    }
}
enum Event {
    Match(String, Result<local_sync::TagSnapshot, String>),
    Scan(u64, Vec<Track>, String, Vec<(String, String)>),
    #[cfg(target_os = "android")]
    ScanFailed(u64, String),
    Search(u64, Option<String>, Result<Vec<Track>, String>),
    Collection(u64, Result<crate::discovery::ArtistPage, String>),
    LibraryCollections(u64, Result<Vec<LibraryCollection>, String>),
    HomeRecommendations(u64, Result<(Vec<Track>, bool), String>),
    Loaded(u64, String, Result<String, String>),
    Downloaded(Track, u64, Result<Option<String>, String>),
    DownloadProgress(String, u64, u64, Option<u64>),
    DownloadLocal(String, u64, String),
    MissingDownload(String, String),
    Relink(String, Result<Track, String>),
    Lyrics(String, u64, Result<Vec<LyricsAlternative>, String>, ls::JwtCache),
    LyricsPreview(String, u64, Vec<LyricsAlternative>),
    LyricsSupplement(String, u64, Result<Vec<LyricsAlternative>, String>),
    Enriched(
        String,
        Result<crate::lyrics_translation::Enrichment, String>,
    ),
    Art(String, Result<image::RgbaImage, String>),
    ArtCached(String),
    RetryArt(String),
    Message(String),
    Migration(u64, Result<(Store, usize), String>),
    Restore(u64, Result<Store, String>),
    Jwt(Result<ls::JwtCache, String>),
    #[cfg(target_os = "windows")]
    OpenEmbeddedChallenge,
    #[cfg(target_os = "windows")]
    ChallengeIpc(crate::challenge::ChallengeSignal),
    #[cfg(target_os = "windows")]
    ChallengeTimeout,
}

#[cfg(target_os = "windows")]
fn challenge_bounds(ui: &MainWindow) -> Option<wry::Rect> {
    use wry::dpi::{LogicalPosition, LogicalSize};

    let x = ui.get_lyrics_auth_x();
    let y = ui.get_lyrics_auth_y();
    let width = ui.get_lyrics_auth_width();
    let height = ui.get_lyrics_auth_height();
    if ![x, y, width, height].into_iter().all(f32::is_finite) || width < 300.0 || height < 280.0 {
        return None;
    }
    Some(wry::Rect {
        position: LogicalPosition::new(x.max(0.0) as f64, y.max(0.0) as f64).into(),
        size: LogicalSize::new(width as f64, height as f64).into(),
    })
}

#[cfg(target_os = "windows")]
fn open_embedded_challenge(ui: &MainWindow, app: &mut App) -> Result<(), String> {
    use wry::WebViewBuilder;

    let bounds = challenge_bounds(ui).ok_or("Не вдалося визначити область слів пісні")?;
    let page = crate::challenge::start_embedded_page()?;
    let url = page.url().to_owned();
    let allowed_url = url.clone();
    let tx = app.tx.clone();
    let builder = WebViewBuilder::new()
        .with_url(&url)
        .with_bounds(bounds)
        .with_background_color((8, 8, 12, 255))
        .with_focused(true)
        .with_navigation_handler(move |target| target == allowed_url)
        .with_ipc_handler(move |request| {
            if request.uri().to_string() != url {
                return;
            }
            if let Some(signal) = crate::challenge::parse_embedded_message(request.body()) {
                let _ = tx.send(Event::ChallengeIpc(signal));
            }
        });
    let host = ui.window().window_handle();
    let webview = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        builder.build_as_child(&host)
    }))
    .map_err(|_| "Вбудоване вікно WebView не запустилося".to_string())?
    .map_err(|error| error.to_string())?;

    app.embedded_challenge = Some(EmbeddedChallenge {
        webview,
        _page: page,
        bounds,
        deadline: Instant::now() + Duration::from_secs(120),
    });
    ui.set_lyrics_auth_loading(false);
    ui.set_jwt_status("Пройдіть перевірку у вікні над текстом пісні".into());
    Ok(())
}

fn start_system_challenge(rt: Arc<Runtime>, tx: mpsc::Sender<Event>) {
    rt.spawn(async move {
        let result = match crate::challenge::acquire().await {
            Ok(token) => {
                let mut jwt = ls::JwtCache::load();
                jwt.exchange(token).await.map(|_| jwt)
            }
            Err(error) => Err(error),
        };
        let _ = tx.send(Event::Jwt(result));
    });
}
fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::from(Rc::new(VecModel::from(rows)))
}
fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}
fn apply_settings(w: &MainWindow, a: &App) {
    let s = &a.store.settings;
    crate::preferences::apply(w, s);
    w.set_volume(s.volume);
    w.set_color_mode(s.color_mode.as_str().into());
    w.set_color_hue(s.color_hue as f32);
    w.set_rate(s.rate);
    w.set_shuffle(s.shuffle);
    w.set_repeat_mode(s.repeat.clone().into());
    w.set_art_mode(s.art_mode.clone().into());
    w.set_lyrics_provider(s.lyrics_provider.clone().into());
    w.set_lyrics_stage(s.lyrics_stage);
    let _ = a.audio.volume(s.volume);
    let _ = a.audio.rate(s.rate);
}
fn persist(_w: &MainWindow, a: &App) {
    a.store_revision.set(a.store_revision.get().wrapping_add(1));
    if !a.snapshot && a.save_due.get().is_none() {
        // Coalesce settings bursts before cloning the store, without disk IO.
        a.save_due
            .set(Some(Instant::now() + Duration::from_millis(200)));
    }
}

fn art_keys(a: &App, t: &Track) -> Vec<String> {
    let mut keys = vec![t.path.clone()];
    let identity = a.store.identity_key(t);
    if !keys.contains(&identity) {
        keys.push(identity.clone());
    }
    if let Some(local) = local_file(a, t) {
        if !keys.contains(&local) {
            keys.push(local);
        }
    }
    keys
}
fn art_image(a: &App, t: &Track) -> slint::Image {
    art_keys(a, t)
        .into_iter()
        .find_map(|key| a.art.get(&key).cloned())
        .unwrap_or_default()
}
fn row_art_image(a: &App, track: &Track) -> slint::Image {
    a.art
        .get(&track.path)
        .cloned()
        .or_else(|| track_video_id(track).and_then(|id| a.art.get(&format!("yt://{id}")).cloned()))
        .unwrap_or_default()
}
fn row(a: &App, t: &Track, local_paths: &std::collections::HashSet<String>) -> TrackRow {
    let local_key = normalize_path(&t.path);
    let local = !crate::services::key_is_remote(&t.path);
    let downloaded = a.store.downloads.contains_key(&t.path) || local_paths.contains(&local_key);
    TrackRow {
        title: t.title.clone().into(),
        artist: t.artist.clone().into(),
        album: t.album.clone().into(),
        duration: fmt_time(t.duration_secs).into(),
        path: t.path.clone().into(),
        art: row_art_image(a, t),
        liked: a.store.is_liked(t),
        downloaded,
        downloading: a.download_pending.contains(&t.path),
        download_progress: a.download_progress.get(&t.path).copied().unwrap_or(-1),
        selected: !a.library_view.selected.is_empty()
            && a.library_view.selected.contains(&a.store.identity_key(t)),
        missing: local && !downloaded,
    }
}
fn is_downloaded(a: &App, path: &str) -> bool {
    a.local_paths.contains(&normalize_path(path))
        || a.store.downloads.contains_key(path)
        || a.store
            .downloads
            .values()
            .any(|p| normalize_path(p) == normalize_path(path))
}
fn is_downloaded_in(a: &App, local_paths: &std::collections::HashSet<String>, path: &str) -> bool {
    a.store.downloads.contains_key(path) || local_paths.contains(&normalize_path(path))
}
fn local_file(a: &App, track: &Track) -> Option<String> {
    let key = a.store.identity_key(track);
    a.store
        .downloads
        .get(&track.path)
        .or_else(|| a.store.downloads.get(&key))
        .cloned()
        .or_else(|| (!crate::services::key_is_remote(&track.path)).then(|| track.path.clone()))
}
/// Deliver one completed cover to its retained rows without rebuilding pages.
fn refresh_media_art_row(w: &MainWindow, a: &App, path: &str) {
    if let Some(art) = a.art.get(path) {
        crate::track_model::update_path(w, path, |row| row.art = art.clone());
    }
}

/// Update one retained row after a cover or download event. Background work
/// can produce many small events; never rescan every mounted page for each one.
fn refresh_media_row(w: &MainWindow, a: &App, path: &str) {
    let downloaded = is_downloaded(a, path);
    let downloading = a.download_pending.contains(path);
    let progress = a.download_progress.get(path).copied().unwrap_or(-1);
    let art = a.art.get(path).cloned();
    crate::track_model::update_path(w, path, |row| {
        row.downloaded = downloaded;
        row.downloading = downloading;
        row.download_progress = progress;
        if let Some(art) = &art {
            row.art = art.clone();
        }
    });
    if a.current.as_ref().is_some_and(|track| track.path == path) {
        w.set_current_downloaded(downloaded);
        w.set_current_downloading(downloading);
        w.set_current_download_progress(progress);
        if let Some(track) = a.current.as_ref() {
            w.set_current_local(local_file(a, track).is_some());
        }
    }
}

fn build_library_collections(
    view: LibraryViewState,
    store: Store,
    library: Vec<Track>,
    local_paths: std::collections::HashSet<String>,
) -> Vec<LibraryCollection> {
    let source = if view.filter == LibraryFilter::Excluded {
        store.excluded_library_tracks()
    } else {
        library
    };
    let downloaded_paths: std::collections::HashSet<_> = store
        .downloads
        .values()
        .map(|path| normalize_path(path))
        .collect();
    view.collections(&store, &source, |track| {
        let path = normalize_path(&track.path);
        local_paths.contains(&path)
            || store.downloads.contains_key(&track.path)
            || downloaded_paths.contains(&path)
    })
}

fn start_collection_job(a: &mut App, generation: u64) {
    let view = LibraryViewState {
        query: a.library_view.query.clone(),
        filter: a.library_view.filter,
        sort: a.library_view.sort,
        category: a.library_view.category,
        selected: Default::default(),
    };
    let needs_catalog =
        view.filter == LibraryFilter::Excluded || view.category == LibraryCategory::Playlists;
    // Snapshot only the data used by collection projection. No Slint object
    // crosses the worker boundary, and lyrics/art caches are not cloned.
    let store = Store {
        tracks: if needs_catalog {
            a.store.tracks.clone()
        } else {
            Vec::new()
        },
        likes: a.store.likes.clone(),
        playlists: a.store.playlists.clone(),
        history: a.store.history.clone(),
        downloads: a.store.downloads.clone(),
        library_excluded: a.store.library_excluded.clone(),
        ..Store::default()
    };
    let library = if view.filter == LibraryFilter::Excluded {
        Vec::new()
    } else {
        a.library.clone()
    };
    let local_paths = a.local_paths.clone();
    let tx = a.tx.clone();
    a.rt.spawn_blocking(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            build_library_collections(view, store, library, local_paths)
        }))
        .map_err(|_| "Не вдалося підготувати колекції медіатеки.".to_string());
        let _ = tx.send(Event::LibraryCollections(generation, result));
    });
}

fn refresh(w: &MainWindow, a: &mut App) {
    w.set_home_revision(w.get_home_revision().wrapping_add(1));
    // Build the local-file reverse lookup once, rather than once per row.
    let mut local_paths = a.local_paths.clone();
    local_paths.extend(a.store.downloads.values().map(|p| normalize_path(p)));
    let view = w.get_active_view().to_string();
    a.visible = match view.as_str() {
        "home" => {
            if a.store.history.is_empty() {
                a.library.clone()
            } else {
                a.store.history.iter().map(|h| h.track.clone()).collect()
            }
        }
        "library" => {
            if matches!(
                a.library_view.category,
                LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists
            ) {
                Vec::new()
            } else {
                let source = if a.library_view.filter == LibraryFilter::Excluded {
                    a.store.excluded_library_tracks()
                } else {
                    a.library.clone()
                };
                a.library_view.apply(&a.store, &source, |track| {
                    is_downloaded_in(a, &local_paths, &track.path)
                })
            }
        }
        "liked" => a
            .store
            .library_tracks()
            .into_iter()
            .filter(|t| a.store.is_liked(t))
            .collect(),
        "playlist" => a.store.playlist_tracks(&a.playlist),
        "queue" => a.queue.tracks.clone(),
        _ => a.visible.clone(),
    };
    if !a.snapshot {
        // Keep queued covers for this page. Repeated refreshes (likes,
        // downloads, sorting) must not restart the visible artwork backlog.
        // Drop queued work for tracks that are no longer part of the view.
        let mut artwork_paths: std::collections::HashSet<String> =
            a.visible.iter().map(|track| track.path.clone()).collect();
        if view == "home" {
            artwork_paths.extend(
                a.home_recommendations
                    .iter()
                    .map(|track| track.path.clone()),
            );
        }
        if let Some(track) = &a.current {
            artwork_paths.insert(track.path.clone());
        }
        a.art_queue.retain_paths(&artwork_paths);
        if let Some(track) = a.current.clone() {
            queue_art(a, &track, true);
        }
        if view == "home" {
            for track in a.home_recommendations.clone() {
                load_art(a, &track);
            }
        }
        let visible = std::mem::take(&mut a.visible);
        for track in &visible {
            load_art(a, track);
        }
        a.visible = visible;
        let playlist_covers: Vec<Track> = a
            .store
            .playlists
            .iter()
            .filter_map(|p| {
                let first = p.tracks.first()?;
                a.store
                    .tracks
                    .iter()
                    .find(|t| &t.path == first || a.store.identity_key(t) == *first)
                    .cloned()
                    .or_else(|| {
                        first.strip_prefix("yt://").map(|id| Track {
                            path: first.clone(),
                            video_id: Some(id.to_string()),
                            ..Default::default()
                        })
                    })
            })
            .collect();
        for track in playlist_covers {
            load_art(a, &track);
        }
    }
    if matches!(view.as_str(), "search" | "explore") {
        a.page_results.insert(view.clone(), a.visible.clone());
    }
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
    w.set_library_scanning(a.scanning);
    w.set_library_source_status(a.source_status.clone().into());
    refresh_queue_view(w, a);
    let selected_tracks: Vec<_> = if a.library_view.selected.is_empty() {
        Vec::new()
    } else {
        a.library
            .iter()
            .filter(|track| {
                a.library_view
                    .selected
                    .contains(&a.store.identity_key(track))
            })
            .collect()
    };
    w.set_library_selection_all_liked(
        !selected_tracks.is_empty() && selected_tracks.iter().all(|track| a.store.is_liked(track)),
    );
    if view == "library"
        && matches!(
            a.library_view.category,
            LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists
        )
    {
        w.set_library_collections_loading(true);
        if let Some(generation) = a.collection_jobs.request() {
            start_collection_job(a, generation);
        }
    } else {
        a.collection_jobs.cancel();
        w.set_library_collections_loading(false);
        if view == "library" {
            a.collection_representatives.clear();
            w.set_library_collections(model(Vec::new()));
        }
    }
    // Seed all local page objects without navigating or starting network work.
    // Only bind the page being shown. Navigation refreshes the destination
    // before travel starts, so eagerly constructing every hidden page here
    // only duplicates large row conversions during startup/library scans.
    crate::track_model::set_tracks(
        w,
        a.visible.iter().map(|t| row(a, t, &local_paths)).collect(),
    );
    refresh_playlists(w, a);
    if let Some(t) = &a.current {
        w.set_current_liked(a.store.is_liked(t));
        w.set_current_disliked(a.store.is_disliked(t));
        w.set_current_downloaded(is_downloaded(a, &t.path));
        w.set_current_downloading(a.download_pending.contains(&t.path));
        w.set_current_download_progress(a.download_progress.get(&t.path).copied().unwrap_or(-1));
        w.set_current_local(local_file(a, t).is_some());
        w.set_current_art(art_image(a, t));
        w.set_current_album(t.album.clone().into());
    }
}
fn playlist_track_lookup(a: &App) -> std::collections::HashMap<String, usize> {
    let mut by_key = std::collections::HashMap::with_capacity(
        a.store.tracks.len().saturating_add(a.store.downloads.len()),
    );
    for (index, track) in a.store.tracks.iter().enumerate() {
        by_key.entry(track.path.clone()).or_insert(index);
        if let Some(id) = track_video_id(track) {
            by_key.entry(format!("yt://{id}")).or_insert(index);
        }
    }
    for (key, path) in &a.store.downloads {
        if let Some(index) = by_key.get(path).copied() {
            by_key.entry(key.clone()).or_insert(index);
        }
    }
    by_key
}

fn playlist_cover(
    a: &App,
    playlist: &Playlist,
    by_key: &std::collections::HashMap<String, usize>,
) -> slint::Image {
    let Some(first) = playlist.tracks.first() else {
        return slint::Image::default();
    };
    if let Some(art) = a.art.get(first).cloned() {
        return art;
    }
    let Some(track) = by_key
        .get(first)
        .and_then(|index| a.store.tracks.get(*index))
    else {
        return slint::Image::default();
    };
    a.art
        .get(&track.path)
        .cloned()
        .or_else(|| track_video_id(track).and_then(|id| a.art.get(&format!("yt://{id}")).cloned()))
        .unwrap_or_default()
}

fn refresh_playlist_artwork(w: &MainWindow, a: &App) {
    let playlists_model = w.get_playlists();
    let destinations_model = w.get_playlist_destinations();

    for (index, keys) in [
        (0, &a.liked_sidebar_art_keys),
        (1, &a.history_sidebar_art_keys),
    ] {
        if let Some(art) = keys.iter().find_map(|key| a.art.get(key)) {
            set_playlist_row_art(&playlists_model, index, art);
        }
    }
    for (index, playlist) in a.store.playlists.iter().enumerate() {
        let Some(art) = playlist.tracks.first().and_then(|key| a.art.get(key)) else {
            continue;
        };
        set_playlist_row_art(&playlists_model, index + 2, art);
        set_playlist_row_art(&destinations_model, index, art);
        if playlist.id == a.playlist {
            w.set_playlist_art(art.clone());
        }
    }
}

fn set_playlist_row_art(model: &ModelRc<PlaylistRow>, index: usize, art: &slint::Image) {
    if let Some(rows) = model.as_any().downcast_ref::<VecModel<PlaylistRow>>() {
        if let Some(mut row) = rows.row_data(index) {
            if row.art != *art {
                row.art = art.clone();
                rows.set_row_data(index, row);
            }
        }
    }
}

fn refresh_history_sidebar_art(w: &MainWindow, a: &mut App, track: &Track) {
    a.history_sidebar_art_keys = art_keys(a, track);
    let art = art_image(a, track);
    set_playlist_row_art(&w.get_playlists(), 1, &art);
}

fn refresh_playlists(w: &MainWindow, a: &mut App) {
    let by_key = playlist_track_lookup(a);
    if let Some(p) = a.store.playlists.iter().find(|p| p.id == a.playlist) {
        let cover = playlist_cover(a, p, &by_key);
        w.set_playlist_title(p.name.clone().into());
        w.set_playlist_detail(playlist_detail(p).into());
        w.set_playlist_art(cover);
        w.set_playlist_public(p.is_public);
    } else {
        w.set_playlist_detail("".into());
        w.set_playlist_art(slint::Image::default());
        w.set_playlist_public(false);
    }
    let liked = a.library.iter().find(|track| a.store.is_liked(track));
    let liked_cover = liked.map(|track| art_image(a, track)).unwrap_or_default();
    let liked_art_keys = liked.map(|track| art_keys(a, track)).unwrap_or_default();
    let history_track = a.store.history.first().map(|entry| &entry.track);
    let history_cover = history_track
        .map(|track| art_image(a, track))
        .unwrap_or_default();
    let history_art_keys = history_track
        .map(|track| art_keys(a, track))
        .unwrap_or_default();
    let liked_count = a
        .library
        .iter()
        .filter(|track| a.store.is_liked(track))
        .count();
    a.liked_sidebar_art_keys = liked_art_keys;
    a.history_sidebar_art_keys = history_art_keys;
    let mut playlists = vec![
        PlaylistRow {
            id: "liked".into(),
            name: crate::i18n::translate(&a.store.settings.language, "Музика, яка сподобалась")
                .into(),
            detail: crate::i18n::translate(
                &a.store.settings.language,
                &format!("{} композицій · локально", liked_count),
            )
            .into(),
            art: liked_cover,
        },
        PlaylistRow {
            id: "history".into(),
            name: crate::i18n::translate(&a.store.settings.language, "Послухати пізніше").into(),
            detail: crate::i18n::translate(
                &a.store.settings.language,
                "Локальні списки відтворення",
            )
            .into(),
            art: history_cover,
        },
    ];
    playlists.extend(a.store.playlists.iter().map(|p| PlaylistRow {
        id: p.id.clone().into(),
        name: p.name.clone().into(),
        detail: playlist_detail(p).into(),
        art: playlist_cover(a, p, &by_key),
    }));
    w.set_playlist_destinations(model(playlists.iter().skip(2).cloned().collect()));
    w.set_playlists(model(playlists));
    w.set_sources(model(
        a.store
            .sources
            .iter()
            .map(|s| s.path.clone().into())
            .collect(),
    ));
}
fn refresh_queue_view(w: &MainWindow, a: &App) {
    let mut local_paths = a.local_paths.clone();
    local_paths.extend(a.store.downloads.values().map(|path| normalize_path(path)));
    w.set_queue_tracks(
        Rc::new(crate::track_model::TrackModel::new(
            a.queue
                .tracks
                .iter()
                .map(|track| row(a, track, &local_paths))
                .collect(),
        ))
        .into(),
    );
    w.set_queue_current_index(a.queue.index.map_or(-1, |index| index as i32));
}
fn playlist_detail(p: &Playlist) -> String {
    let author = if p.author.trim().is_empty() {
        "Ви"
    } else {
        p.author.trim()
    };
    let days = crate::services::now_ms().saturating_sub(p.updated_at) / 86_400_000;
    let modified = if p.updated_at == 0 {
        "ще не змінювався".to_string()
    } else {
        match days {
            0 => "змінено сьогодні".to_string(),
            1 => "змінено учора".to_string(),
            n => format!("змінено {n} дн. тому"),
        }
    };
    format!("{} композицій · {} · {}", p.tracks.len(), author, modified)
}
fn scan(a: &Shared, w: &MainWindow) {
    let mut a = a.borrow_mut();
    if a.scanning {
        return;
    }
    #[cfg(target_os = "android")]
    match crate::android::ensure_audio_permission() {
        Ok(true) => {
            a.permission_wait_started = None;
            crate::android::finish_permission_request();
        }
        Ok(false) => {
            a.permission_wait_started.get_or_insert_with(Instant::now);
            w.set_status("Дозвольте доступ до музики й аудіо. Якщо доступ вимкнено — увімкніть його в налаштуваннях Android і повторіть сканування.".into());
            return;
        }
        Err(error) => {
            w.set_status(error.into());
            return;
        }
    }
    let sources: Vec<core::LibrarySource> = a
        .store
        .sources
        .iter()
        .map(|s| core::LibrarySource {
            id: s.id.clone(),
            kind: if s.kind == "file" {
                core::LibrarySourceKind::File
            } else {
                core::LibrarySourceKind::Folder
            },
            path: s.path.clone(),
        })
        .collect();
    let mut source_labels: std::collections::HashMap<String, String> = sources
        .iter()
        .map(|source| {
            let label = std::path::Path::new(&source.path)
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(&source.path)
                .to_owned();
            (source.id.clone(), label)
        })
        .collect();
    #[cfg(not(target_os = "android"))]
    source_labels.insert("system-music".into(), "Музична папка".into());
    a.scan_generation = a.scan_generation.wrapping_add(1);
    let generation = a.scan_generation;
    let tx = a.tx.clone();
    let downloads = a.store.downloads.clone();
    a.scanning = true;
    w.set_status("Сканування бібліотеки…".into());
    w.set_library_scanning(true);
    let cancellation = Arc::new(std::sync::atomic::AtomicBool::new(false));
    a.scan_cancel = Some(cancellation.clone());
    a.rt.spawn_blocking(move || {
        #[cfg(not(target_os = "android"))]
        let result = core::scan_library_cancellable(sources, cancellation.clone());
        #[cfg(target_os = "android")]
        let result = match core::scan_android_library_cancellable(sources, cancellation.clone()) {
            Ok(result) => result,
            Err(error) => {
                let _ = tx.send(Event::ScanFailed(generation, error));
                return;
            }
        };
        let status = result
            .source_statuses
            .iter()
            .map(|source| {
                let locale = crate::i18n::current();
                let label = source_labels
                    .get(&source.source_id)
                    .map(String::as_str)
                    .unwrap_or("Музичне сховище");
                let label = crate::i18n::translate(locale, label);
                let state = match source.state {
                    core::SourceScanState::Ready => "доступне",
                    core::SourceScanState::Empty => "порожнє",
                    core::SourceScanState::Unavailable => "недоступне",
                };
                let state = crate::i18n::translate(locale, state);
                let count =
                    crate::i18n::translate(locale, &format!("Треків: {}", source.track_count));
                format!("{label}: {state} · {count}")
            })
            .collect::<Vec<_>>()
            .join(" · ");
        let status = if status.is_empty() {
            crate::i18n::translate(
                crate::i18n::current(),
                &format!("Знайдено композицій: {}", result.tracks.len()),
            )
        } else {
            status
        };
        let tracks = result
            .tracks
            .into_iter()
            .map(|t| Track {
                path: t.path,
                title: t.title,
                artist: t.artist,
                album: t.album,
                duration_secs: t.duration_secs,
                video_id: t.video_id,
                ..Default::default()
            })
            .collect();
        let missing = downloads.into_iter()
            .filter(|(_, path)| !std::path::Path::new(path).is_file())
            .collect();
        let _ = tx.send(Event::Scan(generation, tracks, status, missing));
    });
}
fn browse(a: &Shared, w: &MainWindow, query: Option<String>, radio: bool) {
    let mut a = a.borrow_mut();
    a.search_generation += 1;
    if let Some(task) = a.search_task.take() {
        task.abort();
    }
    let id = a.search_generation;
    let tx = a.tx.clone();
    w.set_status("Завантаження з YouTube Music…".into());
    let video = a.current.as_ref().and_then(|t| t.video_id.clone());
    if let Some(q) = &query {
        if let Some(rows) = a.search_cache.get(q) {
            let _ = tx.send(Event::Search(id, query, Ok(rows)));
            return;
        }
    }
    let cache_key = query.clone();
    let task = a.rt.spawn(async move {
        let result = if let Some(q) = query {
            core::ytm_search_songs(q, Some(40)).await.map(|songs| {
                songs
                    .into_iter()
                    .filter(|s| {
                        ["song", "video", "top"].contains(&s.kind.as_str())
                            && !s.video_id.is_empty()
                    })
                    .map(|s| Track {
                        path: format!("yt://{}", s.video_id),
                        video_id: Some(s.video_id),
                        clip_video_id: (!s.clip_video_id.is_empty()).then_some(s.clip_video_id),
                        title: s.title,
                        artist: s.artist,
                        album: s.album,
                        duration_secs: s.duration_secs as f64,
                        thumbnail_url: s.thumbnail_url,
                        catalog_metadata: None,
                    })
                    .collect()
            })
        } else {
            let result = if radio {
                core::ytm_recommendations(video, None, Some(40)).await
            } else {
                core::ytm_browse(None, Some(40)).await
            };
            result.map(|songs| {
                songs
                    .into_iter()
                    .filter(|s| !s.video_id.is_empty())
                    .map(|s| Track {
                        path: format!("yt://{}", s.video_id),
                        video_id: Some(s.video_id),
                        clip_video_id: (!s.clip_video_id.is_empty()).then_some(s.clip_video_id),
                        title: s.title,
                        artist: s.artist,
                        album: s.album,
                        duration_secs: s.duration_secs as f64,
                        thumbnail_url: s.thumbnail_url,
                        catalog_metadata: None,
                    })
                    .collect()
            })
        };
        let _ = tx.send(Event::Search(id, cache_key, result));
    });
    a.search_task = Some(task);
}

async fn fetch_home_recommendations(
    profile: crate::home::RecommendationProfile,
) -> Result<(Vec<Track>, bool), String> {
    let blueberry = tokio::spawn(crate::home::blueberry_candidates());
    let resolved = join_all(profile.seeds.iter().cloned().map(|track| async move {
        let video_id = if let Some(id) = crate::services::track_video_id(&track) {
            Some(id.to_string())
        } else if let Some(query) = crate::home::recommendation_query(&track) {
            core::ytm_match_candidates(query, Some(24))
                .await
                .ok()
                .and_then(|songs| {
                    let candidates = songs.into_iter().map(|song| Track {
                        path: format!("yt://{}", song.video_id), video_id: Some(song.video_id),
                        title: song.title, artist: song.artist, duration_secs: song.duration_secs as f64,
                        ..Default::default()
                    }).collect();
                    crate::home::select_public_seed(&track, candidates)
                })
        } else {
            None
        };
        video_id
    }))
    .await;
    let mut seen_seeds = std::collections::HashSet::new();
    let seed_ids: Vec<_> = resolved
        .into_iter()
        .flatten()
        .filter(|id| seen_seeds.insert(id.clone()))
        .collect();

    let mut batches = join_all(seed_ids.iter().cloned().map(|seed_id| async move {
        core::ytm_recommendations(Some(seed_id.clone()), None, Some(32))
            .await
            .ok()
            .map(|songs| {
                songs
                    .into_iter()
                    .filter(|song| !song.video_id.is_empty() && song.video_id != seed_id)
                    .map(|song| Track {
                        path: format!("yt://{}", song.video_id),
                        title: song.title,
                        artist: song.artist,
                        album: song.album,
                        duration_secs: song.duration_secs as f64,
                        video_id: Some(song.video_id),
                        clip_video_id: (!song.clip_video_id.is_empty())
                            .then_some(song.clip_video_id),
                        thumbnail_url: song.thumbnail_url,
                        catalog_metadata: None,
                    })
                    .collect::<Vec<_>>()
            })
    }))
    .await
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    if let Ok(candidates) = blueberry.await {
        if !candidates.is_empty() { batches.insert(0, candidates); }
    }

    let mut recommendations = crate::home::rank_recommendations(batches, &profile, 12);
    let personalized = !profile.seeds.is_empty() && !recommendations.is_empty();
    if recommendations.is_empty() {
        let songs = core::ytm_recommendations(None, None, Some(40)).await?;
        let general = songs
            .into_iter()
            .filter(|song| !song.video_id.is_empty())
            .map(|song| Track {
                path: format!("yt://{}", song.video_id),
                title: song.title,
                artist: song.artist,
                album: song.album,
                duration_secs: song.duration_secs as f64,
                video_id: Some(song.video_id),
                clip_video_id: (!song.clip_video_id.is_empty()).then_some(song.clip_video_id),
                thumbnail_url: song.thumbnail_url,
                catalog_metadata: None,
            })
            .collect();
        recommendations = crate::home::rank_recommendations(vec![general], &profile, 12);
    }
    Ok((recommendations, personalized))
}

fn load_home_recommendations(a: &Shared, w: &MainWindow) {
    let mut a = a.borrow_mut();
    if a.snapshot {
        a.home_recommendation_loaded = true;
        return;
    }
    if a.home_recommendation_loading {
        return;
    }
    a.home_recommendation_generation = a.home_recommendation_generation.wrapping_add(1);
    let generation = a.home_recommendation_generation;
    let profile = crate::home::recommendation_profile(&a.store, a.current.as_ref());
    let tx = a.tx.clone();
    w.set_home_recommendation_loading(true);
    w.set_home_recommendation_failed(false);
    a.home_recommendation_loading = true;
    a.home_recommendation_task = Some(a.rt.spawn(async move {
        let result = fetch_home_recommendations(profile).await;
        let _ = tx.send(Event::HomeRecommendations(generation, result));
    }));
}

fn show_saved_recommendations(w: &MainWindow, a: &mut App) {
    a.home_recommendations = a.store.recommendations.tracks.clone();
    w.set_home_recommendation_personalized(a.store.recommendations.personalized);
    let mut local_paths = a.local_paths.clone();
    local_paths.extend(a.store.downloads.values().map(|p| normalize_path(p)));
    crate::track_model::set_home_recommendations(
        w,
        a.home_recommendations
            .iter()
            .map(|t| row(a, t, &local_paths))
            .collect(),
    );
    for track in a.home_recommendations.clone() {
        load_art(a, &track);
    }
}

fn load_art(a: &mut App, track: &Track) {
    queue_art(a, track, false);
}

fn queue_art(a: &mut App, track: &Track, priority: bool) {
    let keys = art_keys(a, track);
    if let Some(art) = keys.iter().find_map(|key| a.art.get(key).cloned()) {
        let mut added_alias = false;
        for key in keys {
            if let std::collections::hash_map::Entry::Vacant(entry) = a.art.entry(key) {
                entry.insert(art.clone());
                added_alias = true;
            }
        }
        if added_alias {
            let _ = a.tx.send(Event::ArtCached(track.path.clone()));
        }
        return;
    }
    // Different paths can share an identity while only one is in flight.
    // Keep each path queued so its mounted row receives the resolved cover.
    if a.art_pending.contains(&track.path)
        || a.art_failed
            .get(&track.path)
            .is_some_and(|failed| failed.elapsed() < ART_FAILURE_COOLDOWN)
    {
        return;
    }
    for key in &keys {
        a.art_failed.remove(key);
    }
    if a.art_attempts
        .get(&track.path)
        .is_some_and(|attempts| *attempts >= MAX_ART_ATTEMPTS)
    {
        a.art_attempts.remove(&track.path);
    }
    a.art_queue.push(track, priority);
    pump_art(a);
}

// Load several visible covers together and keep one extra slot for playback.
const MAX_ART_IN_FLIGHT: usize = 4;
const MAX_ART_ATTEMPTS: u8 = 2;
const ART_FAILURE_COOLDOWN: Duration = Duration::from_secs(5);
const ART_RETRY_DELAY: Duration = Duration::from_millis(650);
const ART_METADATA_LOOKUP_TIMEOUT: Duration = Duration::from_secs(8);
const ART_NO_MATCH: &str = "No matching artwork";

async fn fetch_decoded_cover(url: &str) -> Option<image::RgbaImage> {
    let bytes = crate::media_library::fetch_cover(url).await.ok()?;
    tokio::task::spawn_blocking(move || crate::artwork::decode(&bytes))
        .await
        .ok()?
        .ok()
}

fn can_start_art(pending: usize, current_waiting: bool) -> bool {
    pending < MAX_ART_IN_FLIGHT || (pending == MAX_ART_IN_FLIGHT && current_waiting)
}

fn pump_art(a: &mut App) {
    loop {
        // Keep one extra slot for the selected track when background artwork
        // is slow; never start more than MAX_ART_IN_FLIGHT + 1 requests in total.
        let current_waiting = a.art_queue.entries.front().is_some_and(|queued| {
            a.current
                .as_ref()
                .is_some_and(|current| current.path == queued.path)
        });
        if !can_start_art(a.art_pending.len(), current_waiting) {
            break;
        }
        let Some(track) = a.art_queue.pop() else {
            break;
        };
        let keys = art_keys(a, &track);
        if let Some(art) = keys.iter().find_map(|key| a.art.get(key).cloned()) {
            for key in keys {
                a.art.entry(key).or_insert_with(|| art.clone());
            }
            // A queued alias can become available while another track loads.
            // Its already-mounted row must be notified as well.
            let _ = a.tx.send(Event::ArtCached(track.path.clone()));
            continue;
        }
        if !a.art_pending.insert(track.path.clone()) {
            continue;
        }
        let attempts = a
            .art_attempts
            .get(&track.path)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        a.art_attempts.insert(track.path.clone(), attempts);
        let local = local_file(a, &track);
        let cache_key = a.store.identity_key(&track);
        let tx = a.tx.clone();
        a.rt.spawn(async move {
            let key = track.path.clone();
            let result = async {
                let lookup_keys = keys.clone();
                let local_path = local.clone();
                if let Some((img, from_cache)) = tokio::task::spawn_blocking(move || {
                    if let Some(img) = crate::artwork::cached_for_keys(&lookup_keys) {
                        return Some((img, true));
                    }
                    local_path
                        .as_deref()
                        .and_then(crate::artwork::local_decoded)
                        .map(|img| (img, false))
                })
                .await
                .map_err(|e| e.to_string())?
                {
                    return Ok((img, !from_cache));
                }
                let direct_urls = crate::artwork::remote_urls(&track);
                for url in &direct_urls {
                    if let Some(img) = fetch_decoded_cover(&url).await {
                        return Ok((img, true));
                    }
                }

                // Local files often have neither embedded album art nor a
                // YouTube id. Resolve those in the background from YTM using
                // exact title/artist matching; the bounded outer timeout keeps
                // a slow metadata service from holding the cover queue open.
                if local.is_some() && crate::services::track_video_id(&track).is_none() {
                    let candidate = tokio::time::timeout(
                        ART_METADATA_LOOKUP_TIMEOUT,
                        core::ytm_cover_candidate(&track.title, &track.artist, track.duration_secs),
                    )
                    .await
                    .map_err(|_| "Artwork metadata lookup timed out".to_string())??;
                    if let Some(song) = candidate {
                        let mut matched = track.clone();
                        matched.video_id = Some(song.video_id);
                        matched.thumbnail_url = song.thumbnail_url;
                        for url in crate::artwork::remote_urls(&matched) {
                            if let Some(img) = fetch_decoded_cover(&url).await {
                                return Ok((img, true));
                            }
                        }
                        return Err("Artwork image could not be loaded".to_string());
                    }
                }
                if direct_urls.is_empty() {
                    Err(ART_NO_MATCH.to_string())
                } else {
                    Err("Artwork image could not be loaded".to_string())
                }
            }
            .await;
            match result {
                Ok((img, should_cache)) => {
                    // Paint immediately; compression and disk I/O run afterwards.
                    let cached = should_cache.then(|| img.clone());
                    let _ = tx.send(Event::Art(key.clone(), Ok(img)));
                    if let Some(img) = cached {
                        let _ = tokio::task::spawn_blocking(move || {
                            crate::artwork::store_cached_image(&cache_key, &img);
                            if cache_key != key {
                                crate::artwork::store_cached_image(&key, &img);
                            }
                        })
                        .await;
                    }
                }
                Err(error) => {
                    let _ = tx.send(Event::Art(key, Err(error)));
                }
            }
        });
    }
}
fn inspect_local_download(saved_path: Option<String>, mapped_path: Option<String>) -> (Option<String>, Option<String>) {
    let mapped_exists = mapped_path.as_ref().map(|path| std::path::Path::new(path).is_file());
    let missing = mapped_path.clone().filter(|_| mapped_exists == Some(false));
    let saved = saved_path.filter(|path| {
        if mapped_path.as_ref() == Some(path) { mapped_exists == Some(true) }
        else { std::path::Path::new(path).is_file() }
    });
    (saved, missing)
}

fn start(a: &Shared, w: &MainWindow, track: Track) {
    let shared = a.clone();
    let mut a = a.borrow_mut();
    if a.loading
        && a.current
            .as_ref()
            .is_some_and(|current| current.path == track.path)
    {
        return;
    }
    a.loading = true;
    a.ended = false;
    a.loaded_path.clear();
    a.lyric_generation += 1;
    if let Some(task) = a.lyric_task.take() {
        task.abort();
    }
    a.alternatives.clear();
    a.current = Some(track.clone());
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    if let Some(session) = a.media_session.as_mut() {
        session.set_track(&track);
    }
    #[cfg(target_os = "android")]
    {
        let _ = crate::android::media_update(
            &serde_json::json!({
                "title": track.title, "artist": track.artist, "album": track.album,
                "duration": track.duration_secs, "position": 0.0, "playing": false, "active": true
            })
            .to_string(),
        );
    }
    w.set_current_downloaded(is_downloaded(&a, &track.path));
    w.set_current_downloading(a.download_pending.contains(&track.path));
    w.set_current_local(local_file(&a, &track).is_some());
    w.set_current_track_key(track.path.clone().into());
    refresh_queue_view(w, &a);
    w.set_current_video_id(track.video_id.clone().unwrap_or_default().into());
    w.set_current_clip_video_id(track.clip_video_id.clone().unwrap_or_default().into());
    w.set_current_title(track.title.clone().into());
    w.set_current_artist(track.artist.clone().into());
    w.set_current_album(track.album.clone().into());
    w.set_current_art(art_image(&a, &track));
    w.set_current_liked(a.store.is_liked(&track));
    w.set_current_disliked(a.store.is_disliked(&track));
    w.set_player_state("loading".into());
    w.set_position(0.);
    w.set_duration(track.duration_secs as f32);
    w.set_lyric_lines(model(vec![]));
    w.set_lyrics_document("".into());
    w.set_lyrics_enrichment_status("".into());
    w.set_active_lyric_index(-1);
    w.set_lyrics_status("Завантаження слів…".into());
    w.set_status("Завантаження композиції…".into());
    w.set_lyrics_offset(a.store.offset_for(&track) as i32);
    let tx = a.tx.clone();
    let audio = a.audio.clone();
    let serial = a
        .load_generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    let generation = a.load_generation.clone();
    let gate = a.load_gate.clone();
    let saved_path = local_file(&a, &track);
    let mapped_path = a.store.downloads.get(&track.path).cloned();
    if let Some(previous) = a.load_task.take() {
        previous.abort();
    }
    queue_art(&mut a, &track, true);
    a.load_task = Some(a.rt.spawn(async move {
        let (saved_path, missing) = tokio::task::spawn_blocking(move || inspect_local_download(saved_path, mapped_path)).await.unwrap_or_default();
        if let Some(path) = missing { let _ = tx.send(Event::MissingDownload(track.path.clone(), path)); }
        let path = if let Some(path) = saved_path {
            Ok(path)
        } else if let Some(id) = &track.video_id {
            match core::ytm_cache_stream(id.clone()).await {
                Ok(Some(c)) => Ok(c.path),
                Ok(None) => Err("Потік недоступний".into()),
                Err(e) => Err(e),
            }
        } else {
            Ok(track.path.clone())
        };
        // Waiting for the decoder and its serial gate must not occupy async
        // runtime workers needed for artwork, search and lyric requests.
        let result = match path {
            Ok(p) => tokio::task::spawn_blocking(move || {
                let _guard = gate.lock().map_err(|e| e.to_string())?;
                if generation.load(std::sync::atomic::Ordering::SeqCst) != serial {
                    return Err("Скасовано".into());
                }
                core::player_load(&audio, p.clone(), true).map(|_| p)
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string())),
            Err(e) => Err(e),
        };
        let _ = tx.send(Event::Loaded(serial, track.path, result));
    }));
    drop(a);
    lyrics_request(&shared, w, false);
}
fn lyrics_request(a: &Shared, w: &MainWindow, force: bool) {
    let mut a = a.borrow_mut();
    let Some(track) = a.current.clone() else {
        return;
    };
    if !force
        && a.lyric_task
            .as_ref()
            .is_some_and(|task| !task.is_finished())
    {
        return;
    }
    if let Some(task) = a.lyric_task.take() {
        task.abort();
    }
    a.lyric_generation += 1;
    let generation = a.lyric_generation;
    let provider = a.store.lyrics_request_provider(&track);
    #[cfg(debug_assertions)]
    eprintln!(
        "[lyrics-debug] request source={provider} refresh={force} video_id={} album_present={}",
        crate::services::track_video_id(&track).is_some(),
        !track.album.trim().is_empty()
    );
    if !force && !matches!(provider.as_str(), "betterlyrics" | "kugou") {
        if let Some(mut alts) = a
            .store
            .cached_lyrics(&track.path)
            .or_else(|| ls::migrated_cache(&a.store, &track))
        {
            #[cfg(debug_assertions)]
            eprintln!(
                "[lyrics-debug] using saved lyrics: {} variant(s)",
                alts.len()
            );
            crate::braccato::providers::normalize_alternatives(&mut alts);
            let should_supplement = provider == "auto" && !alts.iter().any(|alt|
                alt.provider == "Власні слова");
            a.alternatives = alts;
            show_preferred_lyrics(w, &a);
            if should_supplement {
                if a.kugou_supplement_attempts.len() >= 1024 {
                    a.kugou_supplement_attempts.clear();
                }
                let identity = a.store.identity_key(&track);
                if !a.kugou_supplement_attempts.contains(&identity) {
                    let tx = a.tx.clone();
                    a.lyric_task = Some(a.rt.spawn(async move {
                        let result = ls::fetch(&track, "auto", true, None).await;
                        let _ = tx.send(Event::LyricsSupplement(track.path, generation, result));
                    }));
                }
            }
            return;
        }
    }
    let tx = a.tx.clone();
    w.set_lyrics_status("Шукаємо слова пісні…".into());
    a.lyric_task = Some(a.rt.spawn(async move {
        let jwt = ls::JwtCache::load().token();
        let preview_tx = tx.clone();
        let path = track.path.clone();
        let result = ls::fetch_with_interactive_auth(&track, &provider, force, jwt, move |alts| {
            let _ = preview_tx.send(Event::LyricsPreview(path.clone(), generation, alts));
        })
        .await;
        let jwt = ls::JwtCache::load();
        let _ = tx.send(Event::Lyrics(track.path, generation, result, jwt));
    }));
}
fn show_preferred_lyrics(w: &MainWindow, a: &App) {
    let index = a
        .current
        .as_ref()
        .map(|t| a.store.preferred_lyrics_index(t, &a.alternatives))
        .unwrap_or(0);
    show_lyrics(w, a, index);
}
fn show_lyrics(w: &MainWindow, a: &App, index: usize) {
    let Some(alt) = a.alternatives.get(index) else {
        return;
    };
    let mut enriched = alt.clone();
    w.set_lyrics_enrichment_status("".into());
    if !a.snapshot {
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
                let tx = a.tx.clone();
                a.rt.spawn(async move {
                    let result = crate::lyrics_translation::fetch(&request).await;
                    let _ = tx.send(Event::Enriched(key, result));
                });
            }
        }
    }
    let display = crate::braccato::visible_layers(
        &enriched,
        a.store.settings.lyrics_romanization,
        a.store.settings.lyrics_translation,
    );
    let alt = &display;
    w.set_providers(model(
        a.alternatives
            .iter()
            .map(|l| format!("{} · {:?}", l.provider, l.sync_type).into())
            .collect(),
    ));
    w.set_lyrics_credits(alt.lyrics.songwriters.join(" · ").into());
    w.set_lyrics_timed(crate::braccato::effective_sync_type(alt) != LyricsSyncType::Plain);
    w.set_lyrics_word_timed(matches!(
        crate::braccato::effective_sync_type(alt),
        LyricsSyncType::Word | LyricsSyncType::RichSync
    ));
    w.set_provider_index(index as i32);
    w.set_lyrics_status(format!("{} · натисніть рядок, щоб перейти", alt.provider).into());
    let duration_ms = (a
        .current
        .as_ref()
        .map(|t| t.duration_secs)
        .unwrap_or(0.)
        .max(0.)
        * 1000.) as i64;
    w.set_lyric_lines(model(crate::braccato::rows(alt, duration_ms)));
}
fn selected(a: &App, index: i32) -> Option<Track> {
    if index < 0 {
        a.current.clone()
    } else {
        a.visible.get(index as usize).cloned()
    }
}
fn selected_library_tracks(a: &App) -> Vec<Track> {
    let tracks = a
        .library
        .iter()
        .cloned()
        .chain(a.store.excluded_library_tracks());
    deduplicate_tracks(tracks)
        .into_iter()
        .filter(|track| {
            a.library_view
                .selected
                .contains(&a.store.identity_key(track))
        })
        .collect()
}
fn report(w: &MainWindow, result: Result<(), String>) {
    if let Err(e) = result {
        w.set_status(e.into());
    }
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn handle_media_event(a: &Shared, w: &MainWindow, event: souvlaki::MediaControlEvent) {
    use souvlaki::{MediaControlEvent as E, SeekDirection};
    match event {
        E::Play => {
            let app = a.borrow();
            if app.current.is_none() {
                drop(app);
                w.invoke_select_track(0);
            } else {
                report(w, core::player_play(&app.audio));
            }
        }
        E::Pause | E::Stop => report(w, core::player_pause(&a.borrow().audio)),
        E::Toggle => {
            let app = a.borrow();
            if app.current.is_none() {
                drop(app);
                w.invoke_select_track(0);
            } else {
                report(w, core::player_toggle(&app.audio).map(|_| ()));
            }
        }
        E::Previous => {
            if w.get_position() > 3. {
                report(w, a.borrow().audio.seek(0.));
            } else {
                let track = a.borrow_mut().queue.advance(true, false);
                if let Some(track) = track {
                    start(a, w, track);
                }
            }
        }
        E::Next => {
            let next = {
                let mut app = a.borrow_mut();
                let next = app.queue.advance(false, false);
                if next.is_some() {
                    if let Some(current) = app.current.clone() {
                        app.store.record_skip_if_early(
                            &current,
                            f64::from(w.get_position()),
                            f64::from(w.get_duration()),
                        );
                        persist(w, &app);
                    }
                }
                next
            };
            if let Some(track) = next {
                start(a, w, track);
            }
        }
        E::SetPosition(position) => {
            report(w, a.borrow().audio.seek(position.0.as_secs_f64()));
        }
        E::SeekBy(direction, duration) => {
            let delta = duration.as_secs_f64()
                * if direction == SeekDirection::Forward {
                    1.
                } else {
                    -1.
                };
            let target = (w.get_position() as f64 + delta).clamp(0., w.get_duration() as f64);
            report(w, a.borrow().audio.seek(target));
        }
        E::Seek(direction) => {
            let delta = if direction == SeekDirection::Forward {
                10.
            } else {
                -10.
            };
            let target = (w.get_position() as f64 + delta).clamp(0., w.get_duration() as f64);
            report(w, a.borrow().audio.seek(target));
        }
        _ => {}
    }
}

#[cfg(target_os = "android")]
fn handle_android_media_event(a: &Shared, w: &MainWindow, raw: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return;
    };
    match value
        .get("action")
        .and_then(|action| action.as_str())
        .unwrap_or("")
    {
        "play" => {
            let app = a.borrow();
            if app.current.is_none() {
                drop(app);
                w.invoke_select_track(0);
            } else {
                report(w, core::player_play(&app.audio));
            }
        }
        "pause" => report(w, core::player_pause(&a.borrow().audio)),
        "next" | "previous" => {
            let previous = value["action"] == "previous";
            let track = {
                let mut app = a.borrow_mut();
                let track = app.queue.advance(previous, false);
                if !previous && track.is_some() {
                    if let Some(current) = app.current.clone() {
                        if let Ok(status) = app.audio.status() {
                            app.store.record_skip_if_early(
                                &current,
                                status.position,
                                status.duration,
                            );
                            persist(w, &app);
                        }
                    }
                }
                track
            };
            if let Some(track) = track {
                start(a, w, track);
            }
        }
        "seek" => {
            let seconds = value
                .get("position")
                .and_then(|position| position.as_f64())
                .unwrap_or(0.)
                / 1000.;
            report(w, a.borrow().audio.seek(seconds));
        }
        _ => {}
    }
}

pub fn run() -> Result<(), slint::PlatformError> {
    let startup_started = Instant::now();
    crate::player_debug::Recorder::start();
    let w = MainWindow::new()?;
    w.set_app_version(env!("CARGO_PKG_VERSION").into());
    #[cfg(target_os = "android")]
    w.set_android_platform(true);
    crate::i18n::install(&w);
    crate::player_themes::install(&w);
    crate::native_lyrics::install(&w);
    crate::navigation_fx::install(&w);
    let (tx, rx) = mpsc::channel();
    let rt = Arc::new(Runtime::new().expect("Rust async runtime"));
    let _video_surface = crate::video::install(&w, rt.clone());
    let _ambient_surface = crate::ambient_native::install(&w);
    let widget_smoke = std::env::args().any(|a| a == "--widget-smoke");
    let widget_preview = std::env::args().any(|a| a == "--widget-preview");
    let snapshot = widget_smoke || widget_preview || std::env::args().any(|a| a == "--screenshots");
    let (store, load_error) = if snapshot {
        (Store::default(), None)
    } else {
        match storage::load() {
            Ok(s) => (s, None),
            Err(e) => (Store::default(), Some(e)),
        }
    };
    crate::player_debug::record_for_window(
        &w,
        "startup_storage_load",
        startup_started.elapsed().as_secs_f64() * 1000.,
    );
    w.set_native_file_tools(!cfg!(target_os = "android"));
    let audio = Arc::new(core::start_audio());
    let error_tx = tx.clone();
    let saver = storage::SaveWorker::new(move |error| {
        let _ = error_tx.send(Event::Message(error));
    });
    let a = Rc::new(RefCell::new(App {
        store,
        visible: vec![],
        page_results: std::collections::HashMap::new(),
        library: vec![],
        library_view: LibraryViewState::default(),
        collection_jobs: CollectionJobs::default(),
        collection_representatives: Vec::new(),
        local_paths: Default::default(),
        queue: Queue::default(),
        current: None,
        playlist: String::new(),
        alternatives: vec![],
        enrichment_attempts: RefCell::default(),
        kugou_supplement_attempts: Default::default(),
        audio,
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        media_session: None,
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        media_session_retry_at: None,
        rt,
        tx,
        loading: false,
        scanning: false,
        scan_generation: 0,
        scan_cancel: None,
        source_status: String::new(),
        #[cfg(target_os = "android")]
        permission_wait_started: None,
        search_generation: 0,
        home_recommendation_generation: 0,
        home_recommendation_loading: false,
        home_recommendation_loaded: false,
        home_recommendation_task: None,
        home_recommendations: vec![],
        liked_sidebar_art_keys: vec![],
        history_sidebar_art_keys: vec![],
        search_index: Default::default(),
        search_cache: Default::default(),
        search_task: None,
        pending_search: None,
        lyric_generation: 0,
        lyric_task: None,
        loaded_path: String::new(),
        ended: false,
        load_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        load_task: None,
        download_pending: Default::default(),
        download_serial: 0,
        download_generations: Default::default(),
        download_progress: Default::default(),
        download_tasks: Default::default(),
        load_gate: Arc::new(std::sync::Mutex::new(())),
        last_playing: None,
        pending_tag_path: None,
        pending_tag_video_id: None,
        art: Default::default(),
        art_pending: Default::default(),
        art_failed: Default::default(),
        art_attempts: Default::default(),
        art_queue: ArtworkQueue::default(),
        jwt: ls::JwtCache::load(),
        challenge_in_flight: false,
        #[cfg(target_os = "windows")]
        embedded_challenge: None,
        snapshot,
        store_revision: Cell::new(0),
        save_due: Cell::new(None),
        saver,
    }));
    {
        let a = a.borrow();
        w.set_jwt_status(
            if a.jwt.token().is_some() {
                "JWT збережено · не прострочено"
            } else {
                "Потрібна перевірка"
            }
            .into(),
        );
        w.set_volume(a.store.settings.volume);
        w.set_rate(a.store.settings.rate);
        w.set_shuffle(a.store.settings.shuffle);
        w.set_repeat_mode(a.store.settings.repeat.clone().into());
        w.set_art_mode(a.store.settings.art_mode.clone().into());
        w.set_lyrics_provider(a.store.settings.lyrics_provider.clone().into());
        w.set_lyrics_stage(a.store.settings.lyrics_stage);
        w.set_lyrics_reduced_motion(a.store.settings.lyrics_reduced_motion);
        w.set_lyrics_passive_scroll(a.store.settings.lyrics_passive_scroll);
        let _ = a.audio.volume(a.store.settings.volume);
        let _ = a.audio.rate(a.store.settings.rate);
    }
    #[cfg(not(target_os = "android"))]
    w.set_lyric_font_import_supported(true);
    crate::native_lyric_fonts::restore();
    apply_settings(&w, &a.borrow());
    if let Some(e) = load_error {
        w.set_status(format!("Помилка читання даних: {e}. Оригінальний файл збережено.").into());
    }
    macro_rules! bind {($method:ident, |$ui:ident,$state:ident $(,$arg:ident)*| $body:block)=>{{let weak=w.as_weak();let shared=a.clone();w.$method(move|$($arg),*|{if let Some($ui)=weak.upgrade(){let $state=&shared;$body}});}};}
    bind!(on_search_edited, |ui, state, query| {
        ui.set_status("".into());
        let mut a = state.borrow_mut();
        a.search_generation += 1;
        if let Some(task) = a.search_task.take() {
            task.abort();
        }
        a.pending_search = None;
        if ui.get_active_view() == "search" {
            let App {
                search_index,
                library,
                ..
            } = &mut *a;
            let rows = search_index.find(library, &query);
            a.visible = rows;
            refresh(&ui, &mut a);
            if query.trim().chars().count() >= 2 {
                a.pending_search = Some((Instant::now(), query.to_string()));
            }
        }
    });
    bind!(on_navigate, |ui, state, view| {
        let navigation_started = Instant::now();
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
        ui.set_status("".into());
        state.borrow_mut().search_generation += 1;
        state.borrow_mut().pending_search = None;
        if let Some(task) = state.borrow_mut().search_task.take() {
            task.abort();
        }
        if view == "home" && ui.get_home_rotate() {
            ui.invoke_home_choose(-1);
        }
        if view == "home" && !state.borrow().home_recommendation_loaded {
            ui.invoke_request_home_recommendations();
        }
        let cached = state.borrow().page_results.get(view.as_str()).cloned();
        if matches!(view.as_str(), "search" | "explore") {
            state.borrow_mut().visible = cached.clone().unwrap_or_default();
        }
        ui.set_active_view(view.clone());
        if view == "explore" && cached.is_none() {
            browse(state, &ui, None, false);
        } else {
            refresh(&ui, &mut state.borrow_mut());
        }
        crate::player_debug::record_for_window(
            &ui,
            "navigation_handler",
            navigation_started.elapsed().as_secs_f64() * 1000.,
        );
    });
    bind!(on_refresh_explore, |ui, state| {
        ui.set_catalog_title("".into());
        browse(state, &ui, None, false);
    });
    bind!(on_request_discovery_art, |ui, state, json| {
        if let Ok(track) = serde_json::from_str::<Track>(&json) {
            let mut app = state.borrow_mut();
            if let Some(art) = art_keys(&app, &track)
                .iter()
                .find_map(|key| app.art.get(key).cloned())
            {
                crate::discovery::update_art(&ui, &track.path, &art);
            } else {
                load_art(&mut app, &track);
            }
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
            start(state, &ui, track);
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
                app.search_generation = app.search_generation.wrapping_add(1);
                if let Some(task) = app.search_task.take() {
                    task.abort();
                }
                app.visible = tracks;
                ui.set_catalog_title(context.title.clone().into());
                ui.set_catalog_kind(context.kind.clone().into());
                ui.set_catalog_subtitle(
                    if context.kind == "album" {
                        context.artist.clone()
                    } else {
                        String::new()
                    }
                    .into(),
                );
                ui.set_catalog_art(
                    app.visible
                        .first()
                        .map(|track| art_image(&app, track))
                        .unwrap_or_default(),
                );
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
        ui.set_active_view("explore".into());
        let mut app = state.borrow_mut();
        ui.set_catalog_art(art_image(&app, &Track { path: context.path.clone(), thumbnail_url: context.thumbnail_url.clone(), ..Default::default() }));
        app.search_generation = app.search_generation.wrapping_add(1);
        if let Some(task) = app.search_task.take() {
            task.abort();
        }
        let generation = app.search_generation;
        let tx = app.tx.clone();
        app.visible.clear();
        refresh(&ui, &mut app);
        ui.set_status("Завантаження з YouTube Music…".into());
        app.search_task = Some(app.rt.spawn(async move {
            let result = if context.kind == "artist" && id.starts_with("UC") {
                core::ytm_artist_page(id.to_string()).await
            } else {
                core::ytm_collection_tracks(id.to_string()).await.map(|tracks|
                    crate::discovery::ArtistPage { tracks, ..Default::default() })
            }.map(|mut page| { page.tracks = context.collection_tracks(page.tracks); page });
            let _ = tx.send(Event::Collection(generation, result));
        }));
    });
    bind!(on_request_home_recommendations, |ui, state| {
        load_home_recommendations(state, &ui);
    });
    bind!(on_select_home_recommendation, |ui, state, index| {
        let track = {
            let mut a = state.borrow_mut();
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            let recommendations = a.home_recommendations.clone();
            a.queue.select(recommendations, index.max(0) as usize)
        };
        if let Some(track) = track {
            start(state, &ui, track);
        }
    });
    bind!(on_play_home_recommendations, |ui, _state| {
        ui.invoke_select_home_recommendation(0);
    });
    bind!(on_search, |ui, state, query| {
        ui.set_status("".into());
        ui.set_active_view("search".into());
        {
            let mut a = state.borrow_mut();
            a.search_generation += 1;
            a.pending_search = None;
            if let Some(task) = a.search_task.take() {
                task.abort();
            }
            let App {
                search_index,
                library,
                ..
            } = &mut *a;
            let rows = search_index.find(library, &query);
            a.visible = rows;
            refresh(&ui, &mut a);
        }
        if !query.trim().is_empty() {
            browse(state, &ui, Some(query.to_string()), false);
        }
    });
    bind!(on_select_track, |ui, state, index| {
        let track = {
            let mut a = state.borrow_mut();
            let visible = a.visible.clone();
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            a.queue.select(visible, index.max(0) as usize)
        };
        if let Some(t) = track {
            start(state, &ui, t);
        }
    });
    bind!(on_select_queue_track, |ui, state, index| {
        let track = {
            let mut a = state.borrow_mut();
            let queue = a.queue.tracks.clone();
            a.queue.select(queue, index.max(0) as usize)
        };
        if let Some(track) = track {
            start(&state, &ui, track);
        }
    });
    bind!(on_play_toggle, |ui, state| {
        let a = state.borrow();
        if a.current.is_none() {
            drop(a);
            ui.invoke_select_track(0);
        } else {
            report(&ui, core::player_toggle(&a.audio).map(|_| ()));
        }
    });
    bind!(on_previous_track, |ui, state| {
        if ui.get_position() > 3. {
            report(&ui, state.borrow().audio.seek(0.));
        } else {
            let track = state.borrow_mut().queue.advance(true, false);
            if let Some(t) = track {
                start(state, &ui, t);
            }
        }
    });
    bind!(on_next_track, |ui, state| {
        let track = {
            let mut app = state.borrow_mut();
            let track = app.queue.advance(false, false);
            if track.is_some() {
                if let Some(current) = app.current.clone() {
                    app.store.record_skip_if_early(
                        &current,
                        f64::from(ui.get_position()),
                        f64::from(ui.get_duration()),
                    );
                    persist(&ui, &app);
                }
            }
            track
        };
        if let Some(t) = track {
            start(state, &ui, t);
        }
    });
    bind!(on_seek_to, |ui, state, millis| {
        let max = ui.get_duration() as f64;
        report(
            &ui,
            state
                .borrow()
                .audio
                .seek((millis as f64 / 1000.).clamp(0., max.max(0.))),
        );
    });
    bind!(on_volume_changed, |ui, state, value| {
        let mut a = state.borrow_mut();
        a.store.settings.volume = value.clamp(0., 1.);
        report(&ui, a.audio.volume(a.store.settings.volume));
        persist(&ui, &a);
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
        persist(&ui, &a);
    });
    bind!(on_appearance_changed, |ui, state, mode, hue| {
        let mut a = state.borrow_mut();
        a.store.settings.color_mode = ColorMode::from_name(&mode);
        a.store.settings.color_hue = (hue.round() as u16) % 360;
        ui.set_color_mode(a.store.settings.color_mode.as_str().into());
        ui.set_color_hue(a.store.settings.color_hue as f32);
        persist(&ui, &a);
    });
    bind!(on_toggle_shuffle, |ui, state| {
        let mut a = state.borrow_mut();
        a.store.settings.shuffle = !a.store.settings.shuffle;
        a.queue.shuffle = a.store.settings.shuffle;
        ui.set_shuffle(a.store.settings.shuffle);
        persist(&ui, &a);
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
        persist(&ui, &a);
    });
    bind!(on_change_art_mode, |ui, state| {
        let mut a = state.borrow_mut();
        a.store.settings.art_mode = if a.store.settings.art_mode == "cover" {
            "clip"
        } else {
            "cover"
        }
        .into();
        ui.set_art_mode(a.store.settings.art_mode.clone().into());

        persist(&ui, &a);
    });
    bind!(on_toggle_like, |ui, state, index| {
        let mut a = state.borrow_mut();
        if let Some(t) = selected(&a, index) {
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
            persist(&ui, &a);
            if ui.get_active_view() == "liked"
                || (ui.get_active_view() == "library"
                    && matches!(
                        a.library_view.category,
                        LibraryCategory::Albums
                            | LibraryCategory::Artists
                            | LibraryCategory::Playlists
                    ))
            {
                a.library = a.store.library_tracks();
                refresh(&ui, &mut a);
            } else {
                a.library = a.store.library_tracks();
                refresh_playlists(&ui, &mut a);
            }
        }
    });
    bind!(on_toggle_dislike, |ui, state, index| {
        let mut a = state.borrow_mut();
        if let Some(track) = selected(&a, index) {
            a.store.remember(track.clone());
            a.store.toggle_dislike_track(&track);
            a.library = a.store.library_tracks();
            let disliked = a.store.is_disliked(&track);
            let liked = a.store.is_liked(&track);
            crate::track_model::update_path(&ui, &track.path, |row| row.liked = liked);
            if a.current
                .as_ref()
                .is_some_and(|current| current.path == track.path)
            {
                ui.set_current_disliked(disliked);
                ui.set_current_liked(liked);
            }
            let refresh_home = ui.get_active_view() == "home";
            if refresh_home {
                a.home_recommendation_loaded = false;
            }
            persist(&ui, &a);
            if ui.get_active_view() == "library"
                && matches!(
                    a.library_view.category,
                    LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists
                )
            {
                refresh(&ui, &mut a);
            } else {
                refresh_playlists(&ui, &mut a);
            }
            drop(a);
            if refresh_home {
                load_home_recommendations(&state, &ui);
            }
        }
    });
    bind!(on_scan, |ui, state| {
        scan(state, &ui);
    });
    bind!(on_library_view_changed, |ui, state| {
        let mut a = state.borrow_mut();
        a.library_view.query = ui.get_library_query().to_string();
        a.library_view.filter = LibraryFilter::from_index(ui.get_library_filter_index());
        a.library_view.sort = LibrarySort::from_index(ui.get_library_sort_index());
        a.library_view.category = LibraryCategory::from_index(ui.get_library_category_index());
        a.collection_representatives.clear();
        ui.set_library_collections(model(Vec::new()));
        refresh(&ui, &mut a);
    });
    bind!(on_open_library_collection, |ui, state, kind, key| {
        let mut a = state.borrow_mut();
        if kind == "playlist" {
            a.playlist = key.to_string();
            if let Some(playlist) = a
                .store
                .playlists
                .iter()
                .find(|playlist| playlist.id == key.as_str())
            {
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
        let mut local_paths = a.local_paths.clone();
        local_paths.extend(a.store.downloads.values().map(|path| normalize_path(path)));
        let Some(collection) = a
            .library_view
            .collections(&a.store, &source, |track| {
                is_downloaded_in(&a, &local_paths, &track.path)
            })
            .into_iter()
            .find(|collection| collection.kind == kind.as_str() && collection.key == key.as_str())
        else {
            return;
        };
        a.visible = collection.tracks.clone();
        ui.set_catalog_from_home(false);
        ui.set_catalog_parent_id("".into());
        ui.set_catalog_parent_json("".into());
        ui.set_catalog_title(collection.title.clone().into());
        ui.set_catalog_kind(collection.kind.clone().into());
        ui.set_catalog_subtitle(collection.subtitle.clone().into());
        ui.set_catalog_art(art_image(&a, &collection.representative));
        ui.set_catalog_track_count(collection.track_count as i32);
        ui.set_active_view("explore".into());
        ui.set_status("".into());
        refresh(&ui, &mut a);
    });
    bind!(on_library_collection_action, |ui, state, action| {
        let mut a = state.borrow_mut();
        if a.visible.is_empty() {
            return;
        }
        let mut tracks = a.visible.clone();
        if action == "queue" {
            a.queue.append(tracks);
            refresh(&ui, &mut a);
            ui.set_status("Добірку додано до черги".into());
        } else {
            if action == "shuffle" {
                tracks = shuffle_tracks(tracks);
            }
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            let first = a.queue.select(tracks, 0);
            drop(a);
            if let Some(track) = first {
                start(&state, &ui, track);
            }
        }
    });
    bind!(on_library_track_action, |ui, state, index, action| {
        let mut a = state.borrow_mut();
        let Some(track) = a.visible.get(index.max(0) as usize).cloned() else {
            return;
        };
        match action.as_str() {
            "like" => {
                a.store.toggle_like_track(&track);
                a.library = a.store.library_tracks();
                persist(&ui, &a);
                refresh(&ui, &mut a);
            }
            "queue" | "next" => {
                if action == "next" {
                    if a.current.is_none() {
                        a.queue.insert_next([track]);
                        a.queue.index = Some(0);
                        let first = a.queue.tracks.first().cloned();
                        drop(a);
                        if let Some(first) = first {
                            start(&state, &ui, first);
                        }
                        return;
                    }
                    a.queue.insert_next([track]);
                } else {
                    a.queue.append([track]);
                }
                refresh(&ui, &mut a);
                ui.set_status(
                    if action == "next" {
                        "Композицію поставлено наступною"
                    } else {
                        "Композицію додано до черги"
                    }
                    .into(),
                );
            }
            "add" => ui.set_dialog("add".into()),
            "download" => {
                drop(a);
                ui.invoke_download(index);
            }
            "relink" => {
                drop(a);
                ui.invoke_relink_track(index);
            }
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
            if !a.library_view.selected.remove(&key) {
                a.library_view.selected.insert(key);
            }
            refresh(&ui, &mut a);
        }
    });
    bind!(on_library_bulk_action, |ui, state, action| {
        let mut a = state.borrow_mut();
        let selected = selected_library_tracks(&a);
        if action == "select-all" {
            let keys: Vec<_> = a
                .visible
                .iter()
                .map(|track| a.store.identity_key(track))
                .collect();
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
                if !crate::services::key_is_remote(&track.path) {
                    a.store.set_library_excluded(track, excluded);
                }
            }
            a.library_view.selected.clear();
            a.library = a.store.library_tracks();
            persist(&ui, &a);
            ui.set_status(if excluded {
                "Композиції прибрано з Vey. Файли залишилися на пристрої.".into()
            } else {
                "Композиції повернуто до бібліотеки".into()
            });
            refresh(&ui, &mut a);
        } else if action == "like" {
            let all_liked =
                !selected.is_empty() && selected.iter().all(|track| a.store.is_liked(track));
            for track in &selected {
                if a.store.is_liked(track) != !all_liked {
                    a.store.toggle_like_track(track);
                }
            }
            a.library = a.store.library_tracks();
            persist(&ui, &a);
            refresh(&ui, &mut a);
        } else if action == "queue" || action == "next" {
            let start_next = action == "next" && a.current.is_none() && !selected.is_empty();
            if action == "next" {
                a.queue.insert_next(selected);
            } else {
                a.queue.append(selected);
            }
            let next = if start_next {
                a.queue.index = Some(0);
                a.queue.tracks.first().cloned()
            } else {
                None
            };
            if let Some(track) = next {
                drop(a);
                start(&state, &ui, track);
                return;
            }
            refresh(&ui, &mut a);
            ui.set_status("Композиції додано до черги".into());
        } else if action == "play" && !selected.is_empty() {
            a.queue.shuffle = a.store.settings.shuffle;
            a.queue.repeat = a.store.settings.repeat.clone();
            let track = a.queue.select(selected, 0);
            drop(a);
            if let Some(track) = track {
                start(&state, &ui, track);
            }
            return;
        }
    });
    bind!(on_add_selected_to_playlist, |ui, state, id| {
        let mut a = state.borrow_mut();
        let selected = selected_library_tracks(&a);
        for track in selected {
            a.store.remember(track.clone());
            a.store.playlist_add_track(&id, &track);
        }
        a.library_view.selected.clear();
        persist(&ui, &a);
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
                    if let Some(first) = first {
                        start(&state, &ui, first);
                    }
                    return;
                }
                a.queue.insert_next([track]);
            } else {
                a.queue.append([track]);
            }
            refresh(&ui, &mut a);
            ui.set_status(
                if play_next {
                    "Додано наступною композицією"
                } else {
                    "Додано до черги"
                }
                .into(),
            );
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
                        let keys: Vec<_> = a
                            .queue
                            .tracks
                            .iter()
                            .map(|track| a.store.identity_key(track))
                            .collect();
                        if keys == playlist.tracks {
                            a.queue.move_item(from, to);
                        }
                    }
                }
                moved
            }
            _ => false,
        };
        if moved {
            persist(&ui, &a);
            refresh(&ui, &mut a);
        }
    });
    bind!(on_remove_queue_item, |ui, state, index| {
        let mut a = state.borrow_mut();
        if a.queue.remove(index.max(0) as usize).is_some() {
            refresh(&ui, &mut a);
        }
    });
    bind!(on_clear_queue, |ui, state| {
        let mut a = state.borrow_mut();
        a.queue.clear();
        refresh(&ui, &mut a);
        ui.set_status("Чергу очищено. Поточне відтворення триває.".into());
    });
    #[cfg(not(target_os = "android"))]
    bind!(on_relink_track, |ui, state, index| {
        let old = {
            let a = state.borrow();
            selected(&a, index)
        };
        let Some(old) = old else {
            return;
        };
        let old_path = old.path.clone();
        let tx = state.borrow().tx.clone();
        tokio::task::spawn_blocking(move || {
            let result = (|| -> Result<Track, String> {
                let path = crate::dialogs::FileDialog::new()
                    .add_filter("Музика", &["mp3", "flac", "wav", "m4a", "opus", "ogg"])
                    .pick_file()
                    .ok_or_else(|| "Файл не вибрано".to_string())?;
                let scan = core::scan_library(vec![core::LibrarySource {
                    id: "relink".into(),
                    kind: core::LibrarySourceKind::File,
                    path: path.to_string_lossy().into_owned(),
                }]);
                let meta = scan.tracks.into_iter().next().ok_or_else(|| {
                    "У вибраному файлі не знайдено підтримувану композицію".to_string()
                })?;
                Ok(Track {
                    path: meta.path,
                    title: meta.title,
                    artist: meta.artist,
                    album: meta.album,
                    duration_secs: meta.duration_secs,
                    video_id: meta.video_id,
                    thumbnail_url: String::new(),
                    ..Default::default()
                })
            })();
            let _ = tx.send(Event::Relink(old_path, result));
        });
        ui.set_status("Виберіть файл для відновлення композиції".into());
    });
    #[cfg(target_os = "android")]
    bind!(on_relink_track, |ui, state, _index| {
        ui.set_status("Оновлюємо доступні файли пристрою…".into());
        scan(state, &ui);
    });
    #[cfg(target_os = "android")]
    bind!(on_add_source, |ui, state, _kind| {
        // Shared audio is discovered by MediaStore, not a desktop path picker.
        scan(state, &ui);
    });
    #[cfg(not(target_os = "android"))]
    bind!(on_add_source, |ui, state, kind| {
        let paths = if kind == "folder" {
            crate::dialogs::FileDialog::new()
                .pick_folder()
                .map(|p| vec![p])
        } else {
            crate::dialogs::FileDialog::new()
                .add_filter("Музика", &["mp3", "flac", "wav", "m4a", "opus", "ogg"])
                .pick_files()
        };
        if let Some(paths) = paths {
            let mut a = state.borrow_mut();
            for path in paths {
                let id = format!("source-{}-{}", now_ms(), a.store.sources.len());
                a.store.sources.push(Source {
                    id,
                    kind: kind.to_string(),
                    path: path.to_string_lossy().into_owned(),
                });
            }
            a.store.sources = normalize_sources(std::mem::take(&mut a.store.sources));
            persist(&ui, &a);
            drop(a);
            scan(state, &ui);
        }
    });
    bind!(on_add_path, |ui, state, kind, path| {
        if !path.trim().is_empty() {
            let mut a = state.borrow_mut();
            a.store.sources.push(Source {
                id: format!("source-{}", now_ms()),
                kind: kind.to_string(),
                path: path.to_string(),
            });
            a.store.sources = normalize_sources(std::mem::take(&mut a.store.sources));
            persist(&ui, &a);
            drop(a);
            scan(state, &ui);
        }
    });
    bind!(on_remove_source, |ui, state, index| {
        let mut a = state.borrow_mut();
        if index >= 0 && (index as usize) < a.store.sources.len() {
            a.store.sources.remove(index as usize);
            if let Some(cancellation) = a.scan_cancel.take() {
                cancellation.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            a.scanning = false;
            persist(&ui, &a);
            drop(a);
            scan(state, &ui);
        }
    });
    bind!(on_create_playlist, |ui, state, name| {
        let mut a = state.borrow_mut();
        if let Some(id) = a.store.create_playlist(&name) {
            if ui.get_bulk_adding_to_new_playlist() {
                let selected = selected_library_tracks(&a);
                for track in selected {
                    a.store.remember(track.clone());
                    a.store.playlist_add_track(&id, &track);
                }
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
            persist(&ui, &a);
            refresh(&ui, &mut a);
        }
        ui.set_library_action_index(-1);
    });
    bind!(on_rename_playlist, |ui, state, name| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        if a.store.rename_playlist(&id, &name) {
            ui.set_playlist_title(name.trim().into());
            persist(&ui, &a);
            if ui.get_account_signed_in() {
                ui.invoke_account_command("push".into());
            }
            refresh(&ui, &mut a);
        } else {
            ui.set_status("Введіть назву плейлиста".into());
        }
    });
    bind!(on_open_playlist, |ui, state, id| {
        let mut a = state.borrow_mut();
        if id == "liked" {
            ui.set_active_view("liked".into());
        } else if id == "history" {
            ui.set_active_view("home".into());
        } else {
            a.playlist = id.to_string();
            ui.set_active_view("playlist".into());
            if let Some(p) = a.store.playlists.iter().find(|p| p.id == a.playlist) {
                ui.set_playlist_title(p.name.clone().into());
            }
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
        if let Some(track) = track {
            start(&state, &ui, track);
        }
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
            a.store.remember(t.clone());
            a.store.playlist_add_track(&id, &t);
            persist(&ui, &a);
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
                if p.tracks
                    .get(position)
                    .is_some_and(|path| path == &t.path || path == &identity)
                {
                    p.tracks.remove(position);
                    p.updated_at = crate::services::now_ms();
                }
            }
            persist(&ui, &a);
            refresh(&ui, &mut a);
        }
    });
    bind!(on_delete_playlist, |ui, state| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        a.store.playlists.retain(|p| p.id != id);
        persist(&ui, &a);
        ui.set_active_view("library".into());
        refresh(&ui, &mut a);
    });
    bind!(on_set_playlist_public, |ui, state, is_public| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        if let Some(p) = a.store.playlists.iter_mut().find(|p| p.id == id) {
            p.is_public = is_public;
            p.updated_at = crate::services::now_ms();
            persist(&ui, &a);
            refresh(&ui, &mut a);
            if ui.get_account_signed_in() {
                if ui.get_account_busy() {
                    ui.set_status(
                        "Зміни збережено. Після поточної операції синхронізуйте бібліотеку".into(),
                    );
                } else {
                    ui.invoke_account_command("push".into());
                    ui.set_status(if is_public {
                        "Плейлист опубліковано; синхронізацію бібліотеки запущено".into()
                    } else {
                        "Плейлист приховано; синхронізацію бібліотеки запущено".into()
                    });
                }
            } else {
                ui.set_status(if is_public {
                    "Плейлист позначено публічним. Під’єднайте акаунт Vey для показу у профілі"
                        .into()
                } else {
                    "Плейлист приховано з профілю".into()
                });
            }
        }
    });
    bind!(on_request_lyrics, |ui, state| {
        if state.borrow().alternatives.is_empty() {
            lyrics_request(state, &ui, false);
        }
    });
    bind!(on_refresh_lyrics, |ui, state| {
        let needs_dacubeking_auth = {
            let a = state.borrow();
            a.current.as_ref().is_some_and(|track| {
                let provider = a.store.lyrics_request_provider(track);
                matches!(provider.as_str(), "auto" | "dacubeking")
                    && a.jwt.token().is_none()
                    && crate::services::track_video_id(track).is_some()
            })
        };
        if needs_dacubeking_auth {
            ui.invoke_open_challenge();
            return;
        }
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
        lyrics_request(state, &ui, true);
    });
    bind!(on_choose_provider, |ui, state, index| {
        let mut a = state.borrow_mut();
        let index = index.max(0) as usize;
        if let (Some(track), Some(alt)) = (a.current.clone(), a.alternatives.get(index).cloned()) {
            a.store.remember_lyrics_choice(&track, &alt);
            persist(&ui, &a);
            show_lyrics(&ui, &a, index);
        }
    });
    bind!(on_adjust_offset, |ui, state, delta| {
        let mut a = state.borrow_mut();
        if let Some(t) = a.current.clone() {
            let key = a.store.identity_key(&t);
            let value = a.store.offsets.entry(key).or_default();
            *value = (*value + delta as i64).clamp(-60000, 60000);
            ui.set_lyrics_offset(*value as i32);
            persist(&ui, &a);
        }
    });

    bind!(
        on_lyric_mode_changed,
        |ui, state, stage, reduced, passive| {
            let mut a = state.borrow_mut();
            a.store.settings.lyrics_stage = stage;
            a.store.settings.lyrics_reduced_motion = reduced;
            a.store.settings.lyrics_passive_scroll = passive;
            persist(&ui, &a);
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
        persist(&ui, &a);
        ui.set_dialog("".into());
    });
    bind!(on_import_lyric_font, |ui, _state| {
        if let Some(path) = crate::dialogs::FileDialog::new()
            .add_filter("TTF / OTF", &["ttf", "otf"])
            .pick_file()
        {
            match crate::native_lyric_fonts::import(&path) {
                Ok(family) => {
                    ui.set_lyric_font_family(family.into());
                    crate::lyric_appearance::preview(&ui);
                }
                Err(error) => ui.set_lyric_style_status(error.into()),
            }
        }
    });
    bind!(on_provider_changed, |ui, state, value| {
        let mut a = state.borrow_mut();
        a.store.settings.lyrics_provider = value.to_string();
        persist(&ui, &a);
        drop(a);
        lyrics_request(state, &ui, true);
    });
    bind!(on_import_lyrics, |ui, state, raw| {
        let mut a = state.borrow_mut();
        if let Some(t) = a.current.clone() {
            if let Some((mut parsed, format, sync_type)) = crate::lyrics::parse_payload(&raw) {
                if sync_type != LyricsSyncType::Plain {
                    crate::lyrics::complete_timing(
                        &mut parsed,
                        (t.duration_secs.max(0.) * 1000.) as u64,
                    );
                }
                a.lyric_generation += 1;
                if let Some(task) = a.lyric_task.take() {
                    task.abort();
                }
                a.alternatives = vec![LyricsAlternative {
                    provider: "Власні слова".into(),
                    sync_type,
                    format: format.into(),
                    lyrics: parsed,
                }];
                let alts = a.alternatives.clone();
                a.store.remember_lyrics_choice(&t, &alts[0]);
                a.store.cache_lyrics(t.path.clone(), alts);
                persist(&ui, &a);
                report(&ui, ls::save_cache(&t, &a.alternatives));
                show_lyrics(&ui, &a, 0);
            } else {
                ui.set_lyrics_status("Не вдалося розібрати слова".into());
            }
        }
    });
    bind!(on_copy_lyrics, |ui, state| {
        let a = state.borrow();
        if let Some(alt) = a.alternatives.get(ui.get_provider_index().max(0) as usize) {
            let _text = crate::lyrics::serialize_lrc(&alt.lyrics);
            #[cfg(not(target_os = "android"))]
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                report(&ui, clipboard.set_text(_text).map_err(|e| e.to_string()));
            }
        }
    });
    bind!(on_download, |ui, state, index| {
        let mut a = state.borrow_mut();
        if let Some(t) = selected(&a, index) {
            if a.download_pending.remove(&t.path) {
                a.download_generations.remove(&t.path);
                if let Some(task) = a.download_tasks.remove(&t.path) {
                    task.abort();
                }
                a.download_progress.remove(&t.path);
                refresh_media_row(&ui, &a, &t.path);
                ui.set_status("Завантаження скасовано".into());
                return;
            }
            let saved_path = local_file(&a, &t);
            let mapped_path = a.store.downloads.get(&t.path).cloned();
            if t.video_id.is_some() || saved_path.is_some() {
                if !a.download_pending.insert(t.path.clone()) {
                    return;
                }
                let tx = a.tx.clone();
                let download_key = t.path.clone();
                a.download_serial = a.download_serial.wrapping_add(1);
                let serial = a.download_serial;
                a.download_generations.insert(download_key.clone(), serial);
                a.download_progress.insert(download_key.clone(), -1);
                let progress_key = download_key.clone();
                refresh_media_row(&ui, &a, &t.path);
                ui.set_status("Перевірка та завантаження файлу…".into());
                let task = a.rt.spawn(async move {
                    let (saved, missing) = tokio::task::spawn_blocking(move || inspect_local_download(saved_path, mapped_path)).await.unwrap_or_default();
                    if let Some(path) = missing { let _ = tx.send(Event::MissingDownload(t.path.clone(), path)); }
                    if let Some(path) = saved {
                        let _ = tx.send(Event::DownloadLocal(t.path, serial, path));
                        return;
                    }
                    let Some(id) = t.video_id.clone() else { let _ = tx.send(Event::Downloaded(t, serial, Ok(None))); return; };
                    let progress_tx = tx.clone();
                    let result = core::ytm_download_track_with_cover_progress(
                        id,
                        t.title.clone(),
                        t.artist.clone(),
                        Some(t.album.clone()),
                        Some(t.thumbnail_url.clone()),
                        move |current, total| {
                            let _ = progress_tx.send(Event::DownloadProgress(
                                progress_key.clone(),
                                serial,
                                current,
                                total,
                            ));
                        },
                    )
                    .await;
                    let _ = tx.send(Event::Downloaded(t, serial, result));
                });
                a.download_tasks.insert(download_key, task.abort_handle());
            }
        }
    });
    bind!(on_choose_cover, |_ui, state| {
        let a = state.borrow();
        let Some(track) = a.current.clone() else {
            return;
        };
        let Some(path) = local_file(&a, &track) else {
            return;
        };
        let tx = a.tx.clone();
        a.rt.spawn_blocking(move || {
            let result = (|| -> Result<Option<image::RgbaImage>, String> {
                let Some(selected) = crate::dialogs::FileDialog::new()
                    .add_filter("Обкладинки", &["jpg", "jpeg", "png", "webp"])
                    .pick_file()
                else {
                    return Ok(None);
                };
                if std::fs::metadata(&selected)
                    .map_err(|e| e.to_string())?
                    .len()
                    > crate::artwork::MAX_BYTES as u64
                {
                    return Err("Обкладинка завелика".into());
                }
                let bytes = std::fs::read(selected).map_err(|e| e.to_string())?;
                crate::media_library::replace_cover(std::path::Path::new(&path), &bytes)?;
                let img = crate::artwork::decode(&bytes)?;
                for key in [&path, &track.path] {
                    crate::artwork::store_image(key, &img);
                }
                Ok(Some(img))
            })();
            match result {
                Ok(Some(img)) => {
                    let _ = tx.send(Event::Art(path, Ok(img.clone())));
                    let _ = tx.send(Event::Art(track.path, Ok(img)));
                    let _ = tx.send(Event::Message("Обкладинку збережено у файл".into()));
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = tx.send(Event::Message(e));
                }
            }
        });
    });
    bind!(on_save_cover, |_ui, state| {
        let a = state.borrow();
        let Some(track) = a.current.clone() else {
            return;
        };
        let local = local_file(&a, &track);
        let tx = a.tx.clone();
        a.rt.spawn(async move {
            let destination = tokio::task::spawn_blocking(|| {
                crate::dialogs::FileDialog::new()
                    .add_filter("JPEG", &["jpg"])
                    .set_file_name("cover.jpg")
                    .save_file()
            })
            .await;
            let Ok(Some(destination)) = destination else {
                return;
            };
            let result = async {
                let bytes = if let Some(path) = local {
                    tokio::task::spawn_blocking(move || crate::artwork::local(&path))
                        .await
                        .map_err(|e| e.to_string())?
                        .ok_or("У файлі немає обкладинки")?
                } else if !track.thumbnail_url.is_empty() {
                    crate::media_library::fetch_cover(&track.thumbnail_url).await?
                } else {
                    return Err("Обкладинка недоступна".into());
                };
                tokio::task::spawn_blocking(move || {
                    let bytes = crate::media_library::jpeg(&bytes)?;
                    std::fs::write(destination, bytes).map_err(|e| e.to_string())
                })
                .await
                .map_err(|e| e.to_string())?
            }
            .await;
            let _ = tx.send(Event::Message(match result {
                Ok(()) => "Обкладинку збережено".into(),
                Err(e) => e,
            }));
        });
    });
    bind!(on_radio, |ui, state| {
        ui.set_active_view("explore".into());
        browse(state, &ui, None, true);
    });
    bind!(on_open_login, |ui, _state| {
        report(&ui, core::open_ytm_login());
    });
    bind!(on_clear_history, |ui, state| {
        let mut a = state.borrow_mut();
        a.store.history.clear();
        persist(&ui, &a);
        ui.set_status("Історію очищено".into());
    });
    bind!(on_migrate, |ui, state| {
        let a = state.borrow();
        let tx = a.tx.clone();
        let mut store = a.store.clone();
        let revision = a.store_revision.get();
        ui.set_status("Виберіть файл для перенесення…".into());
        a.rt.spawn_blocking(move || {
            if let Some(path) = crate::dialogs::FileDialog::new().add_filter("Експорт localStorage", &["json"]).pick_file() {
                let result = std::fs::read_to_string(path).map_err(|e| e.to_string())
                    .and_then(|raw| store.migrate(&raw).map(|count| (store, count)));
                let _ = tx.send(Event::Migration(revision, result));
            } else { let _ = tx.send(Event::Message(String::new())); }
        });
    });
    bind!(on_backup, |ui, state| {
        let a = state.borrow();
        let tx = a.tx.clone();
        let store = a.store.clone();
        ui.set_status("Збереження резервної копії…".into());
        a.rt.spawn_blocking(move || {
            if let Some(path) = crate::dialogs::FileDialog::new().set_file_name("playervf-backup.json").save_file() {
                let result = serde_json::to_vec_pretty(&store).map_err(|e| e.to_string())
                    .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
                let _ = tx.send(Event::Message(result.map(|_| "Резервну копію збережено".to_string()).unwrap_or_else(|e| e)));
            } else { let _ = tx.send(Event::Message(String::new())); }
        });
    });
    bind!(on_restore, |ui, state| {
        let a = state.borrow();
        let tx = a.tx.clone();
        let revision = a.store_revision.get();
        ui.set_status("Виберіть резервну копію…".into());
        a.rt.spawn_blocking(move || {
            if let Some(path) = crate::dialogs::FileDialog::new().add_filter("Vey", &["json"]).pick_file() {
                let result = std::fs::read(path).map_err(|e| e.to_string())
                    .and_then(|bytes| serde_json::from_slice::<Store>(&bytes).map_err(|e| e.to_string()));
                let _ = tx.send(Event::Restore(revision, result));
            } else { let _ = tx.send(Event::Message(String::new())); }
        });
    });
    bind!(on_sync_tags, |ui, state| {
        let a = state.borrow();
        if let Some(t) = a
            .current
            .clone()
            .filter(|t| !crate::services::key_is_remote(&t.path))
        {
            let tx = a.tx.clone();
            ui.set_status("Пошук відповідного запису YouTube Music…".into());
            a.rt.spawn(async move {
                let result = async {
                    let id = core::resolve_youtube_video_id(
                        t.title.clone(),
                        t.artist.clone(),
                        Some(t.duration_secs),
                    )
                    .await?
                    .ok_or("Точного збігу не знайдено")?;
                    let songs =
                        core::ytm_search_songs(format!("{} {}", t.title, t.artist), Some(30))
                            .await?;
                    let song = songs
                        .into_iter()
                        .find(|s| s.video_id == id)
                        .ok_or("Не вдалося підтвердити метадані збігу")?;
                    Ok(local_sync::TagSnapshot {
                        title: song.title,
                        artist: song.artist,
                        album: song.album,
                        video_id: Some(id),
                        ..Default::default()
                    })
                }
                .await;
                let _ = tx.send(Event::Match(t.path, result));
            });
        }
    });
    bind!(on_apply_tags, |ui, state, title, artist, album| {
        let mut a = state.borrow_mut();
        if let Some(path) = a.pending_tag_path.take() {
            let tx = a.tx.clone();
            let tags = local_sync::TagSnapshot {
                title: title.to_string(),
                artist: artist.to_string(),
                album: album.to_string(),
                video_id: a.pending_tag_video_id.take(),
                ..Default::default()
            };
            ui.set_status("Збереження тегів і резервної копії…".into());
            a.rt.spawn(async move {
                let msg =
                    match local_sync::local_write_tags(storage::data_dir(), path, tags, Some(true))
                        .await
                    {
                        Ok(_) => {
                            "Теги збережено. Оригінали доступні через «Відновити теги».".into()
                        }
                        Err(e) => e,
                    };
                let _ = tx.send(Event::Message(msg));
            });
        }
    });
    bind!(on_restore_tags, |_ui, state| {
        let a = state.borrow();
        if let Some(t) = a
            .current
            .clone()
            .filter(|t| !crate::services::key_is_remote(&t.path))
        {
            let tx = a.tx.clone();
            a.rt.spawn(async move {
                let msg = match local_sync::local_restore_tags(storage::data_dir(), t.path).await {
                    Ok(_) => "Теги відновлено".into(),
                    Err(e) => e,
                };
                let _ = tx.send(Event::Message(msg));
            });
        }
    });
    bind!(on_submit_auth, |ui, state, token| {
        let a = state.borrow();
        let tx = a.tx.clone();
        ui.set_jwt_status("Перевірка…".into());
        a.rt.spawn(async move {
            let mut jwt = ls::JwtCache::load();
            let result = jwt.exchange(token.to_string()).await.map(|_| jwt);
            let _ = tx.send(Event::Jwt(result));
        });
    });
    bind!(on_open_challenge, |ui, state| {
        let mut a = state.borrow_mut();
        if a.challenge_in_flight {
            ui.set_jwt_status("Перевірка вже відкрита".into());
            return;
        }
        a.challenge_in_flight = true;
        let tx = a.tx.clone();
        let rt = a.rt.clone();
        drop(a);
        ui.set_jwt_status("Завантажуємо перевірку…".into());
        ui.set_lyrics_status("Відкриваємо перевірку доступу над текстом пісні…".into());
        ui.set_lyrics_auth_active(true);
        ui.set_lyrics_auth_loading(true);
        #[cfg(target_os = "windows")]
        rt.spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            let _ = tx.send(Event::OpenEmbeddedChallenge);
        });
        #[cfg(not(target_os = "windows"))]
        {
            ui.set_lyrics_auth_active(false);
            ui.set_lyrics_auth_loading(false);
            ui.set_jwt_status("Вбудована перевірка недоступна; відкриваємо браузер".into());
            ui.set_lyrics_status("Завершіть перевірку у браузері й поверніться до Vey".into());
            start_system_challenge(rt, tx);
        }
    });
    refresh(&w, &mut a.borrow_mut());
    let weak = w.as_weak();
    let shared = a.clone();
    let timer = slint::Timer::default();
    let mut last_status = Instant::now();
    let mut last_seek_serial = 0;
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    let mut last_media_update = Instant::now();
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    let mut last_media_playing = None;
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    let mut last_media_seek_serial = 0;
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    let mut media_window_ready_at: Option<Instant> = None;
    #[cfg(target_os = "android")]
    let mut last_permission_check = Instant::now();
    #[cfg(target_os = "android")]
    let mut last_android_media_update = Instant::now();
    #[cfg(target_os = "android")]
    let mut last_android_playing = None;
    let ui_trace = crate::player_debug::Recorder::start();
    let mut last_ui_tick = Instant::now();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
            if let Some(trace) = &ui_trace {
                let now = Instant::now();
                trace.record(
                    &format!("{}/{}", w.get_active_view(), w.get_player_tab()),
                    "app_ui_timer_interval",
                    now.duration_since(last_ui_tick).as_secs_f64() * 1000.,
                );
                last_ui_tick = now;
            }
            let _ui_tick_span =
                crate::player_debug::begin_span(&w, "app_ui_timer_callback");
            #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
            {
                let now = Instant::now();
                let window_ready = if w.window().is_visible() {
                    now >= *media_window_ready_at.get_or_insert(now + Duration::from_millis(500))
                } else {
                    media_window_ready_at = None;
                    false
                };
                let need_session = {
                    let app = shared.borrow();
                    window_ready && app.media_session.is_none()
                        && app.media_session_retry_at.is_none_or(|retry| now >= retry)
                };
                if need_session {
                    match crate::media_session::MediaSession::new(&w) {
                        Ok(mut session) => {
                            #[cfg(debug_assertions)]
                            eprintln!("[media-session] connected");
                            if let Some(track) = shared.borrow().current.as_ref() {
                                session.set_track(track);
                            }
                            let mut app = shared.borrow_mut();
                            app.media_session = Some(session);
                            app.media_session_retry_at = None;
                        }
                        Err(error) => {
                            eprintln!("Vey system media controls unavailable: {error}");
                            shared.borrow_mut().media_session_retry_at =
                                Some(now + Duration::from_secs(5));
                        }
                    }
                }
                let events: Vec<_> = shared
                    .borrow()
                    .media_session
                    .as_ref()
                    .map(|session| session.drain_events().collect())
                    .unwrap_or_default();
                for event in events {
                    handle_media_event(&shared, &w, event);
                }
            }
            #[cfg(target_os = "android")]
            if last_permission_check.elapsed() >= Duration::from_millis(500) {
                last_permission_check = Instant::now();
                let waiting = shared.borrow().permission_wait_started;
                if let Some(started) = waiting {
                    if crate::android::has_audio_permission().unwrap_or(false) {
                        scan(&shared, &w);
                    } else if (started.elapsed() >= Duration::from_secs(2)
                        && crate::android::has_window_focus())
                        || started.elapsed() >= Duration::from_secs(120)
                    {
                        shared.borrow_mut().permission_wait_started = None;
                        crate::android::finish_permission_request();
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                let expired = shared
                    .borrow()
                    .embedded_challenge
                    .as_ref()
                    .is_some_and(|challenge| Instant::now() >= challenge.deadline);
                if expired {
                    let mut app = shared.borrow_mut();
                    app.embedded_challenge.take();
                    let _ = app.tx.send(Event::ChallengeTimeout);
                } else if let Some(bounds) = challenge_bounds(&w) {
                    let mut app = shared.borrow_mut();
                    if let Some(challenge) = app.embedded_challenge.as_mut() {
                        if bounds != challenge.bounds {
                            let _span = crate::player_debug::begin_span(
                                &w,
                                "embedded_browser_set_bounds",
                            );
                            match challenge.webview.set_bounds(bounds) {
                                Ok(()) => challenge.bounds = bounds,
                                Err(_) => {
                                    app.embedded_challenge.take();
                                    let _ = app.tx.send(Event::ChallengeIpc(
                                        crate::challenge::ChallengeSignal::BrowserFallback,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            let batch_start = Instant::now();
            let mut playlist_art_changed = false;
            for event in rx.try_iter().take(8) {
                let mut a = shared.borrow_mut();
                match event {
                    Event::Match(path, result) => match result {
                        Ok(tags) => {
                            a.pending_tag_path = Some(path);
                            a.pending_tag_video_id = tags.video_id.clone();
                            w.set_match_title(tags.title.into());
                            w.set_match_artist(tags.artist.into());
                            w.set_match_album(tags.album.into());
                            w.set_dialog("match".into());
                            w.set_status("".into());
                        }
                        Err(e) => w.set_status(e.into()),
                    },
                    Event::Scan(generation, tracks, status, missing) => {
                        if generation != a.scan_generation { continue; }
                        a.scanning = false;
                        a.scan_cancel = None;
                        a.source_status = status.clone();
                        for (key, path) in missing {
                            if a.store.downloads.get(&key) == Some(&path) { a.store.downloads.remove(&key); }
                        }
                        a.local_paths = tracks
                            .iter()
                            .filter(|t| !crate::services::key_is_remote(&t.path))
                            .map(|t| normalize_path(&t.path))
                            .collect();
                        // A tagged local file also proves that the saved
                        // `yt://` identity is playable here without streaming.
                        for track in &tracks {
                            if let Some(id) = track
                                .video_id
                                .as_deref()
                                .filter(|id| crate::services::video_id(id))
                            {
                                a.store
                                    .downloads
                                    .entry(format!("yt://{id}"))
                                    .or_insert_with(|| track.path.clone());
                            }
                        }
                        a.store.remember_all(tracks);
                        a.store.migrate_identities();
                        a.library = a.store.library_tracks();
                        w.set_status(status.into());
                        persist(&w, &a);
                        refresh(&w, &mut a);
                    }
                    #[cfg(target_os = "android")]
                    Event::ScanFailed(generation, error) => {
                        if generation != a.scan_generation { continue; }
                        a.scanning = false;
                        a.scan_cancel = None;
                        a.source_status = error.clone();
                        w.set_library_scanning(false);
                        w.set_library_source_status(error.clone().into());
                        w.set_status(error.into());
                    }
                    Event::LibraryCollections(generation, result) => {
                        let current = a.collection_jobs.finish(generation);
                        if current && w.get_active_view() == "library"
                            && matches!(a.library_view.category, LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists)
                        {
                            w.set_library_collections_loading(false);
                            match result {
                                Ok(collections) => {
                                    a.collection_representatives = collections.iter()
                                        .map(|collection| collection.representative.clone()).collect();
                                    w.set_library_collections(model(collections.iter().map(|collection| LibraryCollectionRow {
                                        kind: collection.kind.clone().into(),
                                        key: collection.key.clone().into(),
                                        title: collection.title.clone().into(),
                                        subtitle: if collection.kind == "playlist" { String::new() } else { collection.subtitle.clone() }.into(),
                                        count: collection.track_count.to_string().into(),
                                        art: art_image(&a, &collection.representative),
                                    }).collect()));
                                    if !a.snapshot {
                                        for collection in &collections {
                                            load_art(&mut a, &collection.representative);
                                        }
                                    }
                                }
                                Err(error) => w.set_status(error.into()),
                            }
                        }
                        if let Some(next) = a.collection_jobs.start_pending() {
                            start_collection_job(&mut a, next);
                        }
                    }
                    Event::Collection(id, result) => {
                        if id == a.search_generation {
                            match result {
                                Ok(page) => {
                                    let catalog = page.catalog;
                                    a.visible = page.tracks;
                                    w.set_catalog_track_count(a.visible.len() as i32);
                                    a.store.artist_page_catalog = catalog.clone();
                                    crate::discovery::sync_artist_page(&w, &catalog);
                                    refresh(&w, &mut a);
                                    w.set_status("".into());
                                    drop(a);
                                    crate::discovery::request_artist_art(&w, &catalog);
                                }
                                Err(_) => w.set_status(
                                    "Не вдалося завантажити добірки. Спробуйте ще раз.".into(),
                                ),
                            }
                        }
                    }
                    Event::Search(id, key, result) => {
                        if id == a.search_generation {
                            match result {
                                Ok(tracks) => {
                                    if let Some(key) = key {
                                        a.search_cache.put(&key, tracks.clone());
                                    }
                                    let mut merged = if w.get_active_view() == "search" {
                                        a.visible.clone()
                                    } else {
                                        vec![]
                                    };
                                    merged.extend(tracks);
                                    a.visible = crate::services::deduplicate_tracks(merged);
                                    if a.visible.iter().any(|track| a.store.is_liked(track)) {
                                        a.library = a.store.library_tracks();
                                    }
                                    refresh(&w, &mut a);
                                    w.set_status("".into());
                                }
                                Err(e) => w.set_status(e.into()),
                            }
                        }
                    }
                    Event::HomeRecommendations(id, result) => {
                        if id == a.home_recommendation_generation {
                            a.home_recommendation_task.take();
                            a.home_recommendation_loading = false;
                            a.home_recommendation_loaded = true;
                            w.set_home_recommendation_loading(false);
                            match result {
                                Ok((tracks, personalized)) if !tracks.is_empty() => {
                                    a.store.recommendations.updated_at = crate::services::now_ms();
                                    a.store.recommendations.personalized = personalized;
                                    a.store.recommendations.tracks = tracks.clone();
                                    persist(&w, &a);
                                    a.home_recommendations = tracks;
                                    w.set_home_recommendation_failed(false);
                                    w.set_home_recommendation_personalized(personalized);
                                    let mut local_paths = a.local_paths.clone();
                                    local_paths.extend(
                                        a.store.downloads.values().map(|p| normalize_path(p)),
                                    );
                                    crate::track_model::set_home_recommendations(
                                        &w,
                                        a.home_recommendations
                                            .iter()
                                            .map(|track| row(&a, track, &local_paths))
                                            .collect(),
                                    );
                                    for track in a.home_recommendations.clone().into_iter().rev() {
                                        queue_art(&mut a, &track, true);
                                    }
                                    if let Some(track) = a.current.clone() {
                                        queue_art(&mut a, &track, true);
                                    }
                                }
                                Ok(_) | Err(_) => {
                                    if a.home_recommendations.is_empty() {
                                        a.home_recommendations = crate::home::local_recommendations(&a.store, 12);
                                        if !a.home_recommendations.is_empty() {
                                            a.store.recommendations.updated_at = crate::services::now_ms();
                                            a.store.recommendations.personalized = false;
                                            a.store.recommendations.tracks = a.home_recommendations.clone();
                                            persist(&w, &a);
                                            let mut local_paths = a.local_paths.clone();
                                            local_paths.extend(a.store.downloads.values().map(|p| normalize_path(p)));
                                            crate::track_model::set_home_recommendations(
                                                &w,
                                                a.home_recommendations.iter().map(|track| row(&a, track, &local_paths)).collect(),
                                            );
                                            for track in a.home_recommendations.clone() { queue_art(&mut a, &track, false); }
                                            w.set_home_recommendation_personalized(false);
                                            w.set_home_recommendation_failed(false);
                                        } else {
                                            w.set_home_recommendation_failed(true);
                                        }
                                    } else {
                                        w.set_home_recommendation_failed(false);
                                    }
                                }
                            }
                        }
                    }
                    Event::DownloadProgress(path, serial, current, total) => {
                        if a.download_generations.get(&path) == Some(&serial) {
                            let progress = total
                                .filter(|total| *total > 0)
                                .map(|total| ((current as f64 / total as f64) * 95.0).floor() as i32)
                                .unwrap_or(-1);
                            a.download_progress
                                .insert(path.clone(), progress.clamp(-1, 95));
                            refresh_media_row(&w, &a, &path);
                        }
                    }
                    Event::Downloaded(track, serial, result) => {
                        if a.download_generations.get(&track.path) != Some(&serial) { continue; }
                        a.download_generations.remove(&track.path);
                        a.download_pending.remove(&track.path);
                        a.download_progress.remove(&track.path);
                        a.download_tasks.remove(&track.path);
                        match result {
                            Ok(Some(path)) => {
                                a.store.downloads.insert(track.path.clone(), path.clone());
                                a.store.remember(track.clone());
                                let mut local = track.clone();
                                local.path = path.clone();
                                local.video_id = track.video_id.clone();
                                a.local_paths.insert(normalize_path(&path));
                                a.store.remember(local.clone());
                                a.store.migrate_identities();
                                a.library = a.store.library_tracks();
                                let source_id =
                                    format!("download-{}", track.video_id.as_deref().unwrap_or(""));
                                a.store.sources.retain(|s| s.id != source_id);
                                a.store.sources.push(Source {
                                    id: source_id,
                                    kind: "file".into(),
                                    path: normalize_path(&path),
                                });
                                load_art(&mut a, &local);
                                persist(&w, &a);
                                refresh(&w, &mut a);
                                w.set_status(format!("Збережено з обкладинкою: {path}").into());
                            }
                            Ok(None) => w.set_status("Завантаження недоступне".into()),
                            Err(error) => w.set_status(error.into()),
                        }
                        refresh_media_row(&w, &a, &track.path);
                        if let Some(local) = a.store.downloads.get(&track.path) {
                            refresh_media_row(&w, &a, local);
                        }
                    }
                    Event::DownloadLocal(key, serial, path) => {
                        if a.download_generations.get(&key) == Some(&serial) && a.download_pending.remove(&key) {
                            a.download_generations.remove(&key);
                            a.download_progress.remove(&key); a.download_tasks.remove(&key);
                            refresh_media_row(&w, &a, &key);
                            w.set_status(format!("Файл доступний локально: {path}").into());
                        }
                    }
                    Event::Relink(old_path, result) => match result {
                        Ok(replacement) => {
                            let old_normalized = normalize_path(&old_path);
                            let new_normalized = normalize_path(&replacement.path);
                            let old_key = a.store.tracks.iter()
                                .find(|track| track.path == old_path)
                                .map(|track| a.store.identity_key(track));
                            if a.store.relink_local_path(&old_path, replacement.clone()) {
                                let new_key = a.store.identity_key(&replacement);
                                if old_key.as_ref().is_some_and(|key| a.library_view.selected.remove(key)) {
                                    a.library_view.selected.insert(new_key);
                                }
                                for queued in &mut a.queue.tracks {
                                    if queued.path == old_path { *queued = replacement.clone(); }
                                }
                                a.local_paths.remove(&old_normalized);
                                a.local_paths.insert(new_normalized);
                                a.library = a.store.library_tracks();
                                persist(&w, &a);
                                w.set_status("Файл повторно прив’язано; збережені списки й уподобання залишилися".into());
                                refresh(&w, &mut a);
                            } else {
                                w.set_status("Не вдалося знайти цей запис у бібліотеці".into());
                            }
                        }
                        Err(error) if error == "Файл не вибрано" => {}
                        Err(error) => w.set_status(error.into()),
                    },
                    Event::Loaded(serial, path, result) => {
                        if a.load_generation.load(std::sync::atomic::Ordering::SeqCst) == serial
                            && a.current.as_ref().is_some_and(|t| t.path == path)
                        {
                            a.load_task.take();
                            a.loading = false;
                            match result {
                                Ok(p) => {
                                    a.loaded_path = p.clone();
                                    a.ended = false;
                                    let t = a.current.clone().unwrap();
                                    a.last_playing = Some((t.clone(), p));
                                    a.store.record(&t);
                                    persist(&w, &a);
                                    w.set_status("".into());
                                    if w.get_active_view() == "library"
                                        && a.library_view.sort == LibrarySort::RecentlyPlayed
                                        && matches!(a.library_view.category, LibraryCategory::Albums | LibraryCategory::Artists | LibraryCategory::Playlists)
                                    {
                                        refresh(&w, &mut a);
                                    } else {
                                        refresh_history_sidebar_art(&w, &mut a, &t);
                                    }
                                    drop(a);
                                    // Lyrics already start independently of audio loading.
                                }
                                Err(e) => {
                                    w.set_status(e.into());
                                    if let Some((t, p)) = a.last_playing.clone() {
                                        a.current = Some(t.clone());
                                        a.loaded_path = p;
                                        w.set_current_track_key(t.path.clone().into());
                                        w.set_current_title(t.title.clone().into());
                                        w.set_current_artist(t.artist.clone().into());
                                        w.set_current_album(t.album.clone().into());
                                        w.set_current_art(art_image(&a, &t));
                                        w.set_current_liked(a.store.is_liked(&t));
                                        w.set_current_disliked(a.store.is_disliked(&t));
                                        w.set_current_video_id(
                                            t.video_id.clone().unwrap_or_default().into(),
                                        );
                                        w.set_current_clip_video_id(
                                            t.clip_video_id.clone().unwrap_or_default().into(),
                                        );
                                        w.set_lyrics_offset(a.store.offset_for(&t) as i32);
                                        if let Some(task) = a.lyric_task.take() {
                                            task.abort();
                                        }
                                        a.alternatives.clear();
                                        w.set_lyric_lines(model(vec![]));
                                        w.set_lyrics_document("".into());
                                        drop(a);
                                        lyrics_request(&shared, &w, false);
                                    } else {
                                        w.set_player_state("stopped".into());
                                    }
                                }
                            }
                        }
                    }
                    Event::LyricsSupplement(path, generation, result) => {
                        if generation == a.lyric_generation
                            && a.current.as_ref().is_some_and(|track| track.path == path)
                        {
                            a.lyric_task.take();
                            if let Ok(alts) = result {
                                if let Some(track) = a.current.clone() {
                                    let identity = a.store.identity_key(&track);
                                    a.kugou_supplement_attempts.insert(identity);
                                }
                                let alts = crate::braccato::providers::merge_refreshed_alternatives(alts, &a.alternatives);
                                a.store.cache_lyrics(path, alts.clone());
                                a.alternatives = alts;
                                persist(&w, &a);
                                show_preferred_lyrics(&w, &a);
                            }
                        }
                    }
                    Event::LyricsPreview(path, generation, alts) => {
                        if generation == a.lyric_generation
                            && a.current.as_ref().is_some_and(|t| t.path == path)
                        {
                            a.alternatives = crate::braccato::providers::merge_refreshed_alternatives(alts, &a.alternatives);
                            let saved = a.alternatives.clone();
                            a.store.cache_lyrics(path, saved);
                            persist(&w, &a);
                            show_preferred_lyrics(&w, &a);
                        }
                    }
                    Event::Lyrics(path, generation, result, jwt) => {
                        a.jwt = jwt;
                        w.set_jwt_status(if a.jwt.token().is_some() { "JWT збережено · не прострочено" } else { "Потрібна перевірка" }.into());
                        if a.current.as_ref().is_some_and(|t| t.path == path)
                            && generation == a.lyric_generation
                        {
                            a.lyric_task.take();
                            match result {
                                Ok(alts) => {
                                    let alts = crate::braccato::providers::merge_refreshed_alternatives(alts, &a.alternatives);
                                    a.store.cache_lyrics(path, alts.clone());
                                    a.alternatives = alts;
                                    persist(&w, &a);
                                    show_preferred_lyrics(&w, &a);
                                }
                                Err(e) => w.set_lyrics_status(e.into()),
                            }
                        }
                    }
                    Event::Enriched(key, result) => {
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
                                    if let Some(key) =
                                        a.store.lyrics_enrichment.keys().next().cloned()
                                    {
                                        a.store.lyrics_enrichment.remove(&key);
                                    }
                                }
                                a.store.lyrics_enrichment.insert(key.clone(), value);
                                a.enrichment_attempts.borrow_mut().remove(&key);
                                persist(&w, &a);
                                if current_key.as_ref() == Some(&key) {
                                    show_lyrics(&w, &a, index);
                                }
                            }
                            Err(error) => {
                                if current_key.as_ref() == Some(&key) {
                                    w.set_lyrics_enrichment_status(error.into());
                                }
                            }
                        }
                    }
                    Event::RetryArt(path) => {
                        let track = a
                            .visible
                            .iter()
                            .find(|track| track.path == path)
                            .cloned()
                            .or_else(|| a.current.as_ref().filter(|track| track.path == path).cloned());
                        if let Some(track) = track {
                            queue_art(&mut a, &track, true);
                        } else {
                            a.art_attempts.remove(&path);
                        }
                    }
                    Event::ArtCached(path) => {
                        refresh_media_art_row(&w, &a, &path);
                        playlist_art_changed |= w.get_active_view() == "playlist"
                            && a.visible.iter().any(|track| track.path == path);
                        refresh_playlist_artwork(&w, &a);
                        if a.current.as_ref().is_some_and(|track| track.path == path) {
                            if let Some(art) = a.art.get(&path) {
                                w.set_current_art(art.clone());
                            }
                        }
                        let collection_rows = w.get_library_collections();
                        if let Some(rows) = collection_rows.as_any().downcast_ref::<VecModel<LibraryCollectionRow>>() {
                            for (index, track) in a.collection_representatives.iter().enumerate() {
                                if art_keys(&a, track).iter().any(|key| key == &path) {
                                    if let Some(mut row) = rows.row_data(index) {
                                        row.art = art_image(&a, track);
                                        rows.set_row_data(index, row);
                                    }
                                }
                            }
                        }
                    }
                    Event::Art(path, result) => {
                        a.art_pending.remove(&path);
                        let no_artwork_match =
                            matches!(&result, Err(error) if error == ART_NO_MATCH);
                        if let Ok(img) = result {
                            a.art_failed.remove(&path);
                            a.art_attempts.remove(&path);
                            let art = slint::Image::from_rgba8(
                                slint::SharedPixelBuffer::clone_from_slice(
                                    img.as_raw(),
                                    img.width(),
                                    img.height(),
                                ),
                            );
                            let mut related = vec![path.clone()];
                            if let Some(track) = a.store.tracks.iter().find(|t| t.path == path) {
                                for key in art_keys(&a, track) {
                                    if !related.contains(&key) {
                                        related.push(key);
                                    }
                                }
                            } else if let Some(local) = a.store.downloads.get(&path) {
                                if !related.contains(local) {
                                    related.push(local.clone());
                                }
                            }
                            for key in &related {
                                a.art_failed.remove(key);
                                a.art.insert(key.clone(), art.clone());
                                crate::discovery::update_art(&w, key, &art);
                                refresh_media_art_row(&w, &a, key);
                            }
                            playlist_art_changed |= w.get_active_view() == "playlist"
                                && a.visible.iter().any(|track| related.contains(&track.path));
                            refresh_playlist_artwork(&w, &a);
                            if a.current
                                .as_ref()
                                .is_some_and(|t| related.contains(&t.path))
                            {
                                w.set_current_art(art.clone());
                            }
                            if w.get_active_view() == "explore"
                                && w.get_catalog_kind() != ""
                                && a.visible.first().is_some_and(|track| related.contains(&track.path))
                            {
                                w.set_catalog_art(art.clone());
                            }
                            let collection_rows = w.get_library_collections();
                            if let Some(rows) = collection_rows.as_any().downcast_ref::<VecModel<LibraryCollectionRow>>() {
                                for (index, track) in a.collection_representatives.iter().enumerate() {
                                    if art_keys(&a, track).iter().any(|key| related.contains(key)) {
                                        if let Some(mut row) = rows.row_data(index) {
                                            let next = art_image(&a, track);
                                            if row.art != next {
                                                row.art = next;
                                                rows.set_row_data(index, row);
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            let attempts = a.art_attempts.get(&path).copied().unwrap_or(1);
                            let track = a
                                .visible
                                .iter()
                                .find(|track| track.path == path)
                                .cloned()
                                .or_else(|| a.current.as_ref().filter(|track| track.path == path).cloned());
                            let can_retry = !no_artwork_match
                                && attempts < MAX_ART_ATTEMPTS
                                && track.as_ref().is_some_and(|track| {
                                    !crate::artwork::remote_urls(track).is_empty()
                                        || local_file(&a, track).is_some()
                                });
                            if can_retry {
                                let tx = a.tx.clone();
                                let delay = ART_RETRY_DELAY;
                                a.rt.spawn(async move {
                                    tokio::time::sleep(delay).await;
                                    let _ = tx.send(Event::RetryArt(path));
                                });
                            } else {
                                a.art_failed.retain(|_, failed| failed.elapsed() < ART_FAILURE_COOLDOWN);
                                if a.art_failed.len() >= 256 {
                                    a.art_failed.clear();
                                }
                                a.art_failed.insert(path, Instant::now());
                            }
                        }
                        pump_art(&mut a);
                    }
                    Event::Message(msg) => w.set_status(msg.into()),
                    Event::MissingDownload(key, path) => {
                        if a.store.downloads.get(&key) == Some(&path) {
                            a.store.downloads.remove(&key); refresh_media_row(&w, &a, &key); persist(&w, &a);
                        }
                    }
                    Event::Migration(revision, result) => match result {
                        Ok((store, count)) if a.store_revision.get() == revision => {
                            a.store = store; persist(&w, &a); refresh(&w, &mut a); w.set_status(format!("Перенесено {count} записів").into());
                        }
                        Ok(_) => w.set_status("Бібліотека змінилася під час перенесення. Спробуйте ще раз.".into()),
                        Err(error) => w.set_status(error.into()),
                    },
                    Event::Restore(revision, result) => match result {
                        Ok(_) if a.store_revision.get() != revision => w.set_status("Бібліотека змінилася під час відновлення. Спробуйте ще раз.".into()),
                        Ok(store) if store.version == 1 => {
                            a.store = store; apply_settings(&w, &a); persist(&w, &a); refresh(&w, &mut a);
                            w.set_status("Резервну копію відновлено".into());
                        }
                        Ok(_) => w.set_status("Непідтримувана версія копії".into()),
                        Err(error) => w.set_status(error.into()),
                    },
                    #[cfg(target_os = "windows")]
                    Event::OpenEmbeddedChallenge => {
                        if let Err(error) = open_embedded_challenge(&w, &mut a) {
                            eprintln!("[lyrics-auth] embedded WebView unavailable; using system browser: {error}");
                            w.set_lyrics_auth_active(false);
                            w.set_lyrics_auth_loading(false);
                            w.set_jwt_status("Вбудована перевірка не запустилась; відкриваємо браузер".into());
                            w.set_lyrics_status("Завершіть перевірку у браузері й поверніться до Vey".into());
                            start_system_challenge(a.rt.clone(), a.tx.clone());
                        }
                    }
                    #[cfg(target_os = "windows")]
                    Event::ChallengeIpc(signal) => match signal {
                        crate::challenge::ChallengeSignal::Token(token) => {
                            a.embedded_challenge.take();
                            w.set_lyrics_auth_active(false);
                            w.set_lyrics_auth_loading(false);
                            w.set_jwt_status("Обмінюємо токен перевірки…".into());
                            let tx = a.tx.clone();
                            a.rt.spawn(async move {
                                let mut jwt = ls::JwtCache::load();
                                let result = jwt.exchange(token).await.map(|_| jwt);
                                let _ = tx.send(Event::Jwt(result));
                            });
                        }
                        crate::challenge::ChallengeSignal::Cancel => {
                            a.embedded_challenge.take();
                            a.challenge_in_flight = false;
                            w.set_lyrics_auth_active(false);
                            w.set_lyrics_auth_loading(false);
                            w.set_jwt_status("Перевірку скасовано".into());
                            w.set_lyrics_status("Перевірку скасовано. Натисніть «Оновити слова», щоб повторити".into());
                        }
                        crate::challenge::ChallengeSignal::BrowserFallback => {
                            a.embedded_challenge.take();
                            w.set_lyrics_auth_active(false);
                            w.set_lyrics_auth_loading(false);
                            w.set_jwt_status("Перевірка у браузері — запасний варіант".into());
                            w.set_lyrics_status("Завершіть перевірку у браузері й поверніться до Vey".into());
                            start_system_challenge(a.rt.clone(), a.tx.clone());
                        }
                        crate::challenge::ChallengeSignal::FrameLoaded => {
                            w.set_jwt_status("Перевірка завантажена".into());
                        }
                    },
                    #[cfg(target_os = "windows")]
                    Event::ChallengeTimeout => {
                        a.embedded_challenge.take();
                        a.challenge_in_flight = false;
                        w.set_lyrics_auth_active(false);
                        w.set_lyrics_auth_loading(false);
                        w.set_jwt_status("Час перевірки вичерпано".into());
                        w.set_lyrics_status("Час минув. Натисніть «Оновити слова», щоб спробувати знову".into());
                    }
                    Event::Jwt(result) => {
                        a.challenge_in_flight = false;
                        #[cfg(target_os = "windows")]
                        {
                            a.embedded_challenge.take();
                            w.set_lyrics_auth_active(false);
                            w.set_lyrics_auth_loading(false);
                        }
                        match result {
                            Ok(jwt) => {
                                a.jwt = jwt;
                                w.set_jwt_status("JWT збережено · не прострочено".into());
                                drop(a);
                                lyrics_request(&shared, &w, true);
                            }
                            Err(e) => {
                                w.set_jwt_status(e.into());
                                w.set_lyrics_status("Доступ не отримано. Натисніть «Оновити слова», щоб повторити перевірку".into());
                            }
                        }
                    }
                }
                // Yield to painting/input even when workers finish together.
                if batch_start.elapsed() >= Duration::from_millis(3) {
                    break;
                }
            }
            if playlist_art_changed && w.get_active_view() == "playlist" {
                crate::track_model::rebind_playlist(&w);
            }
            #[cfg(target_os = "android")]
            if last_status.elapsed() >= Duration::from_millis(100) {
                if let Ok(event) = crate::android::media_poll() {
                    if !event.is_empty() {
                        handle_android_media_event(&shared, &w, &event);
                    }
                }
            }
            let mut a = shared.borrow_mut();
            if a.save_due.get().is_some_and(|due| Instant::now() >= due) {
                a.save_due.set(None);
                a.saver.request(a.store.clone());
            }
            // Deliver results at frame cadence; poll the clock independently.
            if last_status.elapsed() < Duration::from_millis(100) {
                return;
            }
            last_status = Instant::now();
            if !a.loading && !a.loaded_path.is_empty() {
                let status_started = Instant::now();
                let status = core::player_status(&a.audio);
                crate::player_debug::record_for_window(
                    &w,
                    "player_status_poll",
                    status_started.elapsed().as_secs_f64() * 1000.,
                );
                if let Ok(s) = status {
                    if s.current_path.as_deref() != Some(a.loaded_path.as_str()) {
                        return;
                    }
                    if s.seek_serial != last_seek_serial {
                        last_seek_serial = s.seek_serial;
                        w.set_native_seek_revision(w.get_native_seek_revision().wrapping_add(1));
                    }
                    w.set_position(s.position_secs as f32);
                    w.set_duration(s.duration_secs as f32);
                    w.set_time_label(
                        format!(
                            "{} / {}",
                            fmt_time(s.position_secs),
                            fmt_time(s.duration_secs)
                        )
                        .into(),
                    );
                    w.set_player_state(
                        match s.state {
                            core::PlaybackState::Playing => "playing",
                            core::PlaybackState::Paused => "paused",
                            _ => "stopped",
                        }
                        .into(),
                    );
                    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
                    if let Some(session) = a.media_session.as_mut() {
                        let playing = s.state == core::PlaybackState::Playing;
                        if last_media_playing != Some(playing)
                            || last_media_seek_serial != s.seek_serial
                            || last_media_update.elapsed() >= Duration::from_secs(1)
                        {
                            let _span = crate::player_debug::begin_span(
                                &w,
                                "media_session_set_playback",
                            );
                            session.set_playback(playing, s.position_secs);
                            last_media_playing = Some(playing);
                            last_media_seek_serial = s.seek_serial;
                            last_media_update = Instant::now();
                        }
                    }
                    #[cfg(target_os = "android")]
                    if let Some(track) = a.current.as_ref().filter(|_| {
                        last_android_media_update.elapsed() >= Duration::from_secs(1)
                            || last_android_playing
                                != Some(s.state == core::PlaybackState::Playing)
                    }) {
                        last_android_media_update = Instant::now();
                        last_android_playing = Some(s.state == core::PlaybackState::Playing);
                        let _ = crate::android::media_update(
                            &serde_json::json!({
                                "title": track.title, "artist": track.artist, "album": track.album,
                                "duration": s.duration_secs, "position": s.position_secs,
                                "playing": s.state == core::PlaybackState::Playing, "active": true
                            }).to_string(),
                        );
                    }
                    if s.ended && !a.ended {
                        a.ended = true;
                        if let Some(track) = a.current.clone() {
                            a.store.record_completion(&track);
                            persist(&w, &a);
                        }
                        let next = a.queue.advance(false, true);
                        drop(a);
                        if let Some(t) = next {
                            start(&shared, &w, t);
                        }
                    }
                }
            }
        },
    );
    if snapshot {
        crate::snapshots::schedule(&w, a.clone());
    } else {
        scan(&a, &w);
    }
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
                let newer_recommendations =
                    cloud.recommendations.updated_at > state.store.recommendations.updated_at;
                state.store.recommendations = cloud.recommendations;
                state.store.offsets = cloud.offsets;
                state.store.lyrics_choices = cloud.lyrics_choices;
                state.store.settings = cloud.settings;
                state.store.downloads = cloud.downloads;
                state.store.migrate_identities();
                state.library = state.store.library_tracks();
                if newer_recommendations {
                    state.home_recommendation_generation =
                        state.home_recommendation_generation.wrapping_add(1);
                    if let Some(task) = state.home_recommendation_task.take() {
                        task.abort();
                    }
                    state.home_recommendation_loading = false;
                    state.home_recommendation_loaded =
                        !state.store.recommendations.tracks.is_empty();
                    ui.set_home_recommendation_loading(false);
                }
                show_saved_recommendations(&ui, &mut state);
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
                let _ = state.audio.volume(state.store.settings.volume);
                refresh(&ui, &mut state);
                persist(&ui, &state);
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
                app.store.personalized_home_catalog = next.personalized_home_catalog;
                persist(&ui, &app);
                refresh(&ui, &mut app);
            }
        })
    };
    let discovery_connection_changed = {
        let state = a.clone();
        let weak = w.as_weak();
        Rc::new(move || {
            state.borrow_mut().store.personalized_home_catalog = crate::discovery::Catalog::default();
            if let Some(ui) = weak.upgrade() {
                ui.invoke_sync_home();
                ui.invoke_request_home_catalog();
            }
        })
    };
    let account_revision = {
        let state = a.clone();
        Rc::new(move || state.borrow().store_revision.get())
    };
    let _music_timer = crate::music_accounts::install(
        &w,
        account_get.clone(),
        account_apply.clone(),
        account_revision.clone(),
        a.borrow().rt.clone(),
        discovery_connection_changed,
    );
    crate::discovery::install(&w, discovery_get, discovery_apply);
    let _account_timer = crate::account::install(
        &w, account_get, account_apply, account_revision, a.borrow().rt.clone(),
    );
    let _search_timer = slint::Timer::default();
    let weak = w.as_weak();
    let state = a.clone();
    _search_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(80),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let query = {
                let mut a = state.borrow_mut();
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
                    browse(&state, &ui, Some(query), false);
                }
            }
        },
    );
    {
        let mut state = a.borrow_mut();
        show_saved_recommendations(&w, &mut state);
        state.home_recommendation_loaded = !state.home_recommendations.is_empty();
    }
    if a.borrow()
        .store
        .recommendations
        .updated_at
        .saturating_add(3_600_000)
        <= crate::services::now_ms()
        || a.borrow().home_recommendations.is_empty()
    {
        load_home_recommendations(&a, &w);
    }
    let recommendation_timer = slint::Timer::default();
    let recommendation_state = a.clone();
    let recommendation_window = w.as_weak();
    recommendation_timer.start(
        slint::TimerMode::Repeated,
        Duration::from_secs(60),
        move || {
            if let Some(ui) = recommendation_window.upgrade() {
                if recommendation_state
                    .borrow()
                    .store
                    .recommendations
                    .updated_at
                    .saturating_add(3_600_000)
                    <= crate::services::now_ms()
                {
                    load_home_recommendations(&recommendation_state, &ui);
                }
            }
        },
    );
    let result = w.run();
    let app = a.borrow();
    if app.save_due.take().is_some() {
        app.saver.request(app.store.clone());
    }
    result
}

#[cfg(test)]
mod artwork_queue_tests {
    use super::*;

    #[test]
    fn local_download_check_preserves_valid_files_and_reports_missing_mapping() {
        let path = std::env::temp_dir().join(format!("vey-local-check-{}-{}.tmp", std::process::id(), crate::services::now_ms()));
        std::fs::write(&path, b"fixture").unwrap();
        let valid = path.to_string_lossy().into_owned();
        let missing = format!("{valid}.missing");
        assert_eq!(inspect_local_download(Some(valid.clone()), Some(valid.clone())), (Some(valid.clone()), None));
        assert_eq!(inspect_local_download(Some(missing.clone()), Some(missing.clone())), (None, Some(missing.clone())));
        assert_eq!(inspect_local_download(Some(valid.clone()), Some(missing.clone())), (Some(valid), Some(missing)));
        std::fs::remove_file(path).unwrap();
    }

    fn track(path: &str) -> Track {
        Track {
            path: path.into(),
            ..Default::default()
        }
    }

    #[test]
    fn priority_deduplication_and_reset_preserve_order() {
        let mut queue = ArtworkQueue::default();
        queue.push(&track("a"), false);
        queue.push(&track("b"), false);
        queue.push(&track("c"), false);
        queue.push(&track("c"), true);
        queue.push(&track("a"), false);
        assert_eq!(queue.pop().unwrap().path, "c");
        assert_eq!(queue.pop().unwrap().path, "a");
        queue.retain_paths(&Default::default());
        queue.push(&track("b"), true);
        assert_eq!(queue.pop().unwrap().path, "b");
        assert!(queue.pop().is_none());
    }

    #[test]
    fn selected_cover_has_one_reserved_slot() {
        assert!(can_start_art(MAX_ART_IN_FLIGHT - 1, false));
        assert!(!can_start_art(MAX_ART_IN_FLIGHT, false));
        assert!(can_start_art(MAX_ART_IN_FLIGHT, true));
        assert!(!can_start_art(MAX_ART_IN_FLIGHT + 1, true));
    }

    #[test]
    fn collection_jobs_coalesce_and_reject_stale_results() {
        let mut jobs = CollectionJobs::default();
        let first = jobs.request().unwrap();
        assert!(jobs.request().is_none());
        assert!(jobs.request().is_none());
        assert!(!jobs.finish(first));
        let latest = jobs.start_pending().unwrap();
        assert_eq!(latest, 3);
        assert!(jobs.finish(latest));
        assert!(jobs.start_pending().is_none());

        let before_navigation = jobs.request().unwrap();
        jobs.cancel();
        assert!(!jobs.finish(before_navigation));
        assert!(jobs.start_pending().is_none());
        let current = jobs.request().unwrap();
        assert!(jobs.finish(current));
    }

    #[test]
    fn collection_projection_runs_on_worker_with_download_filter() {
        let tracks = vec![
            Track {
                path: "C:/music/one.flac".into(),
                title: "One".into(),
                artist: "Artist".into(),
                album: "Album A".into(),
                ..Default::default()
            },
            Track {
                path: "C:/music/two.flac".into(),
                title: "Two".into(),
                artist: "Artist".into(),
                album: "Album B".into(),
                ..Default::default()
            },
        ];
        let store = Store {
            downloads: [("yt://abcdefghijk".into(), "C:/music/one.flac".into())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let view = LibraryViewState {
            category: LibraryCategory::Albums,
            filter: LibraryFilter::Downloaded,
            ..Default::default()
        };
        let rows = std::thread::spawn(move || {
            build_library_collections(view, store, tracks, Default::default())
        })
        .join()
        .expect("collection worker");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Album A");
    }
}
