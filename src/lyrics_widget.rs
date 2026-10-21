//! Original Braccato renderer hosted in WebView2 (desktop) or an iframe (WASM).
//! Rust owns playback, authentication, provider requests and canonical documents.
use crate::*;
use serde::Deserialize;
use slint::{ComponentHandle, Model, ModelRc};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Message {
    Ready,
    Navigation { view: String, section: String },
    Ambient { pixels: String },
    Seek { seconds: f64 },
    Error { message: String },
    Edit { text: String },
    Diagnostics { data: serde_json::Value },
}

#[derive(Default)]
struct Bridge {
    ambient_art: Option<slint::Image>,
    ambient_at: Option<web_time::Instant>,
    ambient_time: f64,
    ambient_pending_at: Option<web_time::Instant>,
    ambient_transition_until: Option<web_time::Instant>,
    ambient_reduced: Option<bool>,
    ambient_sequence: u64,
    ambient_source: String,
    ambient_options: String,
    model: ModelRc<LyricRow>,
    source: slint::SharedString,
    document: String,
    last_document: String,
    last_state: String,
    last_editor: slint::SharedString,
    last_mode: bool,
}

fn same_artwork(a: &slint::Image, b: &slint::Image) -> bool {
    // Slint's empty Image variant is not equal to another empty Image.
    // Treat missing covers as one stable source instead of uploading at 30 fps.
    let empty = |image: &slint::Image| image.size().width == 0 || image.size().height == 0;
    (empty(a) && empty(b)) || a == b
}

// Invert the shared cubic-bezier(.45,0,.55,1). The camera's actual
// on-screen displacement is authoritative, including interrupted movement.
fn camera_time_fraction(displacement: f32) -> f32 {
    let fraction = displacement.clamp(0., 1.);
    if fraction == 0. || fraction == 1. {
        return fraction;
    }
    let (mut low, mut high) = (0., 1.);
    for _ in 0..18 {
        let t = (low + high) * 0.5;
        let y = 3. * (1. - t) * t * t + t * t * t;
        if y < fraction {
            low = t;
        } else {
            high = t;
        }
    }
    let t = (low + high) * 0.5;
    3. * (1. - t) * (1. - t) * t * 0.45 + 3. * (1. - t) * t * t * 0.55 + t * t * t
}

fn camera_elapsed(w: &MainWindow) -> f32 {
    let dx = w.get_camera_end_x() - w.get_camera_start_x();
    let dy = w.get_camera_end_y() - w.get_camera_start_y();
    let fraction = if dx.abs() >= dy.abs() && dx.abs() > 0.0001 {
        (1. - w.get_player_motion_x() / w.get_stage_width().max(1.) - w.get_camera_start_x()) / dx
    } else if dy.abs() > 0.0001 {
        (-1. - w.get_player_motion_y() / w.get_stage_height().max(1.) - w.get_camera_start_y()) / dy
    } else {
        1.
    };
    camera_time_fraction(fraction) * w.get_camera_duration() as f32
}

