//! Rust/WASM controller with the original Braccato rendering widget.
use crate::{lyrics::*, services::*, *};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
#[wasm_bindgen(module = "/web/bridge.js")]
extern "C" {
    #[wasm_bindgen(js_name=pickFiles)]
    fn pick_files() -> js_sys::Promise;
    #[wasm_bindgen(js_name=readJsonFile)]
    fn read_json() -> js_sys::Promise;
    #[wasm_bindgen(js_name=downloadJson)]
    fn download_json(text: &str);
    #[wasm_bindgen(js_name=fileUrl)]
    fn file_url(path: &str) -> String;
    fn challenge() -> js_sys::Promise;
}
struct WebApp {
    store: Store,
    library: Vec<Track>,
    visible: Vec<Track>,
    queue: Queue,
    current: Option<Track>,
    playlist: String,
    audio: web_sys::HtmlAudioElement,
    alternatives: Vec<LyricsAlternative>,
    artwork: HashMap<String, slint::Image>,
    artwork_pending: HashSet<String>,
    generation: u64,
    ended: bool,
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
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Ok(json) = serde_json::to_string(&a.store) {
            let _ = storage.set_item("playervf.rust.v1", &json);
        }
    }
}
fn refresh(w: &MainWindow, a: &mut WebApp) {
    a.visible = match w.get_active_view().as_str() {
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
        "queue" => a.queue.tracks.clone(),
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
        _ => a.visible.clone(),
    };
    w.set_tracks(model(
        a.visible
            .iter()
            .map(|t| TrackRow {
                title: t.title.clone().into(),
                artist: t.artist.clone().into(),
                album: t.album.clone().into(),
                path: t.path.clone().into(),
                duration: format_time(t.duration_secs).into(),
                liked: a.store.likes.contains(&t.path),
                ..Default::default()
            })
            .collect(),
    ));
    w.set_playlists(model(
        a.store
            .playlists
            .iter()
            .map(|p| PlaylistRow {
                id: p.id.clone().into(),
                name: p.name.clone().into(),
                detail: format!("{} композицій · локально", p.tracks.len()).into(),
            })
            .collect(),
    ));
    if let Some(t) = &a.current {
        w.set_current_liked(a.store.likes.contains(&t.path));
    }
}
fn format_time(s: f64) -> String {
    let s = if s.is_finite() { s.max(0.) as u64 } else { 0 };
    format!("{}:{:02}", s / 60, s % 60)
}
fn apply_artwork(w: &MainWindow, image: slint::Image) {
    w.set_current_art(image);
}
fn load_artwork(w: &MainWindow, a: &Rc<RefCell<WebApp>>, track: &Track) {
    if track.thumbnail_url.is_empty() {
        return;
    }
    let path = track.path.clone();
    if let Some(image) = a.borrow().artwork.get(&path).cloned() {
        apply_artwork(w, image);
        return;
    }
    if !a.borrow_mut().artwork_pending.insert(path.clone()) {
        return;
    }
    let url = track.thumbnail_url.clone();
    let weak = w.as_weak();
    let state = a.clone();
    spawn_local(async move {
        let result = async {
            let response = reqwest::get(url)
                .await
                .map_err(|error| error.to_string())?
                .error_for_status()
                .map_err(|error| error.to_string())?;
            if response
                .content_length()
                .is_some_and(|size| size > crate::artwork::MAX_BYTES as u64)
            {
                return Err("Artwork exceeds size limit".to_string());
            }
            let bytes = response.bytes().await.map_err(|error| error.to_string())?;
            let image = crate::artwork::decode(&bytes)?;
            let image = slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
                image.as_raw(),
                image.width(),
                image.height(),
            ));
            Ok(image)
        }
        .await;
        let Some(w) = weak.upgrade() else {
            return;
        };
        let mut app = state.borrow_mut();
        app.artwork_pending.remove(&path);
        if app
            .current
            .as_ref()
            .is_none_or(|current| current.path != path)
        {
            return;
        }
        if let Ok(image) = result {
            app.artwork.insert(path, image.clone());
            apply_artwork(&w, image);
        } else {
            w.set_current_art(slint::Image::default());
        }
    });
}
fn show_lyrics(w: &MainWindow, a: &WebApp, index: usize) {
    if let Some(alt) = a.alternatives.get(index) {
        w.set_lyrics_credits(alt.lyrics.songwriters.join(" · ").into());
        w.set_lyrics_timed(crate::braccato::effective_sync_type(alt) != LyricsSyncType::Plain);
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
    let cached = { a.borrow().store.cached_lyrics(&track.path) };
    if let Some(alts) = cached.filter(|_| !refresh) {
        a.borrow_mut().alternatives = alts;
        show_lyrics(w, &a.borrow(), 0);
        return;
    }
    let provider = a.borrow().store.settings.lyrics_provider.clone();
    let weak = w.as_weak();
    let state = a.clone();
    spawn_local(async move {
        let result = reqwest::Client::new()
            .post(api("/api/lyrics"))
            .query(&[("provider", provider), ("refresh", refresh.to_string())])
            .json(&track)
            .send()
            .await;
        if let Some(w) = weak.upgrade() {
            let a = state.borrow();
            if a.current.as_ref().map(|t| &t.path) != Some(&track.path) {
                return;
            }
            match result {
                Ok(r) if r.status().is_success() => {
                    drop(a);
                    match r.json::<Vec<LyricsAlternative>>().await {
                        Ok(alts) => {
                            let mut a = state.borrow_mut();
                            if a.current.as_ref().map(|t| &t.path) == Some(&track.path) {
                                a.store.cache_lyrics(track.path.clone(), alts.clone());
                                a.alternatives = alts;
                                save(&a);
                                show_lyrics(&w, &a, 0);
                            }
                        }
                        Err(_) => w.set_lyrics_status("Помилка формату слів".into()),
                    }
                }
                _ => w.set_lyrics_status("Слова недоступні. Можна вставити власні.".into()),
            }
        }
    });
}
fn start(w: &MainWindow, a: &Rc<RefCell<WebApp>>, track: Track) {
    let mut state = a.borrow_mut();
    let src = track
        .video_id
        .as_ref()
        .map(|id| format!("/api/audio/{id}"))
        .unwrap_or_else(|| {
            if track.path.starts_with("browser://") {
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
    state.store.record(&track);
    save(&state);
    w.set_current_title(track.title.clone().into());
    w.set_current_artist(track.artist.clone().into());
    w.set_current_album(track.album.clone().into());
    w.set_current_liked(state.store.likes.contains(&track.path));
    w.set_current_art(slint::Image::default());
    w.set_lyric_lines(model(vec![]));
    w.set_lyrics_document("".into());
    w.set_lyrics_offset(*state.store.offsets.get(&track.path).unwrap_or(&0) as i32);
    drop(state);
    load_artwork(w, a, &track);
    let weak = w.as_weak();
    if let Ok(promise) = play {
        spawn_local(async move {
            if JsFuture::from(promise).await.is_err() {
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
    let weak = w.as_weak();
    let shared = a.clone();
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
        let result = match reqwest::get(api(&url)).await {
            Ok(r) if r.status().is_success() => {
                r.json::<Vec<Track>>().await.map_err(|e| e.to_string())
            }
            Ok(r) => Err(format!("HTTP {}", r.status())),
            Err(e) => Err(e.to_string()),
        };
        if let Some(w) = weak.upgrade() {
            let mut a = shared.borrow_mut();
            if a.generation != generation {
                return;
            }
            match result{Ok(rows)=>{a.visible=rows.clone();for t in rows{a.store.remember(t);}refresh(&w,&mut a);w.set_status("".into());},Err(_)=>w.set_status("YouTube Music недоступний. Запустіть PlayerVF --web-server для мережевих функцій.".into())}
        }
    });
}
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let w = MainWindow::new().map_err(|e| JsValue::from_str(&e.to_string()))?;
    crate::renderer::install(&w);
    let _lyrics_widget = crate::lyrics_widget::install(&w);
    if let Some(browser) = web_sys::window() {
        let width = browser.inner_width()?.as_f64().unwrap_or(1024.);
        let height = browser.inner_height()?.as_f64().unwrap_or(768.);
        w.window()
            .set_size(slint::LogicalSize::new(width as f32, height as f32));
        w.set_mobile(width < 800.);
    }

    let store = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("playervf.rust.v1").ok().flatten())
        .and_then(|s| serde_json::from_str::<Store>(&s).ok())
        .unwrap_or_default();
    let a = Rc::new(RefCell::new(WebApp {
        store,
        library: vec![],
        visible: vec![],
        queue: Queue::default(),
        current: None,
        playlist: String::new(),
        audio: web_sys::HtmlAudioElement::new()?,
        alternatives: vec![],
        artwork: HashMap::new(),
        artwork_pending: HashSet::new(),
        generation: 0,
        ended: false,
    }));
    {
        let a = a.borrow();
        w.set_volume(a.store.settings.volume);
        w.set_rate(a.store.settings.rate);
        w.set_lyrics_provider(a.store.settings.lyrics_provider.clone().into());
        w.set_lyrics_stage(a.store.settings.lyrics_stage);
        w.set_lyrics_reduced_motion(a.store.settings.lyrics_reduced_motion);
        w.set_lyrics_passive_scroll(a.store.settings.lyrics_passive_scroll);
        let theme = crate::themes::NativeTheme::default();
        theme.apply(
            &w,
            a.store.settings.lyrics_reduced_motion,
            a.store.settings.lyrics_passive_scroll,
        );
    }
    macro_rules! bind{($method:ident,|$ui:ident,$state:ident $(,$arg:ident)*|$body:block)=>{{let weak=w.as_weak();let shared=a.clone();w.$method(move|$($arg),*|{if let Some($ui)=weak.upgrade(){let $state=&shared;$body}});}};}
    bind!(on_navigate, |ui, state, view| {
        ui.set_active_view(view.clone());
        if view == "explore" {
            search(&ui, state, None, false);
        } else {
            refresh(&ui, &mut state.borrow_mut());
        }
    });
    bind!(on_search, |ui, state, q| {
        ui.set_active_view("search".into());
        search(&ui, state, Some(q.to_string()), false);
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
        let t = state.borrow_mut().queue.advance(false, false);
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
    bind!(on_rate_changed, |_ui, state, value| {
        let mut a = state.borrow_mut();
        a.store.settings.rate = value.clamp(0.5, 2.);
        a.audio.set_playback_rate(a.store.settings.rate as f64);
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
            a.store.toggle_like(&t.path);
            save(&a);
            refresh(&ui, &mut a);
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
                        for t in tracks {
                            a.store.remember(t.clone());
                            a.library.retain(|old| old.path != t.path);
                            a.library.push(t);
                        }
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
    bind!(on_create_playlist, |ui, state, name| {
        let mut a = state.borrow_mut();
        a.store.create_playlist(&name);
        save(&a);
        refresh(&ui, &mut a);
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
    bind!(on_add_to_playlist, |ui, state, id| {
        let mut a = state.borrow_mut();
        if let Some(t) = a.current.clone() {
            a.store.playlist_add(&id, &t.path);
            save(&a);
            refresh(&ui, &mut a);
        }
    });
    bind!(on_remove_from_playlist, |ui, state, index| {
        let mut a = state.borrow_mut();
        let id = a.playlist.clone();
        if let Some(t) = a.visible.get(index.max(0) as usize).cloned() {
            if let Some(p) = a.store.playlists.iter_mut().find(|p| p.id == id) {
                p.tracks.retain(|t2| t2 != &t.path);
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
    bind!(on_request_lyrics, |ui, state| {
        request_lyrics(&ui, state);
    });
    bind!(on_refresh_lyrics, |ui, state| {
        let current = { state.borrow().current.clone() };
        if let Some(t) = current {
            state.borrow_mut().store.lyrics_cache.remove(&t.path);
        }
        request_lyrics_with_refresh(&ui, state, true);
    });
    bind!(on_choose_provider, |ui, state, index| {
        show_lyrics(&ui, &state.borrow(), index.max(0) as usize);
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
        a.alternatives = vec![LyricsAlternative {
            provider: "Власні слова".into(),
            sync_type,
            format: format.into(),
            lyrics,
        }];
        show_lyrics(&ui, &a, 0);
    });
    bind!(on_copy_lyrics, |_ui, state| {
        let a = state.borrow();
        if let Some(alt) = a.alternatives.first() {
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

    bind!(on_provider_changed, |ui, state, value| {
        state.borrow_mut().store.settings.lyrics_provider = value.to_string();
        save(&state.borrow());
        request_lyrics(&ui, state);
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
    bind!(on_change_art_mode, |ui, _state| {
        ui.set_status("Відеокліпи ще не перенесено".into());
    });
    bind!(on_download, |ui, state, index| {
        if let Some(id) = state
            .borrow()
            .visible
            .get(index.max(0) as usize)
            .and_then(|t| t.video_id.as_deref())
        {
            if let Some(w) = web_sys::window() {
                let _ = w.open_with_url_and_target(&format!("/api/audio/{id}"), "_blank");
            }
        } else {
            ui.set_status("Файл уже доступний локально".into());
        }
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
    refresh(&w, &mut a.borrow_mut());
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
                        w.set_mobile(width < 800.);
                    }
                }

                let mut a = shared.borrow_mut();
                if let Some(error) = a.audio.error() {
                    w.set_status(
                        format!("Помилка аудіо ({}): {}", error.code(), error.message()).into(),
                    );
                }
                let pos = a.audio.current_time();
                let duration = a.audio.duration();
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
                if a.audio.ended() && !a.ended {
                    a.ended = true;
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
