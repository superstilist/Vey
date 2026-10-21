//! Cover-derived tonal roles. Sampling is bounded and runs only on art changes.
use crate::services::ColorMode;
use crate::{MainWindow, Tokens};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc, time::Duration};
use web_time::Instant;

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
    let mut hue_x = [0f32; 24];
    let mut hue_y = [0f32; 24];
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
        // Favor the dominant colored area instead of a tiny saturated logo.
        // Neighboring bins form one family, including reds crossing 0/360.
        let weight = delta.sqrt();
        weights[bucket] += weight;
        hue_x[bucket] += h.to_radians().cos() * weight;
        hue_y[bucket] += h.to_radians().sin() * weight;
    }
    let family_weight = |i: usize| weights[(i + 23) % 24] + weights[i] + weights[(i + 1) % 24];
    let winner = (0..24)
        .max_by(|&a, &b| family_weight(a).total_cmp(&family_weight(b)))
        .unwrap();
    if family_weight(winner) < 0.01 {
        85.
    } else {
        let bins = [(winner + 23) % 24, winner, (winner + 1) % 24];
        let x: f32 = bins.iter().map(|&i| hue_x[i]).sum();
        let y: f32 = bins.iter().map(|&i| hue_y[i]).sum();
        y.atan2(x).to_degrees().rem_euclid(360.)
    }
}

fn cover_chroma(bytes: &[u8]) -> f32 {
    let mut chroma = 0.;
    let mut count = 0.;
    let step = (bytes.len() / 4 / 2048).max(1);
    for pixel in bytes.chunks_exact(4).step_by(step) {
        if pixel[3] < 200 {
            continue;
        }
        let high = pixel[0].max(pixel[1]).max(pixel[2]) as f32;
        let low = pixel[0].min(pixel[1]).min(pixel[2]) as f32;
        chroma += (high - low) / 255.;
        count += 1.;
    }
    if count == 0. {
        1.
    } else {
        (chroma / count / 0.35).clamp(0., 1.)
    }
}
fn color(h: f32, s: f32, l: f32) -> slint::Color {
    let [r, g, b] = rgb(h.rem_euclid(360.), s, l);
    slint::Color::from_rgb_u8(r, g, b)
}
#[derive(Clone, Copy, PartialEq)]
struct Palette([slint::Color; 17]);
fn palette(h: f32) -> Palette {
    Palette([
        color(h, 0.45, 0.73),
        color(h, 0.50, 0.84),
        color(h, 0.28, 0.53),
        color(h, 0.45, 0.09),
        color(h, 0.25, 0.22),
        color(h + 35., 0.10, 0.22),
        color(h + 115., 0.13, 0.23),
        color(h, 0.10, 0.065),
        color(h, 0.11, 0.09),
        color(h, 0.11, 0.12),
        color(h, 0.11, 0.16),
        color(h, 0.10, 0.21),
        color(h, 0.08, 0.30),
        color(h, 0.18, 0.92),
        color(h, 0.14, 0.83),
        color(h, 0.12, 0.74),
        color(h, 0.10, 0.64),
    ])
}
fn target_palette(mode: ColorMode, seed: f32, cover: Option<f32>) -> Palette {
    let fixed = palette(seed);
    match mode {
        ColorMode::Static => fixed,
        ColorMode::Dynamic => palette(cover.unwrap_or(seed)),
        ColorMode::Background => {
            let mut result = fixed;
            result.0[7..13].copy_from_slice(&palette(cover.unwrap_or(seed)).0[7..13]);
            result
        }
    }
}