impl Bridge {
    fn state(&mut self, w: &MainWindow) -> serde_json::Value {
        let model = w.get_lyric_lines();
        let source = w.get_lyrics_document();
        if self.model != model || self.source != source {
            self.model = model.clone();
            self.source = source.clone();
            self.document = if !source.is_empty() {
                source.to_string()
            } else {
                // Fixture/fallback callers also get the original renderer.
                let lyrics = (0..model.row_count())
                    .filter_map(|i| model.row_data(i))
                    .map(|row| {
                        let parts = (0..row.words.row_count())
                            .filter_map(|i| row.words.row_data(i))
                            .map(|p| braccato::types::LyricPart {
                                words: p.text.to_string(),
                                start_time_ms: p.start_ms as i64,
                                duration_ms: (p.end_ms as i64 - p.start_ms as i64).max(0),
                                is_background: p.background,
                                explicit: p.explicit,
                            })
                            .collect::<Vec<_>>();
                        let romanization = (0..row.roman_words.row_count())
                            .filter_map(|i| row.roman_words.row_data(i))
                            .map(|p| braccato::types::LyricPart {
                                words: p.text.to_string(),
                                start_time_ms: p.start_ms as i64,
                                duration_ms: (p.end_ms as i64 - p.start_ms as i64).max(0),
                                ..Default::default()
                            })
                            .collect::<Vec<_>>();
                        braccato::types::Lyric {
                            words: row.text.to_string(),
                            start_time_ms: row.start_ms as i64,
                            duration_ms: (row.end_ms as i64 - row.start_ms as i64).max(0),
                            parts: (!parts.is_empty()).then_some(parts),
                            is_instrumental: row.instrumental,
                            agent: (!row.agent.is_empty()).then(|| row.agent.to_string()),
                            translations: (!row.translation.is_empty()).then(|| {
                                std::collections::HashMap::from([(
                                    "translation".into(),
                                    row.translation.to_string(),
                                )])
                            }),
                            romanization: (!row.romanization.is_empty())
                                .then(|| row.romanization.to_string()),
                            timed_romanization: (!romanization.is_empty()).then_some(romanization),
                            ..Default::default()
                        }
                    })
                    .collect::<Vec<_>>();
                serde_json::json!({"lyrics":lyrics,"options":{"songwriters":w.get_lyrics_credits().split(" · ").filter(|s|!s.is_empty()).collect::<Vec<_>>()}}).to_string()
            };
        }
        let size = w.window().size().to_logical(w.window().scale_factor());
        let editing = w.get_dialog() == "paste";
        let visible = editing
            || (w.get_player_on_stage()
                && w.get_player_tab() == "lyrics"
                && w.get_dialog().is_empty()
                && model.row_count() > 0
                && !w.window().is_minimized());
        let mut state = serde_json::json!({
            "language":w.global::<crate::I18n>().get_language().to_string(),
            "visible": visible, "position": if visible && !editing { w.get_position() } else { 0. },
            "mode":if editing {"editor"} else {"lyrics"},
            "playing": visible && !editing && w.get_player_state() == "playing", "rate": w.get_rate(),
            "offset": w.get_lyrics_offset(), "stage": w.get_lyrics_stage(),
            "richsyncOffset": w.get_lyrics_richsync_offset_ms(),
            "lineOffset": w.get_lyrics_line_offset_ms(),
            "timed": w.get_lyrics_timed(), "passive": w.get_lyrics_passive_scroll(),
            "reduced": w.get_lyrics_reduced_motion(), "mobile": w.get_mobile(),
            "size": w.get_lyric_size(),
            "scrollRatio":w.get_lyrics_scroll_ratio(), "lookahead":w.get_lyrics_lookahead_ms(),
            "scrollDuration":w.get_lyrics_scroll_duration_ms(), "passiveSeconds":w.get_lyrics_passive_seconds(),
            "disableRichsync":w.get_lyrics_disable_richsync(),
            "bounds":if editing {
                serde_json::json!({"x":w.get_lyrics_editor_x(),"y":w.get_lyrics_editor_y(),"width":w.get_lyrics_editor_width(),"height":w.get_lyrics_editor_height()})
            } else { serde_json::json!({"x":w.get_lyrics_widget_x(),"y":w.get_lyrics_widget_y(),"width":w.get_lyrics_widget_width(),"height":w.get_lyrics_widget_height()}) },
            "windowWidth":size.width,"windowHeight":size.height,
            "presentationOpacity": if editing {1.} else {w.get_player_reveal()},
            "grouped":true,
            "stageBounds":if editing {
                serde_json::json!({"x":0,"y":0,"width":size.width,"height":size.height,"radius":0})
            } else {
                serde_json::json!({"x":w.get_stage_x(),"y":w.get_stage_y(),"width":w.get_stage_width(),"height":w.get_stage_height(),"radius":if w.get_mobile(){20}else{28}})
            },
            "presentationX": if editing {0.} else {w.get_player_motion_x()},
            "presentationY": if editing {0.} else {w.get_player_motion_y()},
            "overlay": if w.get_tools_visible() {
                serde_json::json!({"x":w.get_tools_x(),"y":w.get_tools_y(),"width":w.get_tools_width(),"height":w.get_tools_height()})
            } else {serde_json::Value::Null}
        });
        state["navigation"] = serde_json::json!({"view":w.get_active_view().to_string(),
            "section":if w.get_active_view() == "settings" { w.get_settings_section().to_string() } else { String::new() }});
        let now = web_time::Instant::now();
        if !editing && w.get_camera_travelling() {
            state["camera"] = serde_json::json!({
                "epoch":w.get_camera_epoch(), "elapsed":camera_elapsed(w),
                "duration":w.get_camera_duration(),
                "fromX":w.get_camera_start_x(),"fromY":w.get_camera_start_y(),
                "toX":w.get_camera_end_x(),"toY":w.get_camera_end_y(),
                "width":w.get_stage_width(),"height":w.get_stage_height()
            });
        } else {
            state["camera"] = serde_json::Value::Null;
        }
        #[cfg(target_os = "windows")]
        {
            state["nativeBackdrop"] = serde_json::json!({
                "opacity":w.get_ambient_opacity(),
                "surface": format!("#{:02x}{:02x}{:02x}", w.global::<crate::Tokens>().get_neutral_50().red(), w.global::<crate::Tokens>().get_neutral_50().green(), w.global::<crate::Tokens>().get_neutral_50().blue()),
                "content": {"x":w.get_backdrop_x(),"y":w.get_backdrop_y(),
                    "width":w.get_backdrop_width(),"height":w.get_backdrop_height()},
                "viewport":{"x":w.get_lyrics_widget_x()+w.get_player_motion_x(),
                    "y":w.get_lyrics_widget_y()+w.get_player_motion_y(),
                    "width":w.get_lyrics_widget_width(),"height":w.get_lyrics_widget_height()}
            });
        }
        let art = w.get_current_art();
        let changed = self
            .ambient_art
            .as_ref()
            .is_none_or(|previous| !same_artwork(previous, &art));
        let elapsed = self
            .ambient_at
            .map(|at| now.duration_since(at).as_secs_f64())
            .unwrap_or(0.);
        let animate = w.get_player_state() == "playing" && !w.get_lyrics_reduced_motion();
        let options = serde_json::json!({"warpIntensity":w.get_kawarp_warp(),"blurPasses":w.get_kawarp_blur().round(),"saturation":w.get_kawarp_saturation(),"dithering":w.get_kawarp_dithering(),"dimBright":w.get_kawarp_dim()});
        let options_key = options.to_string();
        let options_changed = options_key != self.ambient_options;
        let motion_changed = self.ambient_reduced != Some(w.get_lyrics_reduced_motion());
        if w.get_lyrics_reduced_motion() {
            self.ambient_transition_until = None;
        }
        let morphing = self
            .ambient_transition_until
            .is_some_and(|until| now < until);
        // Decorative GPU readback/IPC yields bandwidth to foreground slides.
        // Keep the warp clock continuous; restore normal cadence when settled.
        let ambient_interval = if w.get_page_moving() { 0.06 } else { 0.03 };
        if !w.window().is_minimized()
            && w.get_color_mode() != "static"
            && (changed
                || motion_changed
                || options_changed
                || ((animate || morphing) && elapsed >= ambient_interval))
            && self
                .ambient_pending_at
                .is_none_or(|at| now.duration_since(at).as_millis() >= 500)
        {
            self.ambient_pending_at = Some(now);
            self.ambient_reduced = Some(w.get_lyrics_reduced_motion());
            self.ambient_options = options_key;
            self.ambient_time += if animate { elapsed.min(0.2) * 0.24 } else { 0. };
            self.ambient_at = Some(now);
            // Paused morphs share a frozen warp clock. A request sequence keeps
            // JSON deduplication from dropping their subsequent color frames.
            self.ambient_sequence = self.ambient_sequence.wrapping_add(1);
            let mut ambient = serde_json::json!({"time": self.ambient_time, "animate": animate, "reduced":w.get_lyrics_reduced_motion(),"duration":w.global::<crate::Motion>().get_palette(),"frame":self.ambient_sequence,"options":options});
            if changed {
                use base64::Engine;
                let source = art
                    .to_rgba8()
                    .and_then(|buffer| {
                        image::RgbaImage::from_raw(
                            buffer.width(),
                            buffer.height(),
                            buffer.as_bytes().to_vec(),
                        )
                    })
                    .map(|image| {
                        let thumb = image::imageops::resize(
                            &image,
                            64,
                            64,
                            image::imageops::FilterType::Triangle,
                        );
                        base64::engine::general_purpose::STANDARD.encode(thumb.as_raw())
                    })
                    .unwrap_or_default();
                if source != self.ambient_source
                    && self.ambient_art.is_some()
                    && !w.get_lyrics_reduced_motion()
                {
                    // Finish genuine palette changes even after pausing. New
                    // handles for identical artwork must not restart the tail.
                    self.ambient_transition_until =
                        Some(now + std::time::Duration::from_millis(2600));
                }
                self.ambient_source = source.clone();
                ambient["source"] = source.into();
                self.ambient_art = Some(art);
            }
            state["ambient"] = ambient;
        }
        if self.document != self.last_document {
            state["document"] = self.document.clone().into();
            self.last_document = self.document.clone();
        }
        if editing && (self.last_editor != w.get_lyrics_paste() || !self.last_mode) {
            self.last_editor = w.get_lyrics_paste();
            state["editorText"] = self.last_editor.to_string().into();
        }
        self.last_mode = editing;
        state
    }
}

