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
    queue: Queue,
    current: Option<Track>,
    playlist: String,
    alternatives: Vec<LyricsAlternative>,
    audio: Arc<core::CoreState>,
    rt: Arc<Runtime>,
    tx: mpsc::Sender<Event>,
    loading: bool,
    search_generation: u64,
    lyric_generation: u64,
    loaded_path: String,
    ended: bool,
    load_generation: Arc<std::sync::atomic::AtomicU64>,
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
    Search(u64, Result<Vec<Track>, String>),
    Loaded(String, Result<String, String>),
    Lyrics(String, u64, Result<Vec<LyricsAlternative>, String>),
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
    w.set_volume(s.volume);
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
            name: "Музика, яка сподобалась".into(),
            detail: format!("{} композицій · локально", a.store.likes.len()).into(),
        },
        PlaylistRow {
            id: "history".into(),
            name: "Послухати пізніше".into(),
            detail: "Локальні списки відтворення".into(),
        },
    ];
    playlists.extend(a.store.playlists.iter().map(|p| PlaylistRow {
        id: p.id.clone().into(),
        name: p.name.clone().into(),
        detail: format!("{} композицій", p.tracks.len()).into(),
    }));
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
        w.set_current_art(art_image(a, t));
        w.set_current_album(t.album.clone().into());
    }
}
fn scan(a: &Shared, w: &MainWindow) {
    let a = a.borrow();
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
    w.set_status("Сканування бібліотеки…".into());
    a.rt.spawn_blocking(move || {
        let result = core::scan_library(sources);
        let status = result
            .source_statuses
            .iter()
            .filter_map(|s| s.detail.as_deref())
            .collect::<Vec<_>>()
            .join(" · ");
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
            let bytes = if !track.thumbnail_url.is_empty() {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(15))
                    .build()
                    .map_err(|e| e.to_string())?;
                let mut response = client
                    .get(&track.thumbnail_url)
                    .send()
                    .await
                    .and_then(|r| r.error_for_status())
                    .map_err(|e| e.to_string())?;
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                    if bytes.len() + chunk.len() > crate::artwork::MAX_BYTES {
                        return Err("Artwork exceeds size limit".into());
                    }
                    bytes.extend_from_slice(&chunk);
                }
                bytes
            } else {
                tokio::task::spawn_blocking(move || crate::artwork::local(&track.path))
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No artwork")?
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
    a.loading = true;
    a.ended = false;
    a.loaded_path.clear();
    a.lyric_generation += 1;
    a.alternatives.clear();
    a.current = Some(track.clone());
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
    load_art(&mut a, &track);
    a.rt.spawn(async move {
        let path = if let Some(id) = &track.video_id {
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
        let _ = tx.send(Event::Loaded(track.path, result));
    });
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
            show_lyrics(w, &a, 0);
            return;
        }
    }
    let tx = a.tx.clone();
    let provider = a.store.settings.lyrics_provider.clone();
    let jwt = ls::JwtCache::load().token();
    w.set_lyrics_status("Шукаємо слова пісні…".into());
    a.rt.spawn(async move {
        let result = ls::fetch(&track, &provider, force, jwt).await;
        let _ = tx.send(Event::Lyrics(track.path, generation, result));
    });
}
fn show_lyrics(w: &MainWindow, a: &App, index: usize) {
    let Some(alt) = a.alternatives.get(index) else {
        return;
    };
    w.set_providers(model(
        a.alternatives
            .iter()
            .map(|l| format!("{} · {:?}", l.provider, l.sync_type).into())
            .collect(),
    ));
    w.set_lyrics_credits(alt.lyrics.songwriters.join(" · ").into());
    w.set_lyrics_timed(crate::braccato::effective_sync_type(alt) != LyricsSyncType::Plain);
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
    crate::renderer::install(&w);
    let (tx, rx) = mpsc::channel();
    let rt = Arc::new(Runtime::new().expect("Rust async runtime"));
    let widget_smoke = std::env::args().any(|a| a == "--widget-smoke");
    let widget_preview = std::env::args().any(|a| a == "--widget-preview");
    let snapshot = widget_smoke || widget_preview || std::env::args().any(|a| a == "--screenshots");
    let _lyrics_widget = if !snapshot || widget_smoke || widget_preview {
        crate::lyrics_widget::install(&w)
    } else {
        None
    };
    let (store, load_error) = if snapshot {
        (Store::default(), None)
    } else {
        match storage::load() {
            Ok(s) => (s, None),
            Err(e) => (Store::default(), Some(e)),
        }
    };
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
        queue: Queue::default(),
        current: None,
        playlist: String::new(),
        alternatives: vec![],
        audio,
        rt,
        tx,
        loading: false,
        search_generation: 0,
        lyric_generation: 0,
        loaded_path: String::new(),
        ended: false,
        load_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
    bind!(on_search, |ui, state, query| {
        ui.set_active_view("search".into());
        let needle = query.to_lowercase();
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
    bind!(on_rate_changed, |ui, state, value| {
        let mut a = state.borrow_mut();
        a.store.settings.rate = value.clamp(0.5, 2.);
        report(&ui, a.audio.rate(a.store.settings.rate));
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
        ui.set_status("Режим кліпу: відтворення відео ще не перенесено".into());
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
    bind!(on_add_source, |ui, state, kind| {
        #[cfg(target_os = "android")]
        {
            ui.set_source_kind(kind.clone());
            ui.set_dialog("source".into());
            return;
        }
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
        if a.store.create_playlist(&name).is_some() {
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
        lyrics_request(state, &ui, true);
    });
    bind!(on_choose_provider, |ui, state, index| {
        show_lyrics(&ui, &state.borrow(), index.max(0) as usize);
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
                a.alternatives = vec![LyricsAlternative {
                    provider: "Власні слова".into(),
                    sync_type,
                    format: format.into(),
                    lyrics: parsed,
                }];
                let alts = a.alternatives.clone();
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
        let a = state.borrow();
        if let Some(t) = selected(&a, index) {
            if let Some(id) = t.video_id {
                let tx = a.tx.clone();
                ui.set_status("Завантаження файлу…".into());
                a.rt.spawn(async move {
                    let result =
                        core::ytm_download_track(id, t.title, t.artist, Some(t.album)).await;
                    let msg = match result {
                        Ok(Some(p)) => format!("Збережено: {p}"),
                        Ok(None) => "Завантаження недоступне".into(),
                        Err(e) => e,
                    };
                    let _ = tx.send(Event::Message(msg));
                });
            }
        }
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
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
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
                        a.library = tracks;
                        let tracks = a.library.clone();
                        for t in tracks {
                            a.store.remember(t);
                        }
                        w.set_status(status.into());
                        persist(&w, &a);
                        refresh(&w, &mut a);
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
                    Event::Loaded(path, result) => {
                        if a.current.as_ref().is_some_and(|t| t.path == path) {
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
                                    show_lyrics(&w, &a, 0);
                                }
                                Err(e) => w.set_lyrics_status(e.into()),
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
                        let rows = w.get_tracks();
                        if let Some(rows) = rows.as_any().downcast_ref::<VecModel<TrackRow>>() {
                            for (index, track) in a.visible.iter().enumerate() {
                                if track.path == path {
                                    if let Some(mut row) = rows.row_data(index) {
                                        row.art = art.clone();
                                        rows.set_row_data(index, row);
                                    }
                                }
                            }
                        }
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