fn artwork_target(mode: ColorMode, seed: f32, cover: Option<f32>, chroma: f32) -> Palette {
    let mut target = target_palette(mode, seed, cover);
    if mode == ColorMode::Static || cover.is_none() {
        return target;
    }
    let range = if mode == ColorMode::Background {
        7..13
    } else {
        0..17
    };
    for value in &mut target.0[range] {
        let gray = value.red() as f32 * 0.2126
            + value.green() as f32 * 0.7152
            + value.blue() as f32 * 0.0722;
        let mix = |channel: u8| (gray + (channel as f32 - gray) * chroma).round() as u8;
        *value = slint::Color::from_rgb_u8(mix(value.red()), mix(value.green()), mix(value.blue()));
    }
    target
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * t * (t * (t * 6. - 15.) + 10.)
}
impl Palette {
    fn blend(self, target: Self, t: f32) -> Self {
        Self(std::array::from_fn(|i| {
            let a = self.0[i];
            let b = target.0[i];
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
            slint::Color::from_rgb_u8(
                mix(a.red(), b.red()),
                mix(a.green(), b.green()),
                mix(a.blue(), b.blue()),
            )
        }))
    }
}
macro_rules! roles {
    ($($get:ident => $set:ident),* $(,)?) => {
        impl Palette {
            fn read(w: &MainWindow) -> Self { let t = w.global::<Tokens>(); Self([$(t.$get()),*]) }
            fn paint(self, w: &MainWindow) { let t = w.global::<Tokens>(); let mut values = self.0.into_iter(); $(t.$set(values.next().unwrap());)* }
        }
    }
}
roles! {
    get_brand_500 => set_brand_500, get_brand_300 => set_brand_300,
    get_brand_600 => set_brand_600, get_brand_ink => set_brand_ink,
    get_primary_container => set_primary_container, get_secondary_container => set_secondary_container,
    get_tertiary_container => set_tertiary_container, get_neutral_0 => set_neutral_0,
    get_neutral_50 => set_neutral_50, get_neutral_100 => set_neutral_100,
    get_neutral_150 => set_neutral_150, get_neutral_200 => set_neutral_200,
    get_neutral_300 => set_neutral_300, get_ink_100 => set_ink_100,
    get_ink_150 => set_ink_150, get_ink_200 => set_ink_200, get_ink_300 => set_ink_300,
}
struct Transition {
    from: Palette,
    target: Palette,
    at: Instant,
    cover: Option<f32>,
    chroma: f32,
    art: slint::Image,
}
pub fn install(w: &MainWindow) {
    let initial = Palette::read(w);
    let state = Rc::new(RefCell::new(Transition {
        from: initial,
        target: initial,
        at: Instant::now(),
        cover: None,
        chroma: 1.,
        art: Default::default(),
    }));
    let timer = Rc::new(slint::Timer::default());
    let weak = w.as_weak();
    w.on_artwork_theme(move || {
        let Some(w) = weak.upgrade() else {
            return;
        };
        let mut transition = state.borrow_mut();
        let art = w.get_current_art();
        if art != transition.art {
            // Empty artwork is commonly a loading state: retain the last hue.
            if let Some(image) = art.to_rgba8() {
                transition.cover = Some(hue(image.as_bytes()));
                transition.chroma = cover_chroma(image.as_bytes());
            }
            transition.art = art;
        }
        let target = artwork_target(
            ColorMode::from_name(&w.get_color_mode()),
            w.get_color_hue(),
            transition.cover,
            transition.chroma,
        );
        if w.get_lyrics_reduced_motion() {
            timer.stop();
            target.paint(&w);
            w.set_colors_changing(false);
            transition.target = target;
            return;
        }
        if transition.target == target {
            return;
        }
        // Retarget from the displayed mixture, including interrupted fades.
        transition.from = Palette::read(&w);
        transition.target = target;
        transition.at = Instant::now();
        w.set_colors_changing(true);
        drop(transition);
        let state = state.clone();
        let weak = w.as_weak();
        let weak_timer = Rc::downgrade(&timer);
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(32),
            move || {
                let Some(w) = weak.upgrade() else {
                    return;
                };
                let state = state.borrow();
                let duration = w.global::<crate::Motion>().get_palette().max(1) as f32;
                let t = (state.at.elapsed().as_secs_f32() * 1000. / duration).min(1.);
                state.from.blend(state.target, smooth(t)).paint(&w);
                if t >= 1. {
                    w.set_colors_changing(false);
                    if let Some(timer) = weak_timer.upgrade() {
                        timer.stop();
                    }
                }
            },
        );
    });
    w.invoke_artwork_theme();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes_keep_static_roles_independent_of_artwork() {
        let fixed = target_palette(ColorMode::Static, 85., Some(0.));
        assert!(fixed == target_palette(ColorMode::Static, 85., Some(240.)));
        let warm = target_palette(ColorMode::Background, 85., Some(0.));
        let cool = target_palette(ColorMode::Background, 85., Some(240.));
        assert!(warm.0[..7] == fixed.0[..7] && cool.0[..7] == fixed.0[..7]);
        assert!(warm.0[13..] == fixed.0[13..] && cool.0[13..] == fixed.0[13..]);
        assert!(warm.0[7..13] != cool.0[7..13]);
        assert!(
            target_palette(ColorMode::Dynamic, 85., Some(0.))
                != target_palette(ColorMode::Dynamic, 85., Some(240.))
        );
        let a = palette(0.);
        let b = palette(240.);
        assert!(a.blend(b, smooth(0.)) == a && a.blend(b, smooth(1.)) == b);
        assert!(a.blend(b, smooth(0.5)) != a && a.blend(b, smooth(0.5)) != b);
    }
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
    fn artwork_theme_preserves_neutral_colors_and_dominant_muted_hues() {
        let gray = [150, 150, 150, 255];
        let neutral = artwork_target(
            ColorMode::Dynamic,
            85.,
            Some(hue(&gray)),
            cover_chroma(&gray),
        );
        assert!(neutral
            .0
            .iter()
            .all(|c| c.red() == c.green() && c.green() == c.blue()));
        let fixed = artwork_target(ColorMode::Static, 85., Some(240.), 0.);
        assert!(fixed == palette(85.));
        let mut pixels = [90, 110, 140, 255].repeat(90);
        pixels.extend([255, 0, 0, 255].repeat(10));
        assert!(
            (hue(&pixels) - 216.).abs() < 3.,
            "dominant muted blue wins over small red details"
        );
        let reds = [[255, 0, 8, 255], [255, 8, 0, 255]].concat();
        let red = hue(&reds);
        assert!(
            red < 2. || red > 358.,
            "reds across hue boundary stay red: {red}"
        );
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