fn handle(w: &MainWindow, message: Message) {
    match message {
        Message::Navigation { view, section }
            if matches!(
                view.as_str(),
                "home"
                    | "explore"
                    | "library"
                    | "liked"
                    | "playlist"
                    | "queue"
                    | "search"
                    | "settings"
                    | "lyrics"
            ) && matches!(
                section.as_str(),
                "" | "audio" | "library" | "lyrics" | "data" | "appearance" | "language"
            ) =>
        {
            if !w.get_dialog().is_empty() {
                w.set_dialog("".into());
            }
            if w.get_active_view().as_str() != view {
                w.invoke_navigate(view.clone().into());
            }
            if view == "settings" {
                w.invoke_open_settings(section.into());
            }
        }
        Message::Ambient { pixels } if pixels.len() == 256 * 144 * 4 / 3 * 4 => {
            use base64::Engine;
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(pixels) {
                if bytes.len() == 256 * 144 * 4 {
                    w.set_ambient_frame(slint::Image::from_rgba8(slint::SharedPixelBuffer::<
                        slint::Rgba8Pixel,
                    >::clone_from_slice(
                        &bytes, 256, 144
                    )));
                }
            }
        }
        Message::Seek { seconds } if seconds.is_finite() && seconds >= 0. => {
            w.invoke_seek_to((seconds * 1000.).clamp(0., i32::MAX as f64) as i32);
        }
        Message::Error { message } => w.set_status(
            format!(
                "Braccato: {}",
                message.chars().take(1000).collect::<String>()
            )
            .into(),
        ),
        Message::Edit { text } if w.get_dialog() == "paste" && text.len() <= 2_000_000 => {
            w.set_lyrics_paste(text.into())
        }
        Message::Diagnostics { data } => {
            let _ = data;
        }
        _ => {}
    }
}

