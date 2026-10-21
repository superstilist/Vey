//! Linux frame adapter. FFmpeg is an optional runtime decoder, with silent artwork fallback.
use super::*;
use std::{
    io::Read,
    process::{Child, Command, Stdio},
    sync::mpsc,
};
#[derive(Default)]
pub struct Surface {
    process: Option<Child>,
    frames: Option<mpsc::Receiver<Vec<u8>>>,
    url: String,
    position: f64,
    at: Option<std::time::Instant>,
    playing: bool,
    rate: f64,
    failed_url: String,
}
impl Surface {
    fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.frames = None;
    }
    pub fn update(&mut self, w: &MainWindow, state: serde_json::Value) {
        let url = state["url"].as_str().unwrap_or("");
        let visible = state["visible"].as_bool().unwrap_or(false) && !url.is_empty();
        let playing = state["playing"].as_bool().unwrap_or(false);
        let position = state["position"].as_f64().unwrap_or(0.);
        let rate = state["rate"].as_f64().unwrap_or(1.).clamp(0.25, 4.);
        let predicted = self.position
            + if self.playing {
                self.at
                    .map(|at| at.elapsed().as_secs_f64() * self.rate)
                    .unwrap_or(0.)
            } else {
                0.
            };
        if !visible {
            self.stop();
            self.url.clear();
            w.set_video_frame(Default::default());
            return;
        }
        if url != self.url
            || playing != self.playing
            || (rate - self.rate).abs() > 0.001
            || (position - predicted).abs() > 0.3
        {
            self.stop();
            self.url = url.into();
            self.position = position;
            self.at = Some(std::time::Instant::now());
            self.playing = playing;
            self.rate = rate;
            if self.failed_url == url {
                return;
            }
            let mut cmd = Command::new("ffmpeg");
            cmd.args(["-v","error","-nostdin","-readrate",&rate.to_string(),"-ss",&position.to_string(),"-i",url,"-an","-vf",&format!("setpts=PTS/{rate},scale=480:270:force_original_aspect_ratio=decrease,pad=480:270:(ow-iw)/2:(oh-ih)/2"),"-r","24"]);
            if !playing {
                cmd.args(["-frames:v", "1"]);
            }
            match cmd
                .args(["-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(mut child) => {
                    let mut output = child.stdout.take().unwrap();
                    let (tx, rx) = mpsc::sync_channel(2);
                    std::thread::spawn(move || loop {
                        let mut bytes = vec![0; 480 * 270 * 4];
                        if output.read_exact(&mut bytes).is_err() || tx.send(bytes).is_err() {
                            break;
                        }
                    });
                    self.frames = Some(rx);
                    self.process = Some(child);
                }
                Err(_) => {
                    self.failed_url = url.into();
                    w.set_video_frame(Default::default());
                }
            }
        }
        if let Some(rx) = &self.frames {
            if let Some(bytes) = rx.try_iter().last() {
                w.set_video_frame(slint::Image::from_rgba8(
                    slint::SharedPixelBuffer::clone_from_slice(&bytes, 480, 270),
                ));
            }
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        self.stop();
    }
}
