//! Publish native playback state to Windows SMTC, Linux MPRIS and macOS Now Playing.
use crate::{MainWindow, services::Track};
use slint::ComponentHandle;
#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ffi::c_void,
    sync::mpsc::{self, Receiver},
    time::Duration,
};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};

pub struct MediaSession {
    controls: MediaControls,
    events: Receiver<MediaControlEvent>,
}

impl MediaSession {
    pub fn new(window: &MainWindow) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        let hwnd = {
            let parent = window.window().window_handle();
            let handle = parent
                .window_handle()
                .map_err(|error| error.to_string())?;
            match handle.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as *mut c_void),
                _ => return Err("Vey вікно не має Win32 HWND".into()),
            }
        };
        #[cfg(not(target_os = "windows"))]
        let hwnd: Option<*mut c_void> = None;

        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: "org.vey.player",
            display_name: "Vey",
            hwnd,
        })
        .map_err(|error| error.to_string())?;
        let (tx, events) = mpsc::channel();
        controls
            .attach(move |event| {
                let _ = tx.send(event);
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { controls, events })
    }

    pub fn drain_events(&self) -> impl Iterator<Item = MediaControlEvent> + '_ {
        self.events.try_iter()
    }

    pub fn set_track(&mut self, track: &Track) {
        let _ = self.controls.set_metadata(MediaMetadata {
            title: Some(track.title.as_str()),
            album: non_empty(&track.album),
            artist: non_empty(&track.artist),
            cover_url: non_empty(&track.thumbnail_url),
            duration: (track.duration_secs.is_finite() && track.duration_secs > 0.)
                .then(|| Duration::from_secs_f64(track.duration_secs)),
        });
    }

    pub fn set_playback(&mut self, playing: bool, position_secs: f64) {
        let progress = (position_secs.is_finite() && position_secs >= 0.)
            .then(|| souvlaki::MediaPosition(Duration::from_secs_f64(position_secs)));
        let state = if playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        };
        let _ = self.controls.set_playback(state);
    }

}

fn non_empty(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}
