//! CSS-to-native conversion. Unsupported rules are returned for user review.
use serde::{Deserialize, Serialize};
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
    pub settings: crate::braccato::settings::ThemeSettings,
}
impl Default for NativeTheme {
    fn default() -> Self {
        Self {
            source: String::new(),
            active: 0xfff5f5f5,
            inactive: 0xff888888,
            size: 28.,
            weight: 700,
            unsupported: vec![],
            settings: Default::default(),
        }
    }
}
impl NativeTheme {
    /// Apply the same supported theme behavior on desktop and WASM.
    pub fn apply(&self, w: &crate::MainWindow, reduced: bool, passive: bool) {
        w.set_lyric_active(slint::Color::from_argb_encoded(self.active));
        w.set_lyric_inactive(slint::Color::from_argb_encoded(self.inactive));
        w.set_lyric_size(self.size);
        w.set_lyric_weight(self.weight);
        w.set_lyrics_scroll_ratio(self.settings.target_ratio);
        w.set_lyrics_lookahead_ms(self.settings.lookahead_ms.clamp(0, i32::MAX as i64) as i32);
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
                "--blyrics-active-color"
                | "--blyrics-highlight-color"
                | "--blyrics-lyric-active-color" => {
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
                "--blyrics-timing-offset" => crate::braccato::settings::milliseconds(value)
                    .map(|n| theme.settings.line_offset_ms = n)
                    .is_some(),
                "font-weight" => value
                    .parse::<i32>()
                    .ok()
                    .filter(|v| *v >= 100 && *v <= 900)
                    .map(|v| theme.weight = v)
                    .is_some(),
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
    fn converts_and_reports_css() {
        let t=convert(":root {--blyrics-active-color:#f97316; --blyrics-font-size:32px} .blyrics-line {filter: blur(2px)} body {background:red}");
        assert_eq!(t.active, 0xfff97316);
        assert_eq!(t.size, 32.);
        assert_eq!(t.unsupported.len(), 2);
    }
    #[test]
    fn css_timing_variables_and_comment_settings_are_preserved() {
        let t = convert("/* blyrics-disable-richsync=true; blyrics-passive-scroll-enabled=true; blyrics-passive-scroll-seconds-per-line=4; */ :host {--blyrics-line-scroll-duration:250ms;--blyrics-richsync-timing-offset:0.15s;--blyrics-timing-offset:-100ms;}");
        assert!(t.unsupported.is_empty(), "{:?}", t.unsupported);
        assert!(t.settings.disable_richsync && t.settings.passive_scroll);
        assert_eq!(t.settings.passive_seconds_per_line, 4.);
        assert_eq!(t.settings.scroll_duration_ms, 250);
        assert_eq!(t.settings.richsync_offset_ms, 150);
        assert_eq!(t.settings.line_offset_ms, -100);
    }
}
