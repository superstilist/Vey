//! Cover-derived tonal roles. Sampling is bounded and runs only on art changes.
use crate::{MainWindow, Tokens};
use slint::ComponentHandle;

fn rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1. - (2. * l - 1.).abs()) * s;
    let x = c * (1. - ((h / 60.) % 2. - 1.).abs());
    let (r, g, b) = match (h / 60.) as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = l - c / 2.;
    [
        ((r + m) * 255.).round() as u8,
        ((g + m) * 255.).round() as u8,
        ((b + m) * 255.).round() as u8,
    ]
}
fn hue(bytes: &[u8]) -> f32 {
    let mut weights = [0f32; 24];
    let mut hues = [0f32; 24];
    let step = (bytes.len() / 4 / 2048).max(1);
    for p in bytes.chunks_exact(4).step_by(step) {
        if p[3] < 200 {
            continue;
        }
        let (r, g, b) = (p[0] as f32 / 255., p[1] as f32 / 255., p[2] as f32 / 255.);
        let high = r.max(g).max(b);
        let low = r.min(g).min(b);
        let delta = high - low;
        if delta < 0.08 || high < 0.12 || low > 0.9 {
            continue;
        }
        let h = (if high == r {
            (g - b) / delta
        } else if high == g {
            (b - r) / delta + 2.
        } else {
            (r - g) / delta + 4.
        } * 60.)
            .rem_euclid(360.);
        let bucket = ((h / 15.) as usize).min(23);
        let weight = delta * delta;
        weights[bucket] += weight;
        hues[bucket] += h * weight;
    }
    let winner = (0..24)
        .max_by(|&a, &b| weights[a].total_cmp(&weights[b]))
        .unwrap();
    if weights[winner] < 0.01 {
        85.
    } else {
        hues[winner] / weights[winner]
    }
}
fn color(h: f32, s: f32, l: f32) -> slint::Color {
    let [r, g, b] = rgb(h.rem_euclid(360.), s, l);
    slint::Color::from_rgb_u8(r, g, b)
}
pub fn install(w: &MainWindow) {
    let weak = w.as_weak();
    w.on_artwork_theme(move || {
        let Some(w) = weak.upgrade() else {
            return;
        };
        // Keep the previous palette while the next cover is still loading.
        let Some(image) = w.get_current_art().to_rgba8() else {
            return;
        };
        let h = hue(image.as_bytes());
        let t = w.global::<Tokens>();
        t.set_brand_500(color(h, 0.45, 0.73));
        t.set_brand_300(color(h, 0.50, 0.84));
        t.set_brand_600(color(h, 0.28, 0.53));
        t.set_brand_ink(color(h, 0.45, 0.09));
        t.set_primary_container(color(h, 0.25, 0.22));
        t.set_secondary_container(color(h + 35., 0.10, 0.22));
        t.set_tertiary_container(color(h + 115., 0.13, 0.23));
        t.set_neutral_0(color(h, 0.10, 0.065));
        t.set_neutral_50(color(h, 0.11, 0.09));
        t.set_neutral_100(color(h, 0.11, 0.12));
        t.set_neutral_150(color(h, 0.11, 0.16));
        t.set_neutral_200(color(h, 0.10, 0.21));
        t.set_neutral_300(color(h, 0.08, 0.30));
        t.set_ink_100(color(h, 0.18, 0.92));
        t.set_ink_150(color(h, 0.14, 0.83));
        t.set_ink_200(color(h, 0.12, 0.74));
        t.set_ink_300(color(h, 0.10, 0.64));
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(rgb: [u8; 3]) -> f32 {
        let c = rgb.map(|x| {
            let x = x as f32 / 255.;
            if x <= 0.04045 {
                x / 12.92
            } else {
                ((x + 0.055) / 1.055).powf(2.4)
            }
        });
        c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
    }
    #[test]
    fn neutral_and_transparent_covers_use_brand_fallback() {
        assert_eq!(hue(&[120, 120, 120, 255]), 85.);
        assert_eq!(hue(&[255, 0, 0, 0]), 85.);
        assert_eq!(hue(&[]), 85.);
    }
    #[test]
    fn cover_hues_are_distinct_and_tonal_contrast_is_readable() {
        assert!((hue(&[0, 0, 255, 255]) - 240.).abs() < 1.);
        assert!((hue(&[255, 128, 0, 255]) - 30.).abs() < 1.);
        for h in (0..360).step_by(15) {
            let h = h as f32;
            let text =
                (luminance(rgb(h, 0.45, 0.73)) + 0.05) / (luminance(rgb(h, 0.45, 0.09)) + 0.05);
            let surface =
                (luminance(rgb(h, 0.18, 0.92)) + 0.05) / (luminance(rgb(h, 0.25, 0.22)) + 0.05);
            assert!(text >= 4.5 && surface >= 4.5, "hue {h}: {text}, {surface}");
        }
    }
}
