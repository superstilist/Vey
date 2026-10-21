//! Bounded native backdrop for platforms without the browser Kawarp renderer.
//! Blur/color preparation runs only on artwork/options changes; hidden/static work stops.
use crate::{services::KawarpSettings, MainWindow};
use slint::ComponentHandle;
use std::time::{Duration, Instant};
const W: u32 = 256;
const H: u32 = 144;
pub fn install(w: &MainWindow) -> slint::Timer {
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    let mut art = slint::Image::default();
    let mut options = KawarpSettings::default();
    let mut target = Vec::<u8>::new();
    let mut from = Vec::<u8>::new();
    let mut shown = Vec::<u8>::new();
    let mut settled = false;
    let mut last_reduced = false;
    let mut started = Instant::now();
    let mut phase = 0.;
    let mut last = Instant::now();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(64),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
            let now = Instant::now();
            let elapsed = now.duration_since(last).as_secs_f32().min(0.2);
            last = now;
            if w.window().is_minimized() || w.get_color_mode() == "static" {
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
            let empty = next.size().width == 0 || next.size().height == 0;
            if !empty && (next != art || config != options) {
                if let Some(pixels) = next.to_rgba8() {
                    let image = image::RgbaImage::from_raw(
                        pixels.width(),
                        pixels.height(),
                        pixels.as_bytes().to_vec(),
                    )
                    .unwrap();
                    target = prepare(&image, &config);
                    from = shown.clone();
                    started = now;
                    art = next;
                    options = config;
                    settled = false;
                }
            }
            if target.is_empty() {
                return;
            }
            let reduced = w.get_lyrics_reduced_motion();
            if reduced != last_reduced {
                settled = false;
                last_reduced = reduced;
            }
            let moving = w.get_player_state() == "playing" && !reduced;
            let t = if reduced {
                1.
            } else {
                now.duration_since(started).as_secs_f32().min(1.)
            };
            if !moving && t >= 1. && settled {
                return;
            }
            if moving {
                phase += elapsed * 0.24;
            }
            let mix = t * t * t * (t * (t * 6. - 15.) + 10.);
            shown.resize(target.len(), 0);
            for y in 0..H as usize {
                let shift = if !reduced {
                    ((y as f32 / 24. + phase).sin() * options.warp * 10.) as i32
                } else {
                    0
                };
                for x in 0..W as usize {
                    let sample = (x as i32 + shift).clamp(0, W as i32 - 1) as usize;
                    for c in 0..4 {
                        let i = (y * W as usize + x) * 4 + c;
                        let value = target[(y * W as usize + sample) * 4 + c];
                        shown[i] = if from.len() == target.len() && t < 1. {
                            (from[i] as f32 + (value as f32 - from[i] as f32) * mix).round() as u8
                        } else {
                            value
                        };
                    }
                }
            }
            settled = !moving && t >= 1.;
            w.set_ambient_frame(slint::Image::from_rgba8(
                slint::SharedPixelBuffer::clone_from_slice(&shown, W, H),
            ));
        },
    );
    timer
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
