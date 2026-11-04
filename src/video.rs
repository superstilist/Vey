//! A visual video surface follows the song clock; it never owns audio or lyrics.
use crate::{services::Track, MainWindow};
use serde::{Deserialize, Serialize};
use slint::ComponentHandle;
use std::time::Duration;
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoSource {
    pub url: String,
    pub mime: String,
}

#[cfg(not(target_arch = "wasm32"))]
fn direct_video_id(track: &Track) -> Option<String> {
    track
        .clip_video_id
        .clone()
        .or_else(|| track.video_id.clone())
        .or_else(|| track.path.strip_prefix("yt://").map(str::to_owned))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn resolve(track: &Track) -> Result<Option<VideoSource>, String> {
    let video = if let Some(id) = track.clip_video_id.clone() {
        // A paired clip is authoritative; never replace a failed clip with a different match.
        crate::native::ytm_video_url(id).await?
    } else {
        let direct = direct_video_id(track);
        let mut video = if let Some(id) = direct {
            crate::native::ytm_video_url(id).await?
        } else {
            None
        };
        if video.is_none() {
            if let Some(clip) =
                crate::native::ytm_find_clip(track.title.clone(), track.artist.clone()).await?
            {
                video = crate::native::ytm_video_url(clip.video_id).await?;
            }
        }
        video
    };
    Ok(video.map(|v| VideoSource {
        url: v.url,
        mime: v.mime_type,
    }))
}
fn track(w: &MainWindow) -> Track {
    Track {
        path: w.get_current_track_key().into(),
        title: w.get_current_title().into(),
        artist: w.get_current_artist().into(),
        video_id: (!w.get_current_video_id().is_empty()).then(|| w.get_current_video_id().into()),
        clip_video_id: (!w.get_current_clip_video_id().is_empty())
            .then(|| w.get_current_clip_video_id().into()),
        ..Default::default()
    }
}
fn key(w: &MainWindow) -> String {
    if w.get_art_mode() == "clip" {
        w.get_current_track_key().into()
    } else {
        String::new()
    }
}
fn state(w: &MainWindow, source: &str) -> serde_json::Value {
    if source.is_empty() {
        return serde_json::json!({"url":"","visible":false,"position":0,"playing":false,"rate":1});
    }
    let size = w.window().size().to_logical(w.window().scale_factor());
    serde_json::json!({"url":source,"position":w.get_position(),"playing":w.get_player_state()=="playing",
        "rate":w.get_rate(),"visible":!key(w).is_empty() && w.get_player_on_stage() && !w.get_page_moving() && w.get_player_reveal()>0.999 && w.get_dialog().is_empty() && !w.get_tools_visible() && !w.window().is_minimized(),
        "bounds":{"x":w.get_video_x(),"y":w.get_video_y(),"width":w.get_video_width(),"height":w.get_video_height()},
        "windowWidth":size.width,"windowHeight":size.height})
}

#[cfg(not(target_arch = "wasm32"))]
pub fn install(w: &MainWindow, runtime: std::sync::Arc<tokio::runtime::Runtime>) -> slint::Timer {
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut requested = String::new();
    let mut last_state = String::new();
    let mut source = String::new();
    let mut pending: Option<tokio::task::JoinHandle<()>> = None;
    let mut surface = platform::Surface::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(32),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
            let next = key(&w);
            if next != requested {
                if let Some(task) = pending.take() {
                    task.abort();
                }
                requested = next.clone();
                source.clear();
                w.set_video_frame(Default::default());
                if !next.is_empty() {
                    let tx = tx.clone();
                    let track = track(&w);
                    pending = Some(runtime.spawn(async move {
                        let result = resolve(&track).await;
                        let _ = tx.send((next, result));
                    }));
                }
            }
            for (key, result) in rx.try_iter() {
                if key == requested {
                    if let Ok(Some(video)) = result {
                        source = video.url;
                    }
                }
            }
            let next = state(&w, &source);
            let encoded = next.to_string();
            if encoded != last_state || next["visible"].as_bool().unwrap_or(false) {
                surface.update(&w, next);
                last_state = encoded;
            }
        },
    );
    timer
}

