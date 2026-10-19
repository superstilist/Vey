//! Platform-independent audio control contract. Sources are resolved before load.
#[derive(Debug, Clone, Default)]
pub struct AudioState {
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub ended: bool,
}
pub trait AudioEngine {
    fn play(&self) -> Result<(), String>;
    fn pause(&self) -> Result<(), String>;
    fn seek(&self, seconds: f64) -> Result<(), String>;
    fn volume(&self, value: f32) -> Result<(), String>;
    fn rate(&self, value: f32) -> Result<(), String>;
    fn status(&self) -> Result<AudioState, String>;
}
#[cfg(not(target_arch = "wasm32"))]
impl AudioEngine for crate::native::CoreState {
    fn play(&self) -> Result<(), String> {
        crate::native::player_play(self)
    }
    fn pause(&self) -> Result<(), String> {
        crate::native::player_pause(self)
    }
    fn seek(&self, s: f64) -> Result<(), String> {
        if !s.is_finite() {
            return Err("Некоректний час".into());
        }
        crate::native::player_seek(self, s.max(0.))
    }
    fn volume(&self, v: f32) -> Result<(), String> {
        crate::native::player_volume(self, v.clamp(0., 1.))
    }
    fn rate(&self, v: f32) -> Result<(), String> {
        crate::native::player_rate(self, v.clamp(0.5, 2.))
    }
    fn status(&self) -> Result<AudioState, String> {
        let s = crate::native::player_status(self)?;
        Ok(AudioState {
            playing: s.state == crate::native::PlaybackState::Playing,
            position: s.position_secs,
            duration: s.duration_secs,
            ended: s.ended,
        })
    }
}
#[cfg(target_arch = "wasm32")]
impl AudioEngine for web_sys::HtmlAudioElement {
    fn play(&self) -> Result<(), String> {
        web_sys::HtmlMediaElement::play(self)
            .map(|_| ())
            .map_err(|_| "Відтворення заблоковано браузером".into())
    }
    fn pause(&self) -> Result<(), String> {
        web_sys::HtmlMediaElement::pause(self).map_err(|_| "Помилка паузи".into())
    }
    fn seek(&self, s: f64) -> Result<(), String> {
        if !s.is_finite() {
            return Err("Некоректний час".into());
        }
        self.set_current_time(s.max(0.));
        Ok(())
    }
    fn volume(&self, v: f32) -> Result<(), String> {
        self.set_volume(v.clamp(0., 1.) as f64);
        Ok(())
    }
    fn rate(&self, v: f32) -> Result<(), String> {
        self.set_playback_rate(v.clamp(0.5, 2.) as f64);
        Ok(())
    }
    fn status(&self) -> Result<AudioState, String> {
        Ok(AudioState {
            playing: !self.paused(),
            position: self.current_time(),
            duration: self.duration(),
            ended: self.ended(),
        })
    }
}
