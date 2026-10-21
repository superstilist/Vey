//! Slint callbacks are thin controllers over Rust services.
use crate::audio::AudioEngine;
use crate::{local_sync, lyrics::*, lyrics_service as ls, native as core, services::*, storage, *};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;
type Shared = Rc<RefCell<App>>;
struct App {
    store: Store,
    visible: Vec<Track>,
    page_results: std::collections::HashMap<String, Vec<Track>>,
    library: Vec<Track>,
    local_paths: std::collections::HashSet<String>,
    queue: Queue,
    current: Option<Track>,
    playlist: String,
    alternatives: Vec<LyricsAlternative>,
    enrichment_attempts: RefCell<std::collections::HashSet<String>>,
    audio: Arc<core::CoreState>,
    rt: Arc<Runtime>,
    tx: mpsc::Sender<Event>,
    loading: bool,
    scanning: bool,
    #[cfg(target_os = "android")]
    permission_wait_started: Option<Instant>,
    search_generation: u64,
    lyric_generation: u64,
    loaded_path: String,
    ended: bool,
    load_generation: Arc<std::sync::atomic::AtomicU64>,
    load_task: Option<tokio::task::JoinHandle<()>>,
    download_pending: std::collections::HashSet<String>,
    load_gate: Arc<std::sync::Mutex<()>>,
    last_playing: Option<(Track, String)>,
    pending_tag_path: Option<String>,
    art: std::collections::HashMap<String, slint::Image>,
    art_pending: std::collections::HashSet<String>,
    art_gate: Arc<tokio::sync::Semaphore>,
    jwt: ls::JwtCache,
    snapshot: bool,
    save_due: Cell<Option<Instant>>,
    saver: storage::SaveWorker,
}
enum Event {
    Match(String, Result<local_sync::TagSnapshot, String>),
    Scan(Vec<Track>, String),
    #[cfg(target_os = "android")]
    ScanFailed(String),
    Search(u64, Result<Vec<Track>, String>),
    Loaded(u64, String, Result<String, String>),
    Downloaded(Track, Result<Option<String>, String>),
    Lyrics(String, u64, Result<Vec<LyricsAlternative>, String>),
    Enriched(
        String,
        Result<crate::lyrics_translation::Enrichment, String>,
    ),
    Art(String, Result<image::RgbaImage, String>),
    Message(String),
    Jwt(Result<ls::JwtCache, String>),
    AuthReady(Result<ls::JwtCache, String>),
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
    w.set_lyrics_passive_scroll(s.lyrics_passive_scroll);
    let theme = crate::themes::NativeTheme::default();
    theme.apply(
        &w,
        a.store.settings.lyrics_reduced_motion,
        a.store.settings.lyrics_passive_scroll,
    );
    let _ = a.audio.volume(s.volume);
    let _ = a.audio.rate(s.rate);
}
fn persist(_w: &MainWindow, a: &App) {
    if !a.snapshot && a.save_due.get().is_none() {
        // Coalesce settings bursts before cloning the store, without disk IO.
        a.save_due
            .set(Some(Instant::now() + Duration::from_millis(200)));
    }
}

