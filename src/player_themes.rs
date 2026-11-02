//! Whole-player profiles share the lyric catalog. Explicit Vey CSS properties
//! customize semantic roles without importing YouTube DOM selectors into Slint.
use crate::{themes::NativeTheme, PlayerThemeStyle};
use serde::{Deserialize, Serialize};

/// Install once per window, before restoring saved appearance or artwork.
pub fn install(w: &crate::MainWindow) {
    crate::material::install(w);
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Overrides {
    pub accent: Option<u32>,
    pub background: Option<u32>,
    pub surface: Option<u32>,
    pub ink: Option<u32>,
    pub muted: Option<u32>,
    pub corner: Option<f32>,
    pub artwork_corner: Option<f32>,
    pub control_corner: Option<f32>,
    pub artwork_share: Option<f32>,
    pub ambient: Option<f32>,
    pub motion_ms: Option<f32>,
    pub family: Option<String>,
}
fn mix(a: u32, b: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let a = ((a >> shift) & 255u32) as f32;
        let b = ((b >> shift) & 255u32) as f32;
        (a + (b - a) * t).round() as u32
    };
    0xff000000 | channel(16) << 16 | channel(8) << 8 | channel(0)
}
fn opaque(value: u32) -> u32 {
    value | 0xff000000
}
fn color(value: u32) -> slint::Color {
    slint::Color::from_argb_encoded(opaque(value))
}
fn luminance(value: u32) -> f32 {
    let linear = |v: u32| {
        let x = v as f32 / 255.;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    linear(value >> 16 & 255) * 0.2126
        + linear(value >> 8 & 255) * 0.7152
        + linear(value & 255) * 0.0722
}
fn contrast(a: u32, b: u32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn readable(ink: u32, background: u32) -> u32 {
    if contrast(ink, background) >= 4.5 {
        opaque(ink)
    } else if contrast(0xffffff, background) >= contrast(0, background) {
        0xffffffff
    } else {
        0xff000000
    }
}
pub fn preset_id(source: &str) -> &'static str {
    let source = source
        .split("/* vey-appearance-controls:start */")
        .next()
        .unwrap_or(source)
        .trim();
    crate::theme_presets::all()
        .iter()
        .find(|p| p.css.trim() == source)
        .map_or("custom", |p| p.id)
}
pub fn resolve(theme: &NativeTheme, entire_player: bool) -> PlayerThemeStyle {
    let id = preset_id(&theme.source);
    let active = entire_player
        && ((id != "braccato" && !theme.source.trim().is_empty())
            || theme.player != Overrides::default());
    let accent = theme.player.accent.unwrap_or_else(|| match id {
        "apple-music" => 0xfffa586a,
        "minimal" | "opacity-based-karaoke" | "crisp" => 0xffeeeeef,
        "material3" => 0xff79d7c2,
        _ => theme
            .appearance
            .glow_color
            .filter(|c| *c & 0xffffff != 0xffffff)
            .unwrap_or(theme.active),
    });
    let background = opaque(theme.player.background.unwrap_or_else(|| match id {
        "apple-music" => 0xff000000,
        "minimal" | "opacity-based-karaoke" => 0xff0a0a0a,
        "material3" => 0xff161219,
        _ => mix(0x101110, accent, 0.035),
    }));
    let mut surface = theme
        .player
        .surface
        .unwrap_or_else(|| mix(background, accent, 0.09));
    let mut high = mix(surface, 0xffffff, 0.045);
    let mut higher = mix(surface, 0xffffff, 0.09);
    let mut primary_container = mix(background, accent, 0.20);
    let mut secondary_container = mix(background, accent, 0.09);
    let mut tertiary_container = mix(background, accent, 0.12);
    let mut ink = theme.player.ink.unwrap_or(0xfff3f2f5);
    let mut muted = theme.player.muted.unwrap_or(mix(ink, background, 0.26));
    // Roles evaluated from the pinned upstream stylesheet in dark mode with
    // --m3v-source:#d1bcff; provenance records source and extraction recipe.
    if id == "material3" {
        surface = theme.player.surface.unwrap_or(0xff231e26);
        if theme.player.surface.is_none() {
            high = 0xff2e2830;
            higher = 0xff39333b;
        }
        if theme.player.accent.is_none() {
            primary_container = 0xff005144;
            secondary_container = 0xff603b50;
            tertiary_container = 0xff5c3a65;
        }
        ink = theme.player.ink.unwrap_or(0xffeadfea);
        muted = theme.player.muted.unwrap_or(0xffd0c2d3);
    }
    ink = readable(ink, surface);
    muted = readable(muted, surface);
    let (corner, artwork_corner, control_corner, share, ambient, motion) = match id {
        "apple-music" => (12., 12., 10., 0.45, 1.25, 600.),
        "material3" => (28., 28., 999., 0.43, 0.45, 700.),
        "minimal" | "opacity-based-karaoke" | "crisp" => (8., 8., 8., 0.42, 0., 240.),
        "lucid" => (20., 16., 999., 0.46, 0.8, 850.),
        "spotlight" => (20., 20., 999., 0.40, 0.35, 700.),
        "sustain" => (24., 16., 999., 0.42, 0.85, 900.),
        "nocturne" => (20., 20., 999., 0.43, 0.35, 800.),
        "neon-pulse" => (20., 20., 999., 0.43, 1.15, 600.),
        _ => (28., 24., 999., 0.43, 0.85, 800.),
    };
    let p = &theme.player;
    PlayerThemeStyle {
        enabled: active,
        name: crate::theme_presets::all()
            .iter()
            .find(|p| p.id == id)
            .map_or("Custom", |p| p.name)
            .into(),
        primary: color(accent),
        primary_light: color(mix(accent, 0xffffff, 0.25)),
        primary_dark: color(mix(accent, background, 0.25)),
        on_primary: color(
            if contrast(0x101110, accent) >= contrast(0xffffff, accent) {
                0x101110
            } else {
                0xffffff
            },
        ),
        primary_container: color(primary_container),
        secondary_container: color(secondary_container),
        tertiary_container: color(tertiary_container),
        background: color(background),
        surface_lowest: color(mix(background, 0, 0.12)),
        surface: color(surface),
        surface_high: color(high),
        surface_higher: color(higher),
        outline: color(mix(surface, ink, 0.3)),
        ink: color(ink),
        ink_soft: color(mix(ink, background, 0.1)),
        muted: color(muted),
        quiet: color(mix(muted, background, 0.12)),
        corner: p.corner.unwrap_or(corner),
        artwork_corner: p.artwork_corner.unwrap_or(artwork_corner),
        control_corner: p.control_corner.unwrap_or(control_corner),
        artwork_share: p.artwork_share.unwrap_or(share),
        ambient: p.ambient.unwrap_or(ambient),
        motion_ms: p.motion_ms.unwrap_or(motion),
        family: p
            .family
            .clone()
            .unwrap_or_else(|| {
                if id == "apple-music" {
                    "Segoe UI".into()
                } else {
                    "Roboto".into()
                }
            })
            .into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_are_readable_and_control_edits_keep_the_base_identity() {
        for preset in crate::theme_presets::all() {
            let theme = crate::themes::convert(preset.css);
            let p = resolve(&theme, true);
            assert!(
                contrast(p.ink.as_argb_encoded(), p.surface.as_argb_encoded()) >= 4.5,
                "{} text",
                preset.id
            );
            assert!(
                contrast(p.muted.as_argb_encoded(), p.surface.as_argb_encoded()) >= 4.5,
                "{} muted",
                preset.id
            );
            assert!(
                contrast(p.on_primary.as_argb_encoded(), p.primary.as_argb_encoded()) >= 4.5,
                "{} controls",
                preset.id
            );
            assert_eq!(p.enabled, !preset.css.is_empty());
            assert!(!resolve(&theme, false).enabled);
            assert_eq!(preset_id(&format!("{}\n/* vey-appearance-controls:start */ :root{{font-size:36px}} /* vey-appearance-controls:end */",preset.css)),preset.id);
        }
    }
}