/// Embed an already-resolved visual source using the song clock and art-slot bounds.
/// Dropping the timer releases the native surface and decoder.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_source(w: &MainWindow, source: VideoSource) -> slint::Timer {
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    let mut surface = platform::Surface::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(32),
        move || {
            if let Some(w) = weak.upgrade() {
                let source = if key(&w).is_empty() { "" } else { &source.url };
                surface.update(&w, state(&w, source));
            }
        },
    );
    timer
}

#[cfg(target_arch = "wasm32")]
pub fn install(w: &MainWindow) -> slint::Timer {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(module = "/web/video-surface.js")]
    extern "C" {
        #[wasm_bindgen(js_name=updateVideoSurface)]
        fn update(json: &str) -> bool;
    }
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    let source = Rc::new(RefCell::new(String::new()));
    let generation = Rc::new(std::cell::Cell::new(0u64));
    let mut requested = String::new();
    let mut last_state = String::new();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(32),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
            let next = key(&w);
            if next != requested {
                requested = next.clone();
                source.borrow_mut().clear();
                generation.set(generation.get() + 1);
                if !next.is_empty() {
                    let shared = source.clone();
                    let version = generation.clone();
                    let epoch = version.get();
                    let track = track(&w);
                    wasm_bindgen_futures::spawn_local(async move {
                        let origin = web_sys::window()
                            .and_then(|w| w.location().origin().ok())
                            .unwrap_or_default();
                        if let Ok(response) = reqwest::Client::new()
                            .post(format!("{origin}/api/video"))
                            .json(&track)
                            .send()
                            .await
                        {
                            if response.status().is_success() {
                                if let Ok(Some(video)) =
                                    response.json::<Option<VideoSource>>().await
                                {
                                    if version.get() == epoch {
                                        *shared.borrow_mut() = video.url;
                                    }
                                }
                            }
                        }
                    });
                }
            }
            let next = state(&w, &source.borrow()).to_string();
            if next != last_state {
                update(&next);
                last_state = next;
            }
        },
    );
    timer
}

#[cfg(target_os = "android")]
mod platform {
    use super::*;
    #[derive(Default)]
    pub struct Surface;
    impl Surface {
        pub fn update(&mut self, w: &MainWindow, mut state: serde_json::Value) {
            let scale = w.window().scale_factor() as f64;
            for k in ["x", "y", "width", "height"] {
                state["bounds"][k] = (state["bounds"][k].as_f64().unwrap_or(0.) * scale).into();
            }
            let _ = crate::android::bridge_call("video", &state.to_string());
        }
    }
}

#[cfg(all(
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
#[path = "video_linux.rs"]
mod platform;

#[cfg(test)]
mod tests {
    use super::direct_video_id;
    use crate::services::Track;

    #[test]
    fn paired_clip_id_is_preferred_without_replacing_audio_identity() {
        let track = Track {
            path: "yt://audio-id".into(),
            video_id: Some("audio-id".into()),
            clip_video_id: Some("paired-video-id".into()),
            ..Default::default()
        };

        assert_eq!(direct_video_id(&track).as_deref(), Some("paired-video-id"));
        assert_eq!(track.video_id.as_deref(), Some("audio-id"));
    }

    #[test]
    fn video_id_then_youtube_path_are_fallbacks_for_missing_paired_clip() {
        let by_video_id = Track {
            video_id: Some("audio-video-id".into()),
            ..Default::default()
        };
        assert_eq!(
            direct_video_id(&by_video_id).as_deref(),
            Some("audio-video-id")
        );

        let by_path = Track {
            path: "yt://path-video-id".into(),
            ..Default::default()
        };
        assert_eq!(direct_video_id(&by_path).as_deref(), Some("path-video-id"));
    }
}

// The FFmpeg adapter is portable Rust; type-check its UI contract in Windows tests too.
#[cfg(all(test, target_os = "windows"))]
#[allow(dead_code)]
#[path = "video_linux.rs"]
mod linux_compile_contract;