fn art_image(a: &App, t: &Track) -> slint::Image {
    a.art.get(&t.path).cloned().unwrap_or_default()
}
fn row(a: &App, t: &Track) -> TrackRow {
    TrackRow {
        title: t.title.clone().into(),
        artist: t.artist.clone().into(),
        album: t.album.clone().into(),
        duration: fmt_time(t.duration_secs).into(),
        path: t.path.clone().into(),
        art: art_image(a, t),
        liked: a.store.likes.contains(&t.path),
        downloaded: is_downloaded(a, &t.path),
        downloading: a.download_pending.contains(&t.path),
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
fn local_file(a: &App, track: &Track) -> Option<String> {
    if track.video_id.is_none() {
        Some(track.path.clone())
    } else {
        a.store.downloads.get(&track.path).cloned()
    }
}
fn refresh_media_rows(w: &MainWindow, a: &App) {
    let mut local_paths: std::collections::HashSet<_> = a
        .store
        .downloads
        .values()
        .map(|p| normalize_path(p))
        .collect();
    local_paths.extend(a.local_paths.iter().cloned());
    for model in [
        w.get_tracks(),
        w.get_home_page_tracks(),
        w.get_explore_page_tracks(),
        w.get_library_page_tracks(),
        w.get_liked_page_tracks(),
        w.get_playlist_page_tracks(),
        w.get_queue_page_tracks(),
        w.get_search_page_tracks(),
        w.get_settings_page_tracks(),
        w.get_queue_tracks(),
    ] {
        if let Some(rows) = model.as_any().downcast_ref::<VecModel<TrackRow>>() {
            for i in 0..rows.row_count() {
                if let Some(mut row) = rows.row_data(i) {
                    let downloaded = a.store.downloads.contains_key(row.path.as_str())
                        || local_paths.contains(&normalize_path(row.path.as_str()));
                    let downloading = a.download_pending.contains(row.path.as_str());
                    let mut changed =
                        row.downloaded != downloaded || row.downloading != downloading;
                    row.downloaded = downloaded;
                    row.downloading = downloading;
                    if let Some(art) = a.art.get(row.path.as_str()) {
                        if row.art != *art {
                            row.art = art.clone();
                            changed = true;
                        }
                    }
                    if changed {
                        rows.set_row_data(i, row);
                    }
                }
            }
        }
    }
    if let Some(track) = &a.current {
        w.set_current_downloaded(is_downloaded(a, &track.path));
        w.set_current_downloading(a.download_pending.contains(&track.path));
        w.set_current_local(local_file(a, track).is_some());
    }
}
fn refresh(w: &MainWindow, a: &mut App) {
    let view = w.get_active_view().to_string();
    a.visible = match view.as_str() {
        "home" => {
            if a.store.history.is_empty() {
                a.library.clone()
            } else {
                a.store.history.iter().map(|h| h.track.clone()).collect()
            }
        }
        "library" => a.library.clone(),
        "liked" => a
            .store
            .tracks
            .iter()
            .filter(|t| a.store.likes.contains(&t.path))
            .cloned()
            .collect(),
        "playlist" => a
            .store
            .playlists
            .iter()
            .find(|p| p.id == a.playlist)
            .map(|p| {
                p.tracks
                    .iter()
                    .filter_map(|id| a.store.tracks.iter().find(|t| &t.path == id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        "queue" => a.queue.tracks.clone(),
        _ => a.visible.clone(),
    };
    if !a.snapshot {
        for track in a.visible.clone() {
            load_art(a, &track);
        }
    }
    if matches!(view.as_str(), "search" | "explore") {
        a.page_results.insert(view.clone(), a.visible.clone());
    }
    // Seed all local page objects without navigating or starting network work.
    if !w.get_pages_seeded() {
        let home = if a.store.history.is_empty() {
            a.library.clone()
        } else {
            a.store.history.iter().map(|h| h.track.clone()).collect()
        };
        w.set_home_page_tracks(model(home.iter().map(|t| row(a, t)).collect()));
        w.set_library_page_tracks(model(a.library.iter().map(|t| row(a, t)).collect()));
        w.set_liked_page_tracks(model(
            a.store
                .tracks
                .iter()
                .filter(|t| a.store.likes.contains(&t.path))
                .map(|t| row(a, t))
                .collect(),
        ));
        w.set_queue_page_tracks(model(a.queue.tracks.iter().map(|t| row(a, t)).collect()));
        w.set_pages_seeded(true);
    }
    w.set_tracks(model(a.visible.iter().map(|t| row(a, t)).collect()));
    let mut playlists = vec![
        PlaylistRow {
            id: "liked".into(),
            name: crate::i18n::translate(&a.store.settings.language, "Музика, яка сподобалась")
                .into(),
            detail: crate::i18n::translate(
                &a.store.settings.language,
                &format!("{} композицій · локально", a.store.likes.len()),
            )
            .into(),
        },
        PlaylistRow {
            id: "history".into(),
            name: crate::i18n::translate(&a.store.settings.language, "Послухати пізніше").into(),
            detail: crate::i18n::translate(
                &a.store.settings.language,
                "Локальні списки відтворення",
            )
            .into(),
        },
    ];
    playlists.extend(a.store.playlists.iter().map(|p| {
        PlaylistRow {
            id: p.id.clone().into(),
            name: p.name.clone().into(),
            detail: crate::i18n::translate(
                &a.store.settings.language,
                &format!("{} композицій", p.tracks.len()),
            )
            .into(),
        }
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
    if let Some(t) = &a.current {
        w.set_current_liked(a.store.likes.contains(&t.path));
        w.set_current_downloaded(is_downloaded(a, &t.path));
        w.set_current_downloading(a.download_pending.contains(&t.path));
        w.set_current_local(local_file(a, t).is_some());
        w.set_current_art(art_image(a, t));
        w.set_current_album(t.album.clone().into());
    }
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
    let sources = a
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
    let tx = a.tx.clone();
    a.scanning = true;
    w.set_status("Сканування бібліотеки…".into());
    a.rt.spawn_blocking(move || {
        #[cfg(not(target_os = "android"))]
        let result = core::scan_library(sources);
        #[cfg(target_os = "android")]
        let result = match core::scan_android_library(sources) {
            Ok(result) => result,
            Err(error) => {
                let _ = tx.send(Event::ScanFailed(error));
                return;
            }
        };
        let status = result
            .source_statuses
            .iter()
            .filter_map(|s| s.detail.as_deref())
            .collect::<Vec<_>>()
            .join(" · ");
        let status = if status.is_empty() {
            format!("Знайдено композицій: {}", result.tracks.len())
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
                ..Default::default()
            })
            .collect();
        let _ = tx.send(Event::Scan(tracks, status));
    });
}
fn browse(a: &Shared, w: &MainWindow, query: Option<String>, radio: bool) {
    let mut a = a.borrow_mut();
    a.search_generation += 1;
    let id = a.search_generation;
    let tx = a.tx.clone();
    w.set_status("Завантаження з YouTube Music…".into());
    let video = a.current.as_ref().and_then(|t| t.video_id.clone());
    a.rt.spawn(async move {
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
                    })
                    .collect()
            })
        };
        let _ = tx.send(Event::Search(id, result));
    });
}
fn load_art(a: &mut App, track: &Track) {
    if a.art.contains_key(&track.path) || !a.art_pending.insert(track.path.clone()) {
        return;
    }
    let track = track.clone();
    let local = local_file(a, &track);
    let tx = a.tx.clone();
    let gate = a.art_gate.clone();
    a.rt.spawn(async move {
        // Keep permits until decode completes, including slow local metadata IO.
        let Ok(_permit) = gate.acquire_owned().await else {
            return;
        };
        let key = track.path.clone();
        let result = async {
            use sha1::{Digest, Sha1};
            let dir = storage::data_dir().join("artwork");
            let path = dir.join(format!("{:x}.png", Sha1::digest(track.path.as_bytes())));
            let cache = path.clone();
            if let Some(img) = tokio::task::spawn_blocking(move || crate::artwork::cached(&cache))
                .await
                .map_err(|e| e.to_string())?
            {
                return Ok(img);
            }
            let embedded = if let Some(local) = local {
                tokio::task::spawn_blocking(move || crate::artwork::local(&local))
                    .await
                    .map_err(|e| e.to_string())?
            } else {
                None
            };
            let bytes = if let Some(bytes) = embedded {
                bytes
            } else if !track.thumbnail_url.is_empty() {
                crate::media_library::fetch_cover(&track.thumbnail_url).await?
            } else {
                return Err("No artwork".into());
            };
            tokio::task::spawn_blocking(move || {
                let img = crate::artwork::decode(&bytes)?;
                // Cache writes are best effort; a read-only cache must not hide art.
                if std::fs::create_dir_all(&dir).is_ok() {
                    let tmp = path.with_extension("pending.png");
                    if img.save_with_format(&tmp, image::ImageFormat::Png).is_ok() {
                        let _ = std::fs::rename(tmp, path);
                    }
                }
                Ok(img)
            })
            .await
            .map_err(|e| e.to_string())?
        }
        .await;
        let _ = tx.send(Event::Art(key, result));
    });
}
fn start(a: &Shared, w: &MainWindow, track: Track) {
    let mut a = a.borrow_mut();
    if a.store
        .downloads
        .get(&track.path)
        .is_some_and(|p| !std::path::Path::new(p).is_file())
    {
        a.store.downloads.remove(&track.path);
    }
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
    a.alternatives.clear();
    a.current = Some(track.clone());
    refresh_media_rows(w, &a);
    w.set_current_track_key(track.path.clone().into());
    w.set_current_video_id(track.video_id.clone().unwrap_or_default().into());
    w.set_current_clip_video_id(track.clip_video_id.clone().unwrap_or_default().into());
    w.set_current_title(track.title.clone().into());
    w.set_current_artist(track.artist.clone().into());
    w.set_current_album(track.album.clone().into());
    w.set_current_art(art_image(&a, &track));
    w.set_current_liked(a.store.likes.contains(&track.path));
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
    let saved_path = a
        .store
        .downloads
        .get(&track.path)
        .cloned()
        .filter(|p| std::path::Path::new(p).is_file());
    if let Some(previous) = a.load_task.take() {
        previous.abort();
    }
    load_art(&mut a, &track);
    a.load_task = Some(a.rt.spawn(async move {
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
}
fn lyrics_request(a: &Shared, w: &MainWindow, force: bool) {
    let mut a = a.borrow_mut();
    let Some(track) = a.current.clone() else {
        return;
    };
    a.lyric_generation += 1;
    let generation = a.lyric_generation;
    if !force {
        if let Some(alts) = a
            .store
            .cached_lyrics(&track.path)
            .or_else(|| ls::migrated_cache(&a.store, &track))
        {
            a.alternatives = alts;
            show_preferred_lyrics(w, &a);
            return;
        }
    }
    let tx = a.tx.clone();
    let provider = a.store.lyrics_request_provider(&track);
    let jwt = ls::JwtCache::load().token();
    w.set_lyrics_status("Шукаємо слова пісні…".into());
    a.rt.spawn(async move {
        let result = ls::fetch(&track, &provider, force, jwt).await;
        let _ = tx.send(Event::Lyrics(track.path, generation, result));
    });
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
    w.set_lyrics_document(crate::braccato::web_document(alt, duration_ms).into());
    w.set_lyric_lines(model(crate::braccato::rows(alt, duration_ms)));
}
fn selected(a: &App, index: i32) -> Option<Track> {
    if index < 0 {
        a.current.clone()
    } else {
        a.visible.get(index as usize).cloned()
    }
}
fn report(w: &MainWindow, result: Result<(), String>) {
    if let Err(e) = result {
        w.set_status(e.into());
    }
}

pub fn run() -> Result<(), slint::PlatformError> {
    let w = MainWindow::new()?;
    #[cfg(target_os = "android")]
    w.set_android_platform(true);
    crate::i18n::install(&w);
    crate::material::install(&w);
    crate::renderer::install(&w);
    let (tx, rx) = mpsc::channel();
    let rt = Arc::new(Runtime::new().expect("Rust async runtime"));
    let _video_surface = crate::video::install(&w, rt.clone());
    #[cfg(not(target_os = "windows"))]
    let _ambient_surface = crate::ambient_native::install(&w);
    let widget_smoke = std::env::args().any(|a| a == "--widget-smoke");
    let widget_preview = std::env::args().any(|a| a == "--widget-preview");
    let snapshot = widget_smoke || widget_preview || std::env::args().any(|a| a == "--screenshots");
    let _lyrics_widget = if !snapshot || widget_smoke || widget_preview {
        crate::lyrics_widget::install(&w)
    } else {
        None
    };
    let (mut store, load_error) = if snapshot {
        (Store::default(), None)
    } else {
        match storage::load() {
            Ok(s) => (s, None),
            Err(e) => (Store::default(), Some(e)),
        }
    };
    store
        .downloads
        .retain(|_, path| std::path::Path::new(path).is_file());
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
        local_paths: Default::default(),
        queue: Queue::default(),
        current: None,
        playlist: String::new(),
        alternatives: vec![],
        enrichment_attempts: RefCell::default(),
        audio,
        rt,
        tx,
        loading: false,
        scanning: false,
        #[cfg(target_os = "android")]
        permission_wait_started: None,
        search_generation: 0,
        lyric_generation: 0,
        loaded_path: String::new(),
        ended: false,
        load_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        load_task: None,
        download_pending: Default::default(),
        load_gate: Arc::new(std::sync::Mutex::new(())),
        last_playing: None,
        pending_tag_path: None,
        art: Default::default(),
        art_pending: Default::default(),
        art_gate: Arc::new(tokio::sync::Semaphore::new(2)),
        jwt: ls::JwtCache::load(),
        snapshot,
        save_due: Cell::new(None),
        saver,
    }));
    {
        let a = a.borrow();
        w.set_jwt_status(
            if a.jwt.token().is_some() {
                "JWT чинний"
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
    apply_settings(&w, &a.borrow());
    if let Some(e) = load_error {
        w.set_status(format!("Помилка читання даних: {e}. Оригінальний файл збережено.").into());
    }
    macro_rules! bind {($method:ident, |$ui:ident,$state:ident $(,$arg:ident)*| $body:block)=>{{let weak=w.as_weak();let shared=a.clone();w.$method(move|$($arg),*|{if let Some($ui)=weak.upgrade(){let $state=&shared;$body}});}};}
    let search_timer = Rc::new(slint::Timer::default());
    let edit_timer = Rc::clone(&search_timer);
    bind!(on_search_edited, |ui, _state, query| {
        let weak = ui.as_weak();
        let query = query.to_string();
        edit_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(300),
            move || {
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_search(query.clone().into());
                }
            },
        );
    });
    let submit_timer = Rc::clone(&search_timer);
    bind!(on_navigate, |ui, state, view| {
        ui.set_status("".into());
        state.borrow_mut().search_generation += 1;
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
    });
    bind!(on_refresh_explore, |ui, state| {
        browse(state, &ui, None, false);
    });
    bind!(on_search, |ui, state, query| {
        submit_timer.stop();
        ui.set_active_view("search".into());
        let needle = query.trim().to_lowercase();
        {
            let mut a = state.borrow_mut();
            a.visible = a
                .library
                .iter()
                .filter(|t| {
                    format!("{} {} {}", t.title, t.artist, t.album)
                        .to_lowercase()
                        .contains(&needle)
                })
                .cloned()
                .collect();
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
        let track = state.borrow_mut().queue.advance(false, false);
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
            a.store.toggle_like(&t.path);
            persist(&ui, &a);
            refresh(&ui, &mut a);
        }
    });
    bind!(on_scan, |ui, state| {
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
            persist(&ui, &a);
            drop(a);
            scan(state, &ui);
        }
    });
    bind!(on_create_playlist, |ui, state, name| {
        let mut a = state.borrow_mut();
        if let Some(id) = a.store.create_playlist(&name) {
            if ui.get_adding_to_new_playlist() {
                if let Some(track) = a.current.clone() {
                    a.store.remember(track.clone());
                    a.store.playlist_add(&id, &track.path);
                }
            }
            persist(&ui, &a);
            refresh(&ui, &mut a);
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
    bind!(on_add_to_playlist, |ui, state, id| {
        let mut a = state.borrow_mut();
        if let Some(t) = a.current.clone() {
            a.store.remember(t.clone());
            a.store.playlist_add(&id, &t.path);
            persist(&ui, &a);
            refresh(&ui, &mut a);
        }
    });
    bind!(on_remove_from_playlist, |ui, state, index| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        if let Some(t) = a.visible.get(index.max(0) as usize).cloned() {
            if let Some(p) = a.store.playlists.iter_mut().find(|p| p.id == id) {
                p.tracks.retain(|path| path != &t.path);
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
    bind!(on_request_lyrics, |ui, state| {
        if state.borrow().alternatives.is_empty() {
            lyrics_request(state, &ui, false);
        }
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
            let value = a.store.offsets.entry(t.path).or_default();
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
            if a.store
                .downloads
                .get(&t.path)
                .is_some_and(|p| !std::path::Path::new(p).is_file())
            {
                a.store.downloads.remove(&t.path);
                refresh_media_rows(&ui, &a);
            }
            if let Some(path) = local_file(&a, &t).filter(|p| std::path::Path::new(p).is_file()) {
                ui.set_status(format!("Файл доступний локально: {path}").into());
                return;
            }
            if let Some(id) = t.video_id.clone() {
                if !a.download_pending.insert(t.path.clone()) {
                    return;
                }
                let tx = a.tx.clone();
                refresh_media_rows(&ui, &a);
                ui.set_status("Завантаження файлу й обкладинки…".into());
                a.rt.spawn(async move {
                    let result = core::ytm_download_track_with_cover(
                        id,
                        t.title.clone(),
                        t.artist.clone(),
                        Some(t.album.clone()),
                        Some(t.thumbnail_url.clone()),
                    )
                    .await;
                    let _ = tx.send(Event::Downloaded(t, result));
                });
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
        if let Some(p) = crate::dialogs::FileDialog::new()
            .add_filter("Експорт localStorage", &["json"])
            .pick_file()
        {
            match std::fs::read_to_string(p) {
                Ok(raw) => {
                    let mut a = state.borrow_mut();
                    match a.store.migrate(&raw) {
                        Ok(n) => {
                            persist(&ui, &a);
                            refresh(&ui, &mut a);
                            ui.set_status(format!("Перенесено {n} записів").into());
                        }
                        Err(e) => ui.set_status(e.into()),
                    }
                }
                Err(e) => ui.set_status(e.to_string().into()),
            }
        }
    });
    bind!(on_backup, |ui, state| {
        if let Some(p) = crate::dialogs::FileDialog::new()
            .set_file_name("playervf-backup.json")
            .save_file()
        {
            let a = state.borrow();
            report(
                &ui,
                serde_json::to_vec_pretty(&a.store)
                    .map_err(|e| e.to_string())
                    .and_then(|b| std::fs::write(p, b).map_err(|e| e.to_string())),
            );
        }
    });
    bind!(on_restore, |ui, state| {
        if let Some(p) = crate::dialogs::FileDialog::new()
            .add_filter("Vey", &["json"])
            .pick_file()
        {
            let result = std::fs::read(p)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice::<Store>(&b).map_err(|e| e.to_string()));
            match result {
                Ok(s) if s.version == 1 => {
                    let mut a = state.borrow_mut();
                    a.store = s;
                    apply_settings(&ui, &a);
                    persist(&ui, &a);
                    refresh(&ui, &mut a);
                    ui.set_status("Резервну копію відновлено".into());
                }
                Ok(_) => ui.set_status("Непідтримувана версія копії".into()),
                Err(e) => ui.set_status(e.into()),
            }
        }
    });
    bind!(on_sync_tags, |ui, state| {
        let a = state.borrow();
        if let Some(t) = a.current.clone().filter(|t| t.video_id.is_none()) {
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
        if let Some(t) = a.current.clone().filter(|t| t.video_id.is_none()) {
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
        let a = state.borrow();
        let tx = a.tx.clone();
        ui.set_jwt_status("Перевірка…".into());
        a.rt.spawn(async move {
            let result = crate::challenge::acquire().await;
            let event = match result {
                Ok(token) => {
                    let mut jwt = ls::JwtCache::load();
                    jwt.exchange(token).await.map(|_| jwt)
                }
                Err(e) => Err(e),
            };
            let _ = tx.send(Event::Jwt(event));
        });
    });
    refresh(&w, &mut a.borrow_mut());
    let weak = w.as_weak();
    let shared = a.clone();
    let timer = slint::Timer::default();
    let mut last_status = Instant::now();
    #[cfg(target_os = "android")]
    let mut last_permission_check = Instant::now();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
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
            let batch_start = Instant::now();
            for event in rx.try_iter().take(8) {
                let mut a = shared.borrow_mut();
                match event {
                    Event::Match(path, result) => match result {
                        Ok(tags) => {
                            a.pending_tag_path = Some(path);
                            w.set_match_title(tags.title.into());
                            w.set_match_artist(tags.artist.into());
                            w.set_match_album(tags.album.into());
                            w.set_dialog("match".into());
                            w.set_status("".into());
                        }
                        Err(e) => w.set_status(e.into()),
                    },
                    Event::Scan(tracks, status) => {
                        a.scanning = false;
                        a.store
                            .downloads
                            .retain(|_, path| std::path::Path::new(path).is_file());
                        a.local_paths = tracks
                            .iter()
                            .filter(|t| t.video_id.is_none())
                            .map(|t| normalize_path(&t.path))
                            .collect();
                        a.library = tracks;
                        let tracks = a.library.clone();
                        for t in tracks {
                            a.store.remember(t);
                        }
                        w.set_status(status.into());
                        persist(&w, &a);
                        refresh(&w, &mut a);
                        refresh_media_rows(&w, &a);
                    }
                    #[cfg(target_os = "android")]
                    Event::ScanFailed(error) => {
                        a.scanning = false;
                        w.set_status(error.into());
                    }
                    Event::Search(id, result) => {
                        if id == a.search_generation {
                            match result {
                                Ok(tracks) => {
                                    let mut seen = std::collections::HashSet::new();
                                    let mut merged = if w.get_active_view() == "search" {
                                        a.visible.clone()
                                    } else {
                                        vec![]
                                    };
                                    merged.extend(tracks);
                                    merged.retain(|t| seen.insert(t.path.clone()));
                                    a.visible = merged;
                                    let tracks = a.visible.clone();
                                    for t in tracks.iter() {
                                        a.store.remember(t.clone());
                                    }
                                    refresh(&w, &mut a);
                                    w.set_status("".into());
                                }
                                Err(e) => w.set_status(e.into()),
                            }
                        }
                    }
                    Event::Downloaded(track, result) => {
                        a.download_pending.remove(&track.path);
                        match result {
                            Ok(Some(path)) => {
                                a.store.downloads.insert(track.path.clone(), path.clone());
                                a.store.remember(track.clone());
                                let mut local = track.clone();
                                local.path = path.clone();
                                local.video_id = None;
                                a.local_paths.insert(normalize_path(&path));
                                a.store.remember(local.clone());
                                if !a
                                    .library
                                    .iter()
                                    .any(|t| normalize_path(&t.path) == normalize_path(&path))
                                {
                                    a.library.push(local.clone());
                                }
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
                        refresh_media_rows(&w, &a);
                    }
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
                                    drop(a);
                                    lyrics_request(&shared, &w, false);
                                }
                                Err(e) => {
                                    w.set_status(e.into());
                                    if let Some((t, p)) = a.last_playing.clone() {
                                        a.current = Some(t.clone());
                                        a.loaded_path = p;
                                        w.set_current_title(t.title.into());
                                        w.set_current_artist(t.artist.into());
                                    } else {
                                        w.set_player_state("stopped".into());
                                    }
                                }
                            }
                        }
                    }
                    Event::Lyrics(path, generation, result) => {
                        a.jwt = ls::JwtCache::load();
                        if a.jwt.token().is_some() {
                            w.set_jwt_status("JWT чинний · автоматично".into());
                        }
                        if a.current.as_ref().is_some_and(|t| t.path == path)
                            && generation == a.lyric_generation
                        {
                            match result {
                                Ok(alts) => {
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
                    Event::Art(path, result) => {
                        a.art_pending.remove(&path);
                        // Remember misses for this session as well, avoiding request loops.
                        let art = match result {
                            Ok(img) => slint::Image::from_rgba8(
                                slint::SharedPixelBuffer::clone_from_slice(
                                    img.as_raw(),
                                    img.width(),
                                    img.height(),
                                ),
                            ),
                            Err(_) => slint::Image::default(),
                        };
                        a.art.insert(path.clone(), art.clone());
                        refresh_media_rows(&w, &a);
                        if a.current.as_ref().is_some_and(|t| t.path == path) {
                            w.set_current_art(art);
                        }
                    }
                    Event::Message(msg) => w.set_status(msg.into()),
                    Event::AuthReady(result) => match result {
                        Ok(jwt) => {
                            a.jwt = jwt;
                            w.set_jwt_status("JWT чинний · автоматично".into());
                        }
                        Err(e) => w.set_jwt_status(format!("Автоматична перевірка: {e}").into()),
                    },
                    Event::Jwt(result) => match result {
                        Ok(jwt) => {
                            a.jwt = jwt;
                            w.set_jwt_status("JWT чинний".into());
                            drop(a);
                            lyrics_request(&shared, &w, true);
                        }
                        Err(e) => w.set_jwt_status(e.into()),
                    },
                }
                // Yield to painting/input even when workers finish together.
                if batch_start.elapsed() >= Duration::from_millis(3) {
                    break;
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
                if let Ok(s) = core::player_status(&a.audio) {
                    if s.current_path.as_deref() != Some(a.loaded_path.as_str()) {
                        return;
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
                    if s.ended && !a.ended {
                        a.ended = true;
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
        w.set_jwt_status("Автоматична перевірка…".into());
        let app = a.borrow();
        let tx = app.tx.clone();
        app.rt.spawn(async move {
            let result = ls::prewarm_authentication().await;
            let _ = tx.send(Event::AuthReady(result));
        });
        drop(app);
        scan(&a, &w);
    }
    let result = w.run();
    let app = a.borrow();
    if app.save_due.take().is_some() {
        app.saver.request(app.store.clone());
    }
    result
}