// Cut a rounded hole in the child HWND so Slint's settings overlay receives
// both paint and input, without resizing or rebuilding the lyric widget.
#[cfg(target_os = "windows")]
fn clip_native_view(
    view: &wry::WebView,
    bounds: (i32, i32, i32, i32),
    overlay: &serde_json::Value,
    stage: &serde_json::Value,
    scale: f32,
) {
    use std::ffi::c_void;
    use wry::WebViewExtWindows;
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateRoundRectRgn(l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> *mut c_void;
        fn CombineRgn(dest: *mut c_void, a: *mut c_void, b: *mut c_void, mode: i32) -> i32;
        fn DeleteObject(object: *mut c_void) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowRgn(window: *mut c_void, region: *mut c_void, redraw: i32) -> i32;
    }
    unsafe {
        let mut hwnd = Default::default();
        if view.controller().ParentWindow(&mut hwnd).is_err() {
            return;
        }
        let radius = (40. * scale).round() as i32;
        let region = CreateRoundRectRgn(0, 0, bounds.2 + 1, bounds.3 + 1, radius, radius);
        if region.is_null() {
            return;
        }
        // Move the entire child as one object, clipping it to the player stage.
        if !stage.is_null() {
            let px = |key: &str| (stage[key].as_f64().unwrap_or(0.) * scale as f64).round() as i32;
            let left = px("x") - bounds.0;
            let top = px("y") - bounds.1;
            let diameter = px("radius") * 2;
            let moving = CreateRoundRectRgn(
                left,
                top,
                left + px("width"),
                top + px("height"),
                diameter,
                diameter,
            );
            if !moving.is_null() {
                CombineRgn(region, region, moving, 1); // RGN_AND
                DeleteObject(moving);
            }
        }
        if !overlay.is_null() {
            let px =
                |key: &str| (overlay[key].as_f64().unwrap_or(0.) * scale as f64).round() as i32;
            let x = px("x") - bounds.0;
            let y = px("y") - bounds.1;
            let hole = CreateRoundRectRgn(
                x,
                y,
                x + px("width") + 1,
                y + px("height") + 1,
                radius,
                radius,
            );
            if !hole.is_null() {
                CombineRgn(region, region, hole, 4); // RGN_DIFF
                DeleteObject(hole);
            }
        }
        // Windows owns the region only after a successful SetWindowRgn.
        if SetWindowRgn(hwnd.0, region, 1) == 0 {
            DeleteObject(region);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn install(w: &MainWindow) -> Option<slint::Timer> {
    use std::time::{Duration, Instant};
    w.set_lyrics_web_required(true);
    struct Host {
        view: wry::WebView,
        _context: wry::WebContext,
        events: std::sync::mpsc::Receiver<Message>,
        ready: bool,
        started: Instant,
        visible: bool,
        bounds: (i32, i32, i32, i32),
        bridge: Bridge,
        clip_key: String,
        diagnostic_at: Instant,
        diagnostics: Vec<serde_json::Value>,
    }
    let weak = w.as_weak();
    let mut host: Option<Host> = None;
    let mut attempted = false;
    let mut generation = w.get_lyrics_renderer_generation();
    let mut retry_at = Instant::now();
    let smoke = std::env::args().any(|s| s == "--widget-smoke");
    let mut last_tick: Option<Instant> = None;
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
        let Some(w) = weak.upgrade() else { host.take(); return; };
        // Native child placement follows the foreground camera at display
        // cadence. Ordinary lyric/audio updates keep their lower-cost cadence.
        let moving_player = w.get_page_moving() && w.get_displayed_view() == "lyrics";
        if !moving_player && last_tick.is_some_and(|at| at.elapsed() < Duration::from_millis(32)) { return; }
        last_tick = Some(Instant::now());
        if generation != w.get_lyrics_renderer_generation() {
            generation = w.get_lyrics_renderer_generation();
            host.take(); attempted=false;
            w.set_lyrics_web_ready(false);
            w.set_lyrics_renderer_error("".into());
        }
        if host.as_ref().is_some_and(|h| !h.ready && h.started.elapsed() > Duration::from_secs(10)) {
            host.take();
            retry_at=Instant::now()+Duration::from_secs(3);
            w.set_lyrics_web_ready(false);
            w.set_lyrics_renderer_error("Braccato не відповідає. Повторюємо запуск…".into());
        }
        if host.is_none() && (!attempted || Instant::now() >= retry_at) && w.window().is_visible() {
            // Do not turn Slint's temporary, pre-HWND handle into a failed
            // WebView attempt and a five-second retry delay.
            let Some(parent) = crate::windows_webview::parent(w.window()) else { return; };
            attempted = true;
            retry_at=Instant::now()+Duration::from_secs(5);
            let (tx,rx) = std::sync::mpsc::channel();
            let mut context = wry::WebContext::new(Some(crate::storage::data_dir().join("braccato-webview")));
            let result = wry::WebViewBuilder::new_with_web_context(&mut context)
                // An HWND WebView cannot reliably alpha-compose over Slint's
                // swap chain. Paint the matching backdrop inside the child.
                .with_visible(false).with_focused(false).with_transparent(false)
                .with_background_color((8,8,8,255))
                .with_custom_protocol("playerlyrics".into(), |_,request| {
                    use std::borrow::Cow;
                    let (mime,body):(&str,&'static [u8]) = match request.uri().path() {
                        "/" | "/index.html" => ("text/html; charset=utf-8",include_bytes!("../web/braccato/index.html")),
                        "/widget.js" => ("application/javascript; charset=utf-8",include_bytes!("../web/braccato/widget.js")),
                        "/widget.css" => ("text/css; charset=utf-8",include_bytes!("../web/braccato/widget.css")),
                        "/LICENSES.txt" => ("text/plain; charset=utf-8",include_bytes!("../web/braccato/LICENSES.txt")),
                        _ => return wry::http::Response::builder().status(404).body(Cow::Borrowed(&[][..])).unwrap(),
                    };
                    wry::http::Response::builder().header("Content-Type",mime)
                        .header("Cache-Control","no-cache").body(Cow::Borrowed(body)).unwrap()
                })
                .with_url("playerlyrics://localhost/index.html")
                .with_navigation_handler(|url|url.starts_with("http://playerlyrics.localhost/") || url.starts_with("playerlyrics://localhost/"))
                .with_new_window_req_handler(|_,_|wry::NewWindowResponse::Deny)
                .with_ipc_handler(move |request| {
                    if request.uri().to_string().starts_with("http://playerlyrics.localhost/") && request.body().len() < 1_000_000 {
                        if let Ok(message) = serde_json::from_str::<Message>(request.body()) { let _ = tx.send(message); }
                    }
                }).build_as_child(&parent);
            match result {
                Ok(view) => {
                    eprintln!("[braccato] WebView2 initialized");
                    host=Some(Host {view,_context:context,events:rx,ready:false,started:Instant::now(),visible:false,bounds:(0,0,0,0),bridge:Bridge::default(),clip_key:String::new(),diagnostic_at:Instant::now(),diagnostics:vec![]});
                }
                Err(e) => {
                    // Startup can race native handle creation. Keep retry feedback
                    // on the lyric surface instead of a persistent Home banner.
                    w.set_lyrics_renderer_error("Не вдалося запустити слова. Повторюємо запуск…".into());
                    eprintln!("Braccato WebView2: {e}");
                },
            }
        }
        let Some(h) = host.as_mut() else {return;};
        for event in h.events.try_iter() {
            match event {
                message @ Message::Ambient { .. } => {
                    h.bridge.ambient_pending_at = None;
                    handle(&w,message);
                },
                Message::Ready => {h.ready=true; w.set_lyrics_web_ready(true); w.set_lyrics_renderer_error("".into());},
                Message::Diagnostics {mut data} if smoke => {
                    data["ambientWidth"] = w.get_ambient_frame().size().width.into();
                    h.diagnostics.push(data);
                    let dir=crate::storage::data_dir();
                    let _=std::fs::create_dir_all(&dir);
                    let _=std::fs::write(dir.join("braccato-widget-smoke.json"),serde_json::to_vec_pretty(&h.diagnostics).unwrap_or_default());
                },
                other => handle(&w,other),
            }
        }
        if !h.ready {return;}
        let state = h.bridge.state(&w);
        let scale = w.window().scale_factor();
        let shift = (
            state["presentationX"].as_f64().unwrap_or(0.),
            state["presentationY"].as_f64().unwrap_or(0.),
        );
        let bounds=(
            ((state["bounds"]["x"].as_f64().unwrap_or(0.)+shift.0)*scale as f64).round() as i32,
            ((state["bounds"]["y"].as_f64().unwrap_or(0.)+shift.1)*scale as f64).round() as i32,
            (state["bounds"]["width"].as_f64().unwrap_or(0.)*scale as f64).round().max(1.) as i32,
            (state["bounds"]["height"].as_f64().unwrap_or(0.)*scale as f64).round().max(1.) as i32);
        let visible=state["visible"] == true && bounds.2 > 8 && bounds.3 > 8;
        if visible && bounds != h.bounds {
            let _ = h.view.set_bounds(wry::Rect {position:wry::dpi::PhysicalPosition::new(bounds.0,bounds.1).into(),size:wry::dpi::PhysicalSize::new(bounds.2 as u32,bounds.3 as u32).into()});
            h.bounds=bounds;
        }
        let clip_key = format!("{:?}:{}:{}", bounds, state["stageBounds"], state["overlay"]);
        if visible && h.clip_key != clip_key {
            clip_native_view(&h.view, bounds, &state["overlay"], &state["stageBounds"], scale);
            h.clip_key = clip_key;
        }
        if visible != h.visible {let _=h.view.set_visible(visible);h.visible=visible;}
        let json = state.to_string();
        if json != h.bridge.last_state {
            let _=h.view.evaluate_script(&format!("window.PlayerVFBraccato.update({json})"));
            h.bridge.last_state=json;
        }
        if smoke && state["visible"] == true && h.diagnostic_at.elapsed() > Duration::from_millis(400) {
            let bounds=state["bounds"].to_string();
            let _=h.view.evaluate_script(&format!("{{const data=window.PlayerVFBraccato.diagnostics();data.hostBounds={bounds};window.ipc.postMessage(JSON.stringify({{type:'diagnostics',data}}));}}"));
            h.diagnostic_at=Instant::now();
        }
    });
    Some(timer)
}

#[cfg(target_arch = "wasm32")]
pub fn install(w: &MainWindow) -> Option<slint::Timer> {
    use std::{cell::RefCell, rc::Rc, time::Duration};
    use wasm_bindgen::prelude::*;
    w.set_lyrics_web_required(true);
    #[wasm_bindgen(module = "/web/bridge.js")]
    extern "C" {
        #[wasm_bindgen(js_name=updateBraccato)]
        fn update_braccato(state: &str) -> bool;
        #[wasm_bindgen(js_name=drainBraccatoMessages)]
        fn drain_braccato_messages() -> String;
        #[wasm_bindgen(js_name=resetBraccato)]
        fn reset_braccato();
    }
    let weak = w.as_weak();
    let owner: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));
    let keep = owner.clone();
    let mut bridge = Bridge::default();
    let mut generation = w.get_lyrics_renderer_generation();
    let mut started = web_time::Instant::now();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(32),
        move || {
            let Some(w) = weak.upgrade() else {
                keep.borrow_mut().take();
                return;
            };
            if generation != w.get_lyrics_renderer_generation() {
                generation = w.get_lyrics_renderer_generation();
                reset_braccato();
                bridge = Bridge::default();
                started = web_time::Instant::now();
                w.set_lyrics_web_ready(false);
                w.set_lyrics_renderer_error("".into());
            }
            if let Ok(messages) = serde_json::from_str::<Vec<Message>>(&drain_braccato_messages()) {
                for message in messages {
                    if matches!(&message, Message::Ambient { .. }) {
                        bridge.ambient_pending_at = None;
                    }
                    handle(&w, message);
                }
            }
            let json = bridge.state(&w).to_string();
            if json != bridge.last_state || !w.get_lyrics_web_ready() {
                w.set_lyrics_web_ready(update_braccato(&json));
                bridge.last_state = json;
            }
            if !w.get_lyrics_web_ready() && started.elapsed() > Duration::from_secs(10) {
                w.set_lyrics_renderer_error(
                    "Не вдалося завантажити Braccato. Спробуйте ще раз.".into(),
                );
            } else if w.get_lyrics_web_ready() && !w.get_lyrics_renderer_error().is_empty() {
                w.set_lyrics_renderer_error("".into());
            }
        },
    );
    *owner.borrow_mut() = Some(timer);
    None
}

