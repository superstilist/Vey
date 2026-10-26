//! CSS-to-native conversion. Unsupported rules are returned for user review.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub family: String,
    pub glow_color: Option<u32>,
    pub glow_radius: f32,
    pub glow_duration: f32,
    pub glow: bool,
    pub swipe: bool,
    pub wave: bool,
    pub wobble: bool,
    pub wobble_duration: f32,
    pub wobble_strength: f32,
    pub fade: bool,
    pub inactive_blur: f32,
    pub line_spacing: f32,
    pub active_scale: f32,
    pub inactive_scale: f32,
    pub swipe_duration: f32,
    pub swipe_lead: f32,
    pub wave_duration: f32,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            family: "Satoshi".into(),
            glow_color: None,
            glow_radius: 6.,
            glow_duration: 1200.,
            glow: true,
            swipe: true,
            wave: false,
            wobble: true,
            wobble_duration: 1000.,
            wobble_strength: 1.,
            fade: true,
            inactive_blur: 2.,
            line_spacing: 1.333,
            active_scale: 1.,
            inactive_scale: 0.95,
            swipe_duration: 1.6,
            swipe_lead: 0.1,
            wave_duration: 900.,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTheme {
    #[serde(default)]
    pub source: String,
    pub active: u32,
    pub inactive: u32,
    pub size: f32,
    pub weight: i32,
    pub unsupported: Vec<String>,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub settings: crate::braccato::settings::ThemeSettings,
}
impl Default for NativeTheme {
    fn default() -> Self {
        Self {
            source: String::new(),
            active: 0xfff5f5f5,
            inactive: 0xff888888,
            size: 48.,
            weight: 700,
            unsupported: vec![],
            appearance: Appearance::default(),
            settings: Default::default(),
        }
    }
}
impl NativeTheme {
    pub fn style(&self) -> crate::LyricStyle {
        let a = &self.appearance;
        crate::LyricStyle {
            family: a.family.clone().into(),
            glow_color: slint::Color::from_argb_encoded(
                a.glow_color
                    .unwrap_or((self.active & 0xffffff) | 0x50000000),
            ),
            glow_radius: a.glow_radius,
            glow_duration: a.glow_duration,
            glow: a.glow,
            swipe: a.swipe,
            wave: a.wave,
            wobble: a.wobble,
            wobble_duration: a.wobble_duration,
            wobble_strength: a.wobble_strength,
            fade: a.fade,
            inactive_blur: a.inactive_blur,
            line_spacing: a.line_spacing,
            active_scale: a.active_scale,
            inactive_scale: a.inactive_scale,
            swipe_duration: a.swipe_duration,
            swipe_lead: a.swipe_lead,
            wave_duration: a.wave_duration,
        }
    }
    /// Apply the same supported theme behavior on desktop and WASM.
    pub fn apply(&self, w: &crate::MainWindow, reduced: bool, passive: bool) {
        w.set_lyric_active(slint::Color::from_argb_encoded(self.active));
        w.set_lyric_inactive(slint::Color::from_argb_encoded(self.inactive));
        w.set_lyric_size(self.size);
        w.set_lyric_weight(self.weight);
        w.set_lyric_style(self.style());
        w.set_lyrics_scroll_ratio(self.settings.target_ratio);
        w.set_lyrics_lookahead_ms(self.settings.lookahead_ms.clamp(0, i32::MAX as i64) as i32);
        w.set_lyrics_scroll_offset_ms(self.settings.scroll_offset_ms.clamp(-60000, 60000) as i32);
        w.set_lyrics_ending_threshold_ms(self.settings.ending_threshold_ms.clamp(0, 60000) as i32);
        w.set_lyrics_scroll_duration_ms(self.settings.scroll_duration_ms.min(60000) as i32);
        w.set_lyrics_passive_seconds(self.settings.passive_seconds_per_line);
        w.set_lyrics_disable_richsync(self.settings.disable_richsync);
        w.set_lyrics_richsync_offset_ms(
            self.settings
                .richsync_offset_ms
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        );
        w.set_lyrics_line_offset_ms(
            self.settings
                .line_offset_ms
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        );
        w.set_lyrics_reduced_motion(reduced || self.settings.reduced_motion);
        w.set_lyrics_passive_scroll(passive || self.settings.passive_scroll);
    }
}
fn color(raw: &str) -> Option<u32> {
    let value = raw.trim();
    match value {
        "white" => return Some(0xffffffff),
        "black" => return Some(0xff000000),
        "transparent" => return Some(0),
        _ => {}
    }
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        let body = value.split_once('(')?.1.strip_suffix(')')?;
        let parts = body
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 3 {
            return None;
        }
        let channel = |s: &str| -> Option<u32> {
            if let Some(v) = s.strip_suffix('%') {
                Some((v.parse::<f32>().ok()?.clamp(0., 100.) * 2.55).round() as u32)
            } else {
                Some(s.parse::<f32>().ok()?.clamp(0., 255.).round() as u32)
            }
        };
        let alpha = parts
            .get(3)
            .map(|v| {
                if let Some(v) = v.strip_suffix('%') {
                    v.parse::<f32>().ok().map(|v| v / 100.)
                } else {
                    v.parse::<f32>().ok()
                }
            })
            .unwrap_or(Some(1.))?
            .clamp(0., 1.);
        return Some(
            ((alpha * 255.).round() as u32) << 24
                | channel(parts[0])? << 16
                | channel(parts[1])? << 8
                | channel(parts[2])?,
        );
    }
    let raw = value.strip_prefix('#')?;
    match raw.len() {
        6 => u32::from_str_radix(raw, 16).ok().map(|v| 0xff000000 | v),
        8 => u32::from_str_radix(raw, 16)
            .ok()
            .map(|v| (v >> 8) | ((v & 0xff) << 24)),
        3 => {
            let mut chars = raw.chars();
            let (r, g, b) = (
                chars.next()?.to_digit(16)?,
                chars.next()?.to_digit(16)?,
                chars.next()?.to_digit(16)?,
            );
            Some(0xff000000 | r * 17 << 16 | g * 17 << 8 | b * 17)
        }
        _ => None,
    }
}
fn number(value: &str, unit: &str, min: f32, max: f32) -> Option<f32> {
    value
        .strip_suffix(unit)?
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v >= min && *v <= max)
}
pub fn from_settings(settings: &crate::services::Settings) -> NativeTheme {
    let mut theme = convert(&settings.lyrics_theme_css);
    if !settings.lyrics_font_family.trim().is_empty() {
        theme.appearance.family = settings.lyrics_font_family.trim().into();
    }
    theme
}
pub fn apply_settings(w: &crate::MainWindow, settings: &crate::services::Settings) {
    let theme = from_settings(settings);
    w.set_lyric_theme_source(settings.lyrics_theme_css.clone().into());
    w.set_lyric_theme_applied(settings.lyrics_theme_css.clone().into());
    w.set_lyric_font_family(settings.lyrics_font_family.clone().into());
    w.set_lyric_style_status(if theme.unsupported.is_empty() {
        "".into()
    } else {
        format!("Unsupported: {}", theme.unsupported.join("; ")).into()
    });
    theme.apply(
        w,
        settings.lyrics_reduced_motion,
        settings.lyrics_passive_scroll,
    );
}
pub fn convert(css: &str) -> NativeTheme {
    let source = css.to_owned();
    let compiled = crate::braccato::rics::compile(css, Default::default());
    let css = compiled.css.as_str();
    let mut theme = NativeTheme {
        source,
        settings: crate::braccato::settings::ThemeSettings::from_css(css),
        ..Default::default()
    };
    theme.unsupported.extend(compiled.errors);
    let comments = regex::Regex::new(r"(?s)/\*.*?\*/").unwrap();
    let css = comments.replace_all(css, "");
    let blocks = regex::Regex::new(r"(?s)([^{}]+)\{([^{}]*)\}").unwrap();
    for block in blocks.captures_iter(&css) {
        let selector = block[1].trim();
        let valid = selector == ":root"
            || selector == ":host"
            || selector.contains("blyrics")
            || selector.contains("braccato");
        if !valid {
            theme.unsupported.push(selector.into());
            continue;
        }
        for declaration in block[2].split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let Some((name, value)) = declaration.split_once(':') else {
                theme.unsupported.push(declaration.into());
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            let supported = match name {
                "--blyrics-active-color" | "--blyrics-lyric-active-color" => {
                    color(value).map(|v| theme.active = v).is_some()
                }
                "--blyrics-inactive-color"
                | "--blyrics-color"
                | "--blyrics-lyric-inactive-color" => {
                    color(value).map(|v| theme.inactive = v).is_some()
                }
                "color" => color(value)
                    .map(|v| {
                        if selector.contains("active") {
                            theme.active = v
                        } else {
                            theme.inactive = v
                        }
                    })
                    .is_some(),
                "font-size" | "--blyrics-font-size" => value
                    .strip_suffix("px")
                    .and_then(|v| v.parse::<f32>().ok())
                    .or_else(|| {
                        value
                            .strip_suffix("rem")
                            .and_then(|v| v.parse::<f32>().ok())
                            .map(|v| v * 16.)
                    })
                    .filter(|v| v.is_finite() && *v >= 12. && *v <= 72.)
                    .map(|v| theme.size = v)
                    .is_some(),
                "--blyrics-target-scroll-pos-ratio" => value
                    .parse::<f32>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map(|v| theme.settings.target_ratio = v.clamp(0., 1.))
                    .is_some(),
                "--blyrics-line-scroll-duration" => crate::braccato::settings::milliseconds(value)
                    .filter(|n| *n >= 0)
                    .map(|n| theme.settings.scroll_duration_ms = n as u64)
                    .is_some(),
                "--blyrics-richsync-timing-offset" => {
                    crate::braccato::settings::milliseconds(value)
                        .map(|n| theme.settings.richsync_offset_ms = n)
                        .is_some()
                }
                "--blyrics-scroll-timing-offset" => crate::braccato::settings::milliseconds(value)
                    .map(|n| theme.settings.scroll_offset_ms = n.clamp(-60000, 60000))
                    .is_some(),
                "--blyrics-timing-offset" => crate::braccato::settings::milliseconds(value)
                    .map(|n| theme.settings.line_offset_ms = n)
                    .is_some(),
                "font-weight" | "--blyrics-font-weight" => value
                    .parse::<i32>()
                    .ok()
                    .filter(|v| *v >= 100 && *v <= 900)
                    .map(|v| theme.weight = v)
                    .is_some(),
                "--blyrics-text-color" => color(value)
                    .map(|v| {
                        theme.active = v;
                        theme.inactive = (v & 0xffffff) | 0x4d000000;
                    })
                    .is_some(),
                "font-family" | "--blyrics-default-font-family" | "--blyrics-font-family" => {
                    let family = value
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches(|c| c == '\'' || c == '"');
                    if !family.is_empty() && family.len() <= 128 {
                        theme.appearance.family = family.into();
                        true
                    } else {
                        false
                    }
                }
                "--blyrics-glow-color" | "--blyrics-highlight-color" => color(value)
                    .map(|v| theme.appearance.glow_color = Some(v))
                    .is_some(),
                "--blyrics-highlight-glow-radius-from" => number(value, "px", 0., 32.)
                    .map(|v| theme.appearance.glow_radius = v)
                    .is_some(),
                "--blyrics-highlight-glow-min-duration" | "--blyrics-letter-wave-duration" => {
                    crate::braccato::settings::milliseconds(value)
                        .filter(|n| *n > 0 && *n <= 10000)
                        .map(|n| {
                            if name.contains("glow") {
                                theme.appearance.glow_duration = n as f32;
                            } else {
                                theme.appearance.wave_duration = n as f32;
                            }
                        })
                        .is_some()
                }
                "--blyrics-line-height" => number(value, "", 1., 2.)
                    .map(|v| theme.appearance.line_spacing = v)
                    .is_some(),
                "--blyrics-active-scale" => number(value, "", 0.8, 1.2)
                    .map(|v| theme.appearance.active_scale = v)
                    .is_some(),
                "--blyrics-scale" => number(value, "", 0.8, 1.2)
                    .map(|v| theme.appearance.inactive_scale = v)
                    .is_some(),
                "--blyrics-inactive-blur" => number(value, "px", 0., 12.)
                    .map(|v| theme.appearance.inactive_blur = v)
                    .is_some(),
                "--blyrics-wobble-duration" => crate::braccato::settings::milliseconds(value)
                    .filter(|n| *n > 0 && *n <= 10000)
                    .map(|n| theme.appearance.wobble_duration = n as f32)
                    .is_some(),
                "--vey-word-wobble-strength" => number(value, "", 0., 2.)
                    .map(|v| theme.appearance.wobble_strength = v)
                    .is_some(),
                "--blyrics-swipe-duration-ratio" => number(value, "", 0.2, 4.)
                    .map(|v| theme.appearance.swipe_duration = v)
                    .is_some(),
                "--blyrics-swipe-lead-ratio" => number(value, "", 0., 0.5)
                    .map(|v| theme.appearance.swipe_lead = v)
                    .is_some(),
                "--blyrics-animate-highlight-glow"
                | "--blyrics-animate-highlight-swipe"
                | "--blyrics-animate-letter-wave"
                | "--blyrics-animate-word-wobble"
                | "--blyrics-animate-highlight-fade" => match value {
                    "0" | "1" | "true" | "false" => {
                        let on = value == "1" || value == "true";
                        match name {
                            "--blyrics-animate-highlight-glow" => theme.appearance.glow = on,
                            "--blyrics-animate-highlight-swipe" => theme.appearance.swipe = on,
                            "--blyrics-animate-letter-wave" => theme.appearance.wave = on,
                            "--blyrics-animate-word-wobble" => theme.appearance.wobble = on,
                            _ => theme.appearance.fade = on,
                        };
                        true
                    }
                    _ => false,
                },
                _ => false,
            };
            if !supported {
                theme
                    .unsupported
                    .push(format!("{selector}: {name}: {value}"));
            }
        }
    }
    if theme.unsupported.len() > 100 {
        theme.unsupported.truncate(100);
    }
    theme
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normal_fill_is_light_with_whole_word_motion_and_optional_letter_waves() {
        let default = NativeTheme::default();
        assert!(!default.appearance.wave && default.appearance.wobble);
        assert_eq!(default.appearance.glow_radius, 6.);
        assert_eq!(default.style().glow_color.alpha(), 0x50);
        let enabled=convert(":root{--blyrics-animate-letter-wave:1;--blyrics-animate-word-wobble:1;--blyrics-highlight-glow-radius-from:12.8px;}");
        assert!(enabled.appearance.wave && enabled.appearance.wobble);
        assert_eq!(enabled.appearance.glow_radius, 12.8);
        let custom=convert(":root{--blyrics-wobble-duration:700ms;--vey-word-wobble-strength:0.6;--blyrics-animate-word-wobble:0;}");
        assert!(custom.unsupported.is_empty());
        assert_eq!(custom.appearance.wobble_duration, 700.);
        assert_eq!(custom.appearance.wobble_strength, 0.6);
        assert!(!custom.appearance.wobble);
    }
    #[test]
    fn custom_fonts_effects_and_persistent_theme_round_trip() {
        let css=":root {font-family:'Roboto',sans-serif;--blyrics-font-weight:500;--blyrics-font-size:36px;--blyrics-glow-color:#55aaffcc;--blyrics-highlight-glow-radius-from:18px;--blyrics-highlight-glow-min-duration:1.5s;--blyrics-animate-letter-wave:0;--blyrics-swipe-duration-ratio:1.2;--blyrics-line-height:1.5;}";
        let settings = crate::services::Settings {
            lyrics_theme_css: css.into(),
            ..Default::default()
        };
        let restored: crate::services::Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        let theme = from_settings(&restored);
        assert!(theme.unsupported.is_empty(), "{:?}", theme.unsupported);
        assert_eq!(theme.appearance.family, "Roboto");
        assert_eq!(theme.size, 36.);
        assert_eq!(theme.weight, 500);
        assert_eq!(theme.appearance.glow_radius, 18.);
        assert_eq!(theme.appearance.glow_duration, 1500.);
        assert!(!theme.appearance.wave);
        assert_eq!(theme.appearance.swipe_duration, 1.2);
        assert_eq!(theme.appearance.line_spacing, 1.5);
        assert_eq!(
            from_settings(&crate::services::Settings {
                lyrics_font_family: "Noto Sans".into(),
                ..restored
            })
            .appearance
            .family,
            "Noto Sans"
        );
    }
    #[test]
    fn invalid_effect_values_are_reported_without_invalidating_defaults() {
        let theme=convert(":root {--blyrics-highlight-glow-radius-from:NaNpx;--blyrics-line-height:-10;--blyrics-animate-highlight-glow:maybe;}");
        assert_eq!(theme.unsupported.len(), 3);
        assert_eq!(theme.appearance.glow_radius, 6.);
        assert_eq!(theme.appearance.line_spacing, 1.333);
        assert!(theme.appearance.glow);
    }
    #[test]
    fn converts_and_reports_css() {
        let t=convert(":root {--blyrics-active-color:#f97316; --blyrics-font-size:32px} .blyrics-line {filter: blur(2px)} body {background:red}");
        assert_eq!(t.active, 0xfff97316);
        assert_eq!(t.size, 32.);
        assert_eq!(t.unsupported.len(), 2);
    }
    #[test]
    fn css_timing_variables_and_comment_settings_are_preserved() {
        let t = convert("/* blyrics-disable-richsync=true; blyrics-passive-scroll-enabled=true; blyrics-passive-scroll-seconds-per-line=4; blyrics-early-scroll-consider-s=0.1; blyrics-lyric-ending-threshold-s=0.25; */ :host {--blyrics-line-scroll-duration:250ms;--blyrics-scroll-timing-offset:0.5s;--blyrics-richsync-timing-offset:0.15s;--blyrics-timing-offset:-100ms;}");
        assert!(t.unsupported.is_empty(), "{:?}", t.unsupported);
        assert!(t.settings.disable_richsync && t.settings.passive_scroll);
        assert_eq!(t.settings.passive_seconds_per_line, 4.);
        assert_eq!(t.settings.scroll_duration_ms, 250);
        assert_eq!(t.settings.richsync_offset_ms, 150);
        assert_eq!(t.settings.line_offset_ms, -100);
        assert_eq!(t.settings.scroll_offset_ms, 500);
        assert_eq!(t.settings.lookahead_ms, 100);
        assert_eq!(t.settings.ending_threshold_ms, 250);
    }
}
