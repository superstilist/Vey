//! Bounded native backdrop for platforms without the browser Kawarp renderer.
//! Blur/color preparation runs only on artwork/options changes; hidden/static work stops.
use crate::{services::KawarpSettings, MainWindow};
use slint::ComponentHandle;
use std::time::{Duration, Instant};
const W: u32 = 256;
const H: u32 = 144;
const TRAVEL_FRAME_INTERVAL: Duration = Duration::from_millis(100);

fn present_during_travel(moving: bool, reduced: bool, has_frame: bool, elapsed: Duration) -> bool {
    !moving || reduced || !has_frame || elapsed >= TRAVEL_FRAME_INTERVAL
}

// One in-flight frame bounds the CPU queue even while the UI is busy.
struct FrameJob {
    source: Option<(image::RgbaImage, KawarpSettings)>,
    phase: f32,
    reduced: bool,
    mix: f32,
    warp: f32,
}

pub fn install(w: &MainWindow) -> slint::Timer {
    let (send, jobs) = std::sync::mpsc::sync_channel::<FrameJob>(1);
    let (done, frames) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("vey-ambient".into())
        .spawn(move || {
            let mut target = Vec::new();
            let mut from = Vec::new();
            let mut shown = Vec::new();
            while let Ok(job) = jobs.recv() {
                if let Some((source, options)) = job.source {
                    target = prepare(&source, &options);
                    from.clone_from(&shown);
                }
                crate::ambient_motion::compose(
                    &target,
                    &from,
                    &mut shown,
                    (W as usize, H as usize),
                    job.phase,
                    job.warp,
                    job.reduced,
                    job.mix,
                );
                if done.send(shown.clone()).is_err() {
                    break;
                }
            }
        })
        .expect("native ambient worker");
    let throttle_travel = std::env::var("VEY_LEGACY_AMBIENT_TRAVEL").as_deref() != Ok("1");
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    let mut art = slint::Image::default();
    let mut options = KawarpSettings::default();
    let mut initialized = false;
    let mut pending = false;
    let mut has_frame = false;
    let mut settled = false;
    let mut last_reduced = false;
    let mut started = Instant::now();
    let mut phase = 0.;
    let mut last = Instant::now();
    let mut last_presented = Instant::now();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(33),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
            let _timer_span = crate::player_debug::begin_span(&w, "ambient_timer_callback");
            let now = Instant::now();
            let elapsed = now.duration_since(last).as_secs_f32().min(0.2);
            last = now;
            // Always drain a completed job, including while hidden, so resuming
            // cannot leave the bounded worker waiting behind an unconsumed frame.
            if let Ok(pixels) = frames.try_recv() {
                pending = false;
                w.set_ambient_frame(slint::Image::from_rgba8(
                    slint::SharedPixelBuffer::clone_from_slice(&pixels, W, H),
                ));
                has_frame = true;
                last_presented = now;
            }
            if w.window().is_minimized()
                || (w.get_color_mode() == "static" && !w.get_player_theme_style().enabled)
                || w.get_ambient_opacity() <= 0.001
            {
                return;
            }
            let reduced = w.get_lyrics_reduced_motion();
            let moving = w.get_player_state() == "playing" && !reduced;
            if moving {
                phase += elapsed * 0.24;
            }
            if pending {
                return;
            }
            let next = w.get_current_art();
            let config = KawarpSettings {
                opacity: w.get_kawarp_opacity(),
                warp: w.get_kawarp_warp(),
                blur: w.get_kawarp_blur() as u32,
                saturation: w.get_kawarp_saturation(),
                dithering: w.get_kawarp_dithering(),
                dim_bright: w.get_kawarp_dim(),
            }
            .normalized();
            let mut source = None;
            if !initialized || next != art || config != options {
                let image = next
                    .to_rgba8()
                    .and_then(|p| {
                        image::RgbaImage::from_raw(p.width(), p.height(), p.as_bytes().to_vec())
                    })
                    .unwrap_or_else(|| {
                        image::RgbaImage::from_fn(64, 64, |x, y| {
                            image::Rgba([
                                (40 + x * 2) as u8,
                                (45 + y) as u8,
                                (110 + y * 2) as u8,
                                255,
                            ])
                        })
                    });
                source = Some((image, config.clone()));
                art = next;
                options = config;
                initialized = true;
                started = now;
                settled = false;
            }
            if reduced != last_reduced {
                settled = false;
                last_reduced = reduced;
            }
            let t = if reduced {
                1.
            } else {
                now.duration_since(started).as_secs_f32().min(1.)
            };
            if !moving && t >= 1. && settled {
                return;
            }
            if source.is_none()
                && throttle_travel
                && !present_during_travel(
                    w.get_page_moving(),
                    reduced,
                    has_frame,
                    now.duration_since(last_presented),
                )
            {
                return;
            }
            let mix = t * t * t * (t * (t * 6. - 15.) + 10.);
            pending = send
                .try_send(FrameJob {
                    source,
                    phase,
                    reduced,
                    mix,
                    warp: options.warp,
                })
                .is_ok();
            settled = !moving && t >= 1.;
        },
    );
    timer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_throttles_only_existing_non_reduced_frames() {
        let short = Duration::from_millis(33);
        assert!(!present_during_travel(true, false, true, short));
        assert!(present_during_travel(
            true,
            false,
            true,
            TRAVEL_FRAME_INTERVAL
        ));
        assert!(present_during_travel(false, false, true, short));
        assert!(present_during_travel(true, true, true, short));
        assert!(present_during_travel(true, false, false, short));
    }
}
fn prepare(image: &image::RgbaImage, k: &KawarpSettings) -> Vec<u8> {
    let small = image::imageops::resize(image, W, H, image::imageops::FilterType::Triangle);
    let mut pixels = image::imageops::blur(&small, k.blur as f32 * 0.6).into_raw();
    let brightness = pixels
        .chunks_exact(4)
        .map(|p| {
            (p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722)
                * (p[3] as f32 / 255.)
        })
        .sum::<f32>()
        / (W * H) as f32
        / 255.;
    let dim = if k.dim_bright && brightness > 0.6 {
        1. - ((brightness - 0.6) / 0.4).min(1.) * 0.45
    } else {
        1.
    };
    for (index, p) in pixels.chunks_exact_mut(4).enumerate() {
        let gray = p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722;
        let noise = (((index as u32)
            .wrapping_mul(1664525)
            .wrapping_add(1013904223)
            >> 24) as f32
            / 255.
            - 0.5)
            * k.dithering
            * 255.;
        for c in 0..3 {
            p[c] =
                ((gray + (p[c] as f32 - gray) * k.saturation) * dim + noise).clamp(0., 255.) as u8;
        }
        p[3] = 255;
    }
    pixels
}