#[cfg(all(not(target_os = "windows"), not(target_arch = "wasm32")))]
pub fn install(_: &MainWindow) -> Option<slint::Timer> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_timing_matches_the_displayed_bezier_path() {
        assert!(camera_time_fraction(-0.5).abs() < 0.00001);
        assert!((camera_time_fraction(1.5) - 1.).abs() < 0.00001);
        assert!((camera_time_fraction(0.15625) - 0.2828125).abs() < 0.00001);
        assert!((camera_time_fraction(0.5) - 0.5).abs() < 0.00001);
        assert!((camera_time_fraction(0.84375) - 0.7171875).abs() < 0.00001);
    }
    #[test]
    fn missing_covers_are_a_stable_ambient_source() {
        let empty = slint::Image::default();
        assert!(same_artwork(&empty, &slint::Image::default()));
        let cover = slint::Image::from_rgba8(slint::SharedPixelBuffer::new(1, 1));
        assert!(!same_artwork(&empty, &cover));
        assert!(same_artwork(&cover, &cover.clone()));
    }
    #[test]
    fn validates_widget_events_without_accepting_script_commands() {
        assert!(matches!(
            serde_json::from_str::<Message>(r#"{"type":"seek","seconds":4.5}"#).unwrap(),
            Message::Seek { seconds: 4.5 }
        ));
        assert!(serde_json::from_str::<Message>(r#"{"type":"execute","script":"x"}"#).is_err());
        assert!(serde_json::from_str::<Message>(r#"{"type":"seek","seconds":"bad"}"#).is_err());
        assert!(serde_json::from_str::<Message>(r#"{"type":"ambient","pixels":null}"#).is_err());
    }
}
