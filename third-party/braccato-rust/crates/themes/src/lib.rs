//! Rust-native theme values and a small compiler for upstream RICS theme declarations.
//!
//! RICS comments and `--blyrics-*` custom properties are parsed as data. No stylesheet is loaded
//! by the renderer: supported values become typed Rust settings consumed by the Slint view.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WobbleTransform {
    pub x_em: f32,
    pub y_em: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

impl Default for WobbleTransform {
    fn default() -> Self {
        Self {
            x_em: 0.0,
            y_em: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LyricsTheme {
    pub background: String,
    pub foreground: String,
    pub muted: String,
    pub accent: String,
    pub glow_color: Option<String>,
    pub font_family: String,
    pub font_size: f32,
    pub font_weight: u16,
    pub line_spacing: f32,
    pub active_scale: f32,
    pub inactive_scale: f32,
    pub padding_px: f32,
    pub target_scroll_pos_ratio: f32,
    pub transition_ms: u32,
    pub line_scale_duration_ms: f32,
    pub long_word_threshold_ms: f32,
    pub word_wobble: bool,
    pub wobble_duration_ms: f32,
    pub wobble_peak_offset: f32,
    pub wobble_settle_offset: f32,
    pub wobble_easing: String,
    pub wobble_peak_easing: String,
    pub wobble_end_easing: String,
    pub wobble_from: WobbleTransform,
    pub wobble_peak: WobbleTransform,
    pub wobble_settle: WobbleTransform,
    pub wobble_to: WobbleTransform,
    pub letter_wave_y_em: f32,
    pub letter_wave_settle_y_em: f32,
    pub letter_wave: bool,
    pub letter_wave_emphasis_scale: f32,
    pub letter_wave_duration_ms: f32,
    pub highlight_swipe: bool,
    pub highlight_glow: bool,
    pub highlight_fade: bool,
    pub highlight_swipe_easing: String,
    pub highlight_glow_easing: String,
    pub highlight_fade_in_ms: f32,
    pub highlight_fade_out_ms: f32,
    pub highlight_swipe_start_from: f32,
    pub highlight_swipe_end_from: f32,
    pub highlight_swipe_start_to: f32,
    pub highlight_swipe_end_to: f32,
    pub highlight_glow_radius_from_px: f32,
    pub highlight_glow_radius_to_px: f32,
    pub highlight_glow_duration_ratio: f32,
    pub highlight_glow_min_duration_ms: f32,
    pub swipe_lead_ratio: f32,
    pub swipe_duration_ratio: f32,
    pub letter_wave_rise_easing: String,
    pub letter_wave_fall_easing: String,
    pub animate_line_scale: bool,
    pub animate_scroll: bool,
    pub animate_instrumental: bool,
    pub animate_decoration_entry: bool,
    pub sustain_blur: bool,
    pub lucid_focus: bool,
    pub background_font_scale: f32,
    pub scroll_duration_ms: Option<f32>,
    pub scroll_bezier: Option<[f32; 4]>,
    /// Native soft focus for neighboring source lines; zero disables it.
    pub inactive_blur_px: f32,
    /// Native curved sweep edge, measured in em; zero keeps a straight feather.
    pub swipe_wave_em: f32,
    /// All upstream `blyrics-*` values, including settings not yet mapped to a native visual.
    pub settings: BTreeMap<String, String>,
}

impl Default for LyricsTheme {
    fn default() -> Self {
        Self {
            background: "#101014".into(),
            foreground: "#f7f6fa".into(),
            muted: "#777780".into(),
            accent: "#f50032".into(),
            glow_color: None,
            font_family: "Satoshi".into(),
            font_size: 34.0,
            font_weight: 700,
            line_spacing: 1.35,
            active_scale: 1.0,
            inactive_scale: 0.95,
            padding_px: 32.0,
            target_scroll_pos_ratio: 0.42,
            transition_ms: 320,
            line_scale_duration_ms: 166.0,
            long_word_threshold_ms: 1500.0,
            word_wobble: true,
            wobble_duration_ms: 1000.0,
            wobble_peak_offset: 0.125,
            wobble_settle_offset: 0.75,
            wobble_easing: "ease".into(),
            wobble_peak_easing: "ease-in-out".into(),
            wobble_end_easing: "ease-out".into(),
            wobble_from: WobbleTransform::default(),
            wobble_peak: WobbleTransform {
                x_em: 0.05,
                y_em: 0.0,
                scale_x: 1.025,
                scale_y: 1.0,
            },
            wobble_settle: WobbleTransform::default(),
            wobble_to: WobbleTransform::default(),
            letter_wave_y_em: -0.05,
            letter_wave_settle_y_em: -0.02,
            letter_wave: true,
            letter_wave_emphasis_scale: 1.11,
            letter_wave_duration_ms: 900.0,
            highlight_swipe: true,
            highlight_glow: true,
            highlight_fade: true,
            highlight_swipe_easing: "linear".into(),
            highlight_glow_easing: "ease".into(),
            highlight_fade_in_ms: 330.0,
            highlight_fade_out_ms: 500.0,
            highlight_swipe_start_from: -0.2,
            highlight_swipe_end_from: -0.1,
            highlight_swipe_start_to: 1.4,
            highlight_swipe_end_to: 1.5,
            highlight_glow_radius_from_px: 12.8,
            highlight_glow_radius_to_px: 0.0,
            highlight_glow_duration_ratio: 1.2,
            highlight_glow_min_duration_ms: 1200.0,
            swipe_lead_ratio: 0.1,
            swipe_duration_ratio: 1.6,
            letter_wave_rise_easing: "ease-in-out".into(),
            letter_wave_fall_easing: "ease-out".into(),
            animate_line_scale: true,
            animate_scroll: true,
            animate_instrumental: true,
            animate_decoration_entry: true,
            sustain_blur: false,
            lucid_focus: false,
            background_font_scale: 0.72,
            scroll_duration_ms: None,
            scroll_bezier: None,
            inactive_blur_px: 0.0,
            swipe_wave_em: 0.0,
            settings: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ThemeError {
    #[error("invalid theme declaration on line {0}")]
    InvalidLine(usize),
    #[error("invalid value for {0}")]
    InvalidValue(String),
}

/// Compile native RICS values and the upstream theme subset into Slint-ready settings.
///
/// RICS comments (`blyrics-name = value`) and root `--blyrics-name` declarations are both read.
/// Declarations in other CSS selectors are ignored because the native component has no CSS cascade.
pub fn compile_rics(source: &str) -> Result<LyricsTheme, ThemeError> {
    let mut theme = LyricsTheme::default();
    let mut declarations = Vec::<(usize, String, String)>::new();
    let mut cleaned = String::with_capacity(source.len());
    let mut comments = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '*') {
            chars.next();
            let start = index + 2;
            let mut end = source.len();
            while let Some((at, current)) = chars.next() {
                if current == '*' && chars.peek().is_some_and(|(_, next)| *next == '/') {
                    end = at;
                    chars.next();
                    break;
                }
            }
            comments.push(&source[start..end]);
            cleaned.push(' ');
        } else {
            cleaned.push(ch);
        }
    }
    for comment in comments {
        for item in comment.split(';') {
            if let Some((key, value)) = item.split_once('=') {
                let key = key.trim();
                if key.starts_with("blyrics-") || key.starts_with("--blyrics-") {
                    declarations.push((
                        line_at(source, comment),
                        key.to_owned(),
                        value.trim().to_owned(),
                    ));
                }
            }
        }
    }

    for block in cleaned.split('}') {
        let (raw_selector, body) = block.split_once('{').unwrap_or(("", block));
        // Native files may put standalone declarations before their first selector.
        // Keep that preamble instead of discarding it as part of the selector.
        let preamble = raw_selector
            .rfind(';')
            .map(|end| &raw_selector[..=end])
            .unwrap_or("");
        let preamble = if preamble.trim_start().starts_with('@') {
            ""
        } else {
            preamble
        };
        for item in preamble.split(';').filter(|item| !item.trim().is_empty()) {
            if item.trim().starts_with('@') {
                continue;
            }
            let (key, value) = item
                .trim()
                .split_once(':')
                .ok_or_else(|| ThemeError::InvalidLine(line_at(source, item.trim())))?;
            declarations.push((
                line_at(source, item.trim()),
                key.trim().to_owned(),
                value.trim().to_owned(),
            ));
        }
        let selector = raw_selector
            .rsplit(';')
            .next()
            .unwrap_or(raw_selector)
            .trim();
        if selector.starts_with('@')
            || (!selector.is_empty() && selector != ":root" && selector != ".blyrics-container")
        {
            continue;
        }
        for item in body.split(';') {
            let declaration = item.trim();
            if declaration.is_empty() {
                continue;
            }
            let Some((key, value)) = declaration.split_once(':') else {
                return Err(ThemeError::InvalidLine(line_at(source, declaration)));
            };
            declarations.push((
                line_at(source, declaration),
                key.trim().to_owned(),
                value.trim().to_owned(),
            ));
        }
    }

    for (line, raw_key, raw_value) in declarations {
        let key = raw_key.trim_start_matches('-');
        let setting = key.strip_prefix("blyrics-").unwrap_or(key);
        let value = raw_value
            .trim()
            .trim_matches(';')
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if raw_key.starts_with("--") && !raw_key.starts_with("--blyrics-") {
            continue;
        }
        if raw_key.starts_with("--blyrics-") || raw_key.starts_with("blyrics-") {
            theme.settings.insert(setting.to_owned(), value.to_owned());
        }
        apply_setting(&mut theme, setting, value).map_err(|_| {
            if raw_key.starts_with("--blyrics-") || raw_key.starts_with("blyrics-") {
                ThemeError::InvalidValue(format!("{setting} on line {line}"))
            } else {
                ThemeError::InvalidValue(format!("{raw_key} on line {line}"))
            }
        })?;
    }
    validate(&theme)?;
    Ok(theme)
}

fn line_at(source: &str, fragment: &str) -> usize {
    source.find(fragment).map_or(1, |index| {
        source[..index].bytes().filter(|b| *b == b'\n').count() + 1
    })
}

fn apply_setting(theme: &mut LyricsTheme, key: &str, value: &str) -> Result<(), ()> {
    let number = || parse_number(key, value).map_err(|_| ());
    match key {
        "background" | "background-color" => theme.background = parse_color(value).ok_or(())?,
        "foreground" | "text" | "color" => theme.foreground = parse_color(value).ok_or(())?,
        "lyric-active-color" => {
            if let Some(color) = parse_color(value) {
                theme.foreground = color;
            } else if !value.contains("var(") {
                return Err(());
            }
        }
        "muted" | "secondary" => theme.muted = parse_color(value).ok_or(())?,
        "lyric-inactive-color" => {
            if let Some(color) = parse_color(value) {
                theme.muted = color;
            } else if !value.contains("var(") {
                return Err(());
            }
        }
        "accent" => theme.accent = parse_color(value).ok_or(())?,
        "highlight-color" => {
            if let Some(color) = parse_color(value) {
                theme.accent = color;
            }
        }
        "glow-color" => theme.glow_color = Some(parse_color(value).ok_or(())?),
        "font-family" => {
            theme.font_family = value
                .split(',')
                .next()
                .unwrap_or(value)
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_owned()
        }
        "font-size" => {
            theme.font_size = parse_length(value)
                .or_else(|| parse_css_clamp(value))
                .ok_or(())?
        }
        "font-weight" => theme.font_weight = number()?.round() as u16,
        "line-spacing" | "line-height" => theme.line_spacing = number()?,
        "active-scale" => theme.active_scale = number()?,
        "scale" => theme.inactive_scale = number()?,
        "padding" => theme.padding_px = parse_length(value).ok_or(())?,
        "target-scroll-pos-ratio" => theme.target_scroll_pos_ratio = number()?,
        "transition-ms" | "animation-duration" => theme.transition_ms = number()?.max(0.0) as u32,
        "scale-transition-duration" => {
            theme.line_scale_duration_ms = parse_duration(value).ok_or(())?
        }
        "long-word-threshold" => theme.long_word_threshold_ms = parse_duration(value).ok_or(())?,
        "animate-word-wobble" => theme.word_wobble = parse_bool(value).ok_or(())?,
        "wobble-duration" => theme.wobble_duration_ms = parse_duration(value).ok_or(())?,
        "word-wobble-transform-from" => {
            theme.wobble_from = parse_transform(value, theme.font_size).ok_or(())?
        }
        "word-wobble-transform-peak" => {
            theme.wobble_peak = parse_transform(value, theme.font_size).ok_or(())?
        }
        "word-wobble-transform-settle" => {
            theme.wobble_settle = parse_transform(value, theme.font_size).ok_or(())?
        }
        "word-wobble-transform-to" => {
            theme.wobble_to = parse_transform(value, theme.font_size).ok_or(())?
        }
        "word-wobble-peak-offset" => theme.wobble_peak_offset = number()?,
        "word-wobble-settle-offset" => theme.wobble_settle_offset = number()?,
        "word-wobble-easing" => theme.wobble_easing = parse_easing(value).ok_or(())?.into(),
        "word-wobble-peak-easing" => {
            theme.wobble_peak_easing = parse_easing(value).ok_or(())?.into()
        }
        "word-wobble-end-easing" => theme.wobble_end_easing = parse_easing(value).ok_or(())?.into(),
        "letter-wave-rise-easing" => {
            theme.letter_wave_rise_easing = parse_easing(value).ok_or(())?.into()
        }
        "letter-wave-fall-easing" => {
            theme.letter_wave_fall_easing = parse_easing(value).ok_or(())?.into()
        }
        "letter-wave-transform" => {
            theme.letter_wave_y_em = parse_translate_y(value, theme.font_size).ok_or(())?
        }
        "letter-wave-settle" => {
            theme.letter_wave_settle_y_em = parse_translate_y(value, theme.font_size).ok_or(())?
        }
        "letter-wave" => theme.letter_wave = parse_bool(value).ok_or(())?,
        "letter-wave-emphasis-scale" => theme.letter_wave_emphasis_scale = number()?,
        "letter-wave-duration" => theme.letter_wave_duration_ms = parse_duration(value).ok_or(())?,
        "animate-highlight-swipe" => theme.highlight_swipe = parse_bool(value).ok_or(())?,
        "animate-highlight-glow" => theme.highlight_glow = parse_bool(value).ok_or(())?,
        "animate-highlight-fade" => theme.highlight_fade = parse_bool(value).ok_or(())?,
        "highlight-swipe-easing" => {
            theme.highlight_swipe_easing = parse_easing(value).ok_or(())?.into()
        }
        "highlight-glow-easing" => {
            theme.highlight_glow_easing = parse_easing(value).ok_or(())?.into()
        }
        "lyric-highlight-fade-in-duration" => {
            theme.highlight_fade_in_ms = parse_duration(value).ok_or(())?
        }
        "lyric-highlight-fade-out-duration" => {
            theme.highlight_fade_out_ms = parse_duration(value).ok_or(())?
        }
        "animate-line-scale" => theme.animate_line_scale = parse_bool(value).ok_or(())?,
        "animate-scroll" => theme.animate_scroll = parse_bool(value).ok_or(())?,
        "animate-instrumental" => theme.animate_instrumental = parse_bool(value).ok_or(())?,
        "animate-decoration-entry" => {
            theme.animate_decoration_entry = parse_bool(value).ok_or(())?
        }
        "sustain-blur" => theme.sustain_blur = parse_bool(value).ok_or(())?,
        "lucid-focus" => theme.lucid_focus = parse_bool(value).ok_or(())?,
        "background-font-scale" => theme.background_font_scale = number()?,
        "lyric-scroll-duration" => {
            theme.scroll_duration_ms = Some(parse_duration(value).ok_or(())?)
        }
        "lyric-scroll-timing-function" => {
            theme.scroll_bezier = Some(parse_bezier(value).ok_or(())?)
        }
        "inactive-blur" => theme.inactive_blur_px = parse_length(value).ok_or(())?,
        "swipe-wave" => theme.swipe_wave_em = number()?,
        "highlight-swipe-start-from" => theme.highlight_swipe_start_from = number()?,
        "highlight-swipe-end-from" => theme.highlight_swipe_end_from = number()?,
        "highlight-swipe-start-to" => theme.highlight_swipe_start_to = number()?,
        "highlight-swipe-end-to" => theme.highlight_swipe_end_to = number()?,
        "highlight-glow-radius-from" => {
            theme.highlight_glow_radius_from_px = parse_length(value).ok_or(())?
        }
        "highlight-glow-radius-to" => {
            theme.highlight_glow_radius_to_px = parse_length(value).ok_or(())?
        }
        "highlight-glow-duration-ratio" => theme.highlight_glow_duration_ratio = number()?,
        "highlight-glow-min-duration" => {
            theme.highlight_glow_min_duration_ms = parse_duration(value).ok_or(())?
        }
        "swipe-lead-ratio" => theme.swipe_lead_ratio = number()?,
        "swipe-duration-ratio" => theme.swipe_duration_ratio = number()?,
        // Valid upstream values without a direct Slint counterpart remain inspectable in settings.
        _ if theme.settings.contains_key(key) => {}
        // Regular CSS colors/spacing are accepted only where the native model has a defined mapping.
        _ => return Err(()),
    }
    Ok(())
}

fn validate(theme: &LyricsTheme) -> Result<(), ThemeError> {
    if theme
        .scroll_duration_ms
        .is_some_and(|value| !value.is_finite() || !(0.0..=10000.0).contains(&value))
    {
        return Err(ThemeError::InvalidValue("lyric-scroll-duration".into()));
    }
    for (key, value, range) in [
        ("font-size", theme.font_size, 8.0..=120.0),
        ("line-spacing", theme.line_spacing, 0.5..=4.0),
        ("active-scale", theme.active_scale, 0.5..=2.0),
        ("scale", theme.inactive_scale, 0.5..=2.0),
        ("padding", theme.padding_px, 0.0..=256.0),
        (
            "target-scroll-pos-ratio",
            theme.target_scroll_pos_ratio,
            0.0..=1.0,
        ),
        ("wobble-duration", theme.wobble_duration_ms, 0.0..=10000.0),
        (
            "word-wobble-peak-offset",
            theme.wobble_peak_offset,
            0.0..=1.0,
        ),
        (
            "word-wobble-settle-offset",
            theme.wobble_settle_offset,
            0.0..=1.0,
        ),
        (
            "letter-wave-duration",
            theme.letter_wave_duration_ms,
            0.0..=10000.0,
        ),
        (
            "letter-wave-emphasis-scale",
            theme.letter_wave_emphasis_scale,
            0.5..=2.0,
        ),
        ("letter-wave-transform", theme.letter_wave_y_em, -2.0..=2.0),
        (
            "letter-wave-settle",
            theme.letter_wave_settle_y_em,
            -2.0..=2.0,
        ),
        (
            "highlight-glow-duration-ratio",
            theme.highlight_glow_duration_ratio,
            0.0..=10.0,
        ),
        (
            "highlight-glow-min-duration",
            theme.highlight_glow_min_duration_ms,
            0.0..=10000.0,
        ),
        (
            "highlight-glow-radius-from",
            theme.highlight_glow_radius_from_px,
            0.0..=128.0,
        ),
        (
            "highlight-glow-radius-to",
            theme.highlight_glow_radius_to_px,
            0.0..=128.0,
        ),
        (
            "lyric-highlight-fade-in-duration",
            theme.highlight_fade_in_ms,
            0.0..=10000.0,
        ),
        (
            "lyric-highlight-fade-out-duration",
            theme.highlight_fade_out_ms,
            0.0..=10000.0,
        ),
        ("swipe-lead-ratio", theme.swipe_lead_ratio, 0.0..=2.0),
        ("inactive-blur", theme.inactive_blur_px, 0.0..=12.0),
        (
            "background-font-scale",
            theme.background_font_scale,
            0.2..=1.5,
        ),
        ("swipe-wave", theme.swipe_wave_em, 0.0..=0.15),
        (
            "swipe-duration-ratio",
            theme.swipe_duration_ratio,
            0.05..=10.0,
        ),
        (
            "scale-transition-duration",
            theme.line_scale_duration_ms,
            0.0..=10000.0,
        ),
        (
            "long-word-threshold",
            theme.long_word_threshold_ms,
            0.0..=30000.0,
        ),
        (
            "highlight-swipe-start-from",
            theme.highlight_swipe_start_from,
            -2.0..=2.0,
        ),
        (
            "highlight-swipe-end-from",
            theme.highlight_swipe_end_from,
            -2.0..=2.0,
        ),
        (
            "highlight-swipe-start-to",
            theme.highlight_swipe_start_to,
            -2.0..=3.0,
        ),
        (
            "highlight-swipe-end-to",
            theme.highlight_swipe_end_to,
            -2.0..=3.0,
        ),
    ] {
        if !value.is_finite() || !range.contains(&value) {
            return Err(ThemeError::InvalidValue(key.into()));
        }
    }
    if theme.wobble_settle_offset < theme.wobble_peak_offset {
        return Err(ThemeError::InvalidValue(
            "word-wobble-settle-offset must follow peak".into(),
        ));
    }
    if !(theme.highlight_swipe_start_to > theme.highlight_swipe_start_from
        && theme.highlight_swipe_end_to > theme.highlight_swipe_end_from)
    {
        return Err(ThemeError::InvalidValue(
            "highlight-swipe range must move forward".into(),
        ));
    }
    for frame in [
        theme.wobble_from,
        theme.wobble_peak,
        theme.wobble_settle,
        theme.wobble_to,
    ] {
        if !frame.x_em.is_finite()
            || !frame.y_em.is_finite()
            || !frame.scale_x.is_finite()
            || !frame.scale_y.is_finite()
            || !(-2.0..=2.0).contains(&frame.x_em)
            || !(-2.0..=2.0).contains(&frame.y_em)
            || !(0.5..=2.0).contains(&frame.scale_x)
            || !(0.5..=2.0).contains(&frame.scale_y)
        {
            return Err(ThemeError::InvalidValue("word-wobble-transform".into()));
        }
    }
    if !(100..=900).contains(&theme.font_weight) {
        return Err(ThemeError::InvalidValue("font-weight (100–900)".into()));
    }
    if theme.font_family.trim().is_empty() {
        return Err(ThemeError::InvalidValue("font-family".into()));
    }
    if theme.transition_ms > 10000 {
        return Err(ThemeError::InvalidValue("transition-ms (0–10000)".into()));
    }
    for (key, value) in [
        ("background", &theme.background),
        ("foreground", &theme.foreground),
        ("muted", &theme.muted),
        ("accent", &theme.accent),
    ] {
        if !(value.len() == 7 || value.len() == 9)
            || !value.starts_with('#')
            || !value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(ThemeError::InvalidValue(format!(
                "{key}: use #RRGGBB or #RRGGBBAA"
            )));
        }
    }
    Ok(())
}

fn parse_number(key: &str, value: &str) -> Result<f32, ThemeError> {
    let number = value
        .trim_end_matches("px")
        .trim_end_matches("ms")
        .trim()
        .parse::<f32>()
        .map_err(|_| ThemeError::InvalidValue(key.into()))?;
    if !number.is_finite() {
        return Err(ThemeError::InvalidValue(key.into()));
    }
    Ok(number)
}
fn parse_length(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse::<f32>().ok();
    }
    if let Some(rem) = value.strip_suffix("rem") {
        return rem.trim().parse::<f32>().ok().map(|v| v * 16.0);
    }
    if let Some(em) = value.strip_suffix("em") {
        return em.trim().parse::<f32>().ok().map(|v| v * 16.0);
    }
    value.parse::<f32>().ok()
}
fn parse_css_clamp(value: &str) -> Option<f32> {
    let arguments = value
        .trim()
        .strip_prefix("clamp(")?
        .strip_suffix(')')?
        .split(',')
        .collect::<Vec<_>>();
    arguments
        .last()
        .and_then(|value| parse_length(value.trim()))
}
fn parse_duration(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(milliseconds) = value.strip_suffix("ms") {
        milliseconds.trim().parse().ok()
    } else if let Some(seconds) = value.strip_suffix('s') {
        seconds.trim().parse::<f32>().ok().map(|v| v * 1000.0)
    } else {
        value.parse().ok()
    }
}
fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}
fn parse_easing(value: &str) -> Option<&str> {
    ["linear", "ease", "ease-in", "ease-out", "ease-in-out"]
        .into_iter()
        .find(|item| *item == value.trim())
}

fn parse_bezier(value: &str) -> Option<[f32; 4]> {
    let named = match value.trim() {
        "linear" => Some([0.0, 0.0, 1.0, 1.0]),
        "ease" => Some([0.25, 0.1, 0.25, 1.0]),
        "ease-in" => Some([0.42, 0.0, 1.0, 1.0]),
        "ease-out" => Some([0.0, 0.0, 0.58, 1.0]),
        "ease-in-out" => Some([0.42, 0.0, 0.58, 1.0]),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let values = value
        .trim()
        .strip_prefix("cubic-bezier(")?
        .strip_suffix(')')?
        .split(',')
        .map(|v| v.trim().parse::<f32>().ok())
        .collect::<Option<Vec<_>>>()?;
    let points: [f32; 4] = values.try_into().ok()?;
    // Slint 1.18 requires literal easing curves. Keep an explicit supported set
    // rather than silently accepting a custom curve that cannot be rendered.
    [
        [0.0, 0.0, 1.0, 1.0],
        [0.25, 0.1, 0.25, 1.0],
        [0.42, 0.0, 1.0, 1.0],
        [0.0, 0.0, 0.58, 1.0],
        [0.42, 0.0, 0.58, 1.0],
        [0.45, 0.0, 0.15, 1.0],
        [0.86, 0.0, 0.2, 1.0],
    ]
    .contains(&points)
    .then_some(points)
}
fn parse_transform(value: &str, font_size: f32) -> Option<WobbleTransform> {
    let mut transform = WobbleTransform::default();
    for token in value.split(')').map(str::trim).filter(|t| !t.is_empty()) {
        let (name, args) = token.split_once('(')?;
        let values = args.split(',').map(str::trim).collect::<Vec<_>>();
        let measure = |v: &str| parse_em(v, font_size);
        match name.trim() {
            "translateX" => transform.x_em = measure(values.first()?)?,
            "translateY" => transform.y_em = measure(values.first()?)?,
            "translate" => {
                transform.x_em = measure(values.first()?)?;
                if let Some(y) = values.get(1) {
                    transform.y_em = measure(y)?;
                }
            }
            "scaleX" => transform.scale_x = values.first()?.parse().ok()?,
            "scaleY" => transform.scale_y = values.first()?.parse().ok()?,
            "scale" => {
                transform.scale_x = values.first()?.parse().ok()?;
                transform.scale_y = values.get(1).unwrap_or(values.first()?).parse().ok()?;
            }
            _ => return None,
        }
    }
    Some(transform)
}
fn parse_translate_y(value: &str, font_size: f32) -> Option<f32> {
    parse_transform(value, font_size).map(|transform| transform.y_em)
}
fn parse_em(value: &str, font_size: f32) -> Option<f32> {
    let value = value.trim();
    if let Some(em) = value.strip_suffix("em") {
        em.trim().parse().ok()
    } else if let Some(px) = value.strip_suffix("px") {
        px.trim()
            .parse::<f32>()
            .ok()
            .map(|v| v / font_size.max(1.0))
    } else {
        value.parse().ok()
    }
}

#[allow(clippy::excessive_precision)]
fn parse_color(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let expanded = match hex.len() {
            3 => format!(
                "#{}{}{}{}{}{}",
                &hex[0..1],
                &hex[0..1],
                &hex[1..2],
                &hex[1..2],
                &hex[2..3],
                &hex[2..3]
            ),
            4 => format!(
                "#{}{}{}{}{}{}{}{}",
                &hex[0..1],
                &hex[0..1],
                &hex[1..2],
                &hex[1..2],
                &hex[2..3],
                &hex[2..3],
                &hex[3..4],
                &hex[3..4]
            ),
            6 | 8 => value.to_owned(),
            _ => return None,
        };
        return Some(expanded.to_ascii_lowercase());
    }
    let body = value;
    let (rgb, a): ([f32; 3], f32) = if let Some(body) = body
        .strip_prefix("oklch(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let parts = body
            .replace('/', " ")
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if parts.len() < 3 {
            return None;
        }
        let l = parts[0].parse::<f32>().ok()?;
        let c = parts[1].parse::<f32>().ok()?;
        let h = parts[2]
            .trim_end_matches("deg")
            .parse::<f32>()
            .ok()?
            .to_radians();
        let a = c * h.cos();
        let b = c * h.sin();
        let ll = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
        let mm = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
        let ss = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
        (
            [
                4.0767416621 * ll - 3.3077115913 * mm + 0.2309699292 * ss,
                -1.2684380046 * ll + 2.6097574011 * mm - 0.3413193965 * ss,
                -0.0041960863 * ll - 0.7034186147 * mm + 1.7076147010 * ss,
            ],
            if parts.len() > 3 {
                parts[3].parse().unwrap_or(1.0)
            } else {
                1.0
            },
        )
    } else if let Some(body) = body
        .strip_prefix("color(display-p3 ")
        .and_then(|v| v.strip_suffix(')'))
    {
        let channels = body
            .replace('/', " ")
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if channels.len() < 3 {
            return None;
        }
        let p = [
            channels[0].parse::<f32>().ok()?,
            channels[1].parse::<f32>().ok()?,
            channels[2].parse::<f32>().ok()?,
        ]
        .map(decode_srgb);
        let x = 0.4865709486 * p[0] + 0.2656676932 * p[1] + 0.1982172852 * p[2];
        let y = 0.2289745641 * p[0] + 0.6917385218 * p[1] + 0.0792869141 * p[2];
        let z = 0.0 * p[0] + 0.0451133819 * p[1] + 1.0439443689 * p[2];
        (
            [
                3.2409699419 * x - 1.5373831776 * y - 0.4986107603 * z,
                -0.9692436363 * x + 1.8759675015 * y + 0.0415550574 * z,
                0.0556300797 * x - 0.2039769589 * y + 1.0569715142 * z,
            ],
            if channels.len() > 3 {
                channels[3].parse().unwrap_or(1.0)
            } else {
                1.0
            },
        )
    } else if let Some(body) = body
        .strip_prefix("rgb(")
        .or_else(|| body.strip_prefix("rgba("))
        .and_then(|v| v.strip_suffix(')'))
    {
        let values = body
            .replace([',', '/'], " ")
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if values.len() < 3 {
            return None;
        }
        let channel = |i: usize| {
            values[i]
                .trim_end_matches('%')
                .parse::<f32>()
                .ok()
                .map(|v| {
                    if values[i].ends_with('%') {
                        v / 100.0
                    } else {
                        v / 255.0
                    }
                })
        };
        (
            [
                decode_srgb(channel(0)?),
                decode_srgb(channel(1)?),
                decode_srgb(channel(2)?),
            ],
            if values.len() > 3 {
                values[3].parse().unwrap_or(1.0)
            } else {
                1.0
            },
        )
    } else {
        return None;
    };
    Some(format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        encode_srgb(rgb[0]),
        encode_srgb(rgb[1]),
        encode_srgb(rgb[2]),
        (a.clamp(0.0, 1.0) * 255.0).round() as u8
    ))
}
fn decode_srgb(v: f32) -> f32 {
    let v = v.max(0.0);
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn encode_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let e = if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_lucid_preset_preserves_lyric_controls_and_supports_scroll_curve() {
        let theme =
            super::compile_rics(include_str!("../../demo/assets/themes/lucid.rics")).unwrap();
        assert_eq!(theme.font_family, "Rosemary");
        assert_eq!(theme.font_size, 48.0);
        assert_eq!(theme.font_weight, 400);
        assert!(theme.lucid_focus);
        assert!(!theme.word_wobble);
        assert_eq!(theme.background_font_scale, 0.65);
        assert_eq!(theme.scroll_duration_ms, Some(800.0));
        assert_eq!(theme.scroll_bezier, Some([0.45, 0.0, 0.15, 1.0]));
        assert!(
            super::compile_rics("--blyrics-lyric-scroll-timing-function: cubic-bezier(2,0,1,1);")
                .is_err()
        );
        assert!(super::compile_rics("--blyrics-lyric-scroll-duration: 20000ms;").is_err());
    }
    #[test]
    fn native_focus_and_wave_settings_are_bounded_and_can_be_disabled() {
        let theme =
            super::compile_rics("--blyrics-inactive-blur: 1.5px; --blyrics-swipe-wave: 0.07;")
                .unwrap();
        assert_eq!(theme.inactive_blur_px, 1.5);
        assert_eq!(theme.swipe_wave_em, 0.07);
        let off =
            super::compile_rics("--blyrics-inactive-blur: 0px; --blyrics-swipe-wave: 0;").unwrap();
        assert_eq!(off.inactive_blur_px, 0.0);
        assert_eq!(off.swipe_wave_em, 0.0);
        assert!(super::compile_rics("--blyrics-inactive-blur: -1px;").is_err());
        assert!(super::compile_rics("--blyrics-swipe-wave: 0.2;").is_err());
    }
    use super::*;

    #[test]
    fn compiles_upstream_rics_variables_and_comments() {
        let t = compile_rics("/* blyrics-target-scroll-pos-ratio = 0.5; */\n.blyrics-container {\n --blyrics-font-size: 3rem;\n --blyrics-word-wobble-transform-from: translateY(0);\n --blyrics-word-wobble-transform-peak: translateX(0.05em) scaleX(1.025);\n --blyrics-word-wobble-transform-settle: translateY(-0.028em);\n --blyrics-word-wobble-transform-to: translateY(-0.0375em);\n --blyrics-word-wobble-peak-offset: 0.125;\n --blyrics-word-wobble-settle-offset: 0.75;\n --blyrics-wobble-duration: 1s;\n --blyrics-letter-wave-transform: translateY(-0.07em);\n --blyrics-letter-wave-duration: 0.9s;\n --blyrics-highlight-glow-radius-from: 0.8rem;\n --blyrics-line-height: 1.333;\n --blyrics-lyric-active-color: oklch(0.97 0.003 40);\n}").unwrap();
        assert_eq!(t.font_size, 48.0);
        assert_eq!(t.target_scroll_pos_ratio, 0.5);
        assert_eq!(t.wobble_duration_ms, 1000.0);
        assert_eq!(t.wobble_peak.x_em, 0.05);
        assert_eq!(t.wobble_peak.scale_x, 1.025);
        assert_eq!(t.wobble_settle.y_em, -0.028);
        assert_eq!(t.wobble_to.y_em, -0.0375);
        assert_eq!(t.letter_wave_y_em, -0.07);
        assert_eq!(t.highlight_glow_radius_from_px, 12.8);
        assert!(t.foreground.starts_with('#'));
        assert!(t.settings.contains_key("font-size"));
    }

    #[test]
    fn retains_unknown_upstream_settings_but_rejects_unsupported_css() {
        let t = compile_rics("/* blyrics-instrumental-wave-oscillation-duration = 1.25s; */\n.blyrics-container { --blyrics-instrumental-fill-fade-duration: 150ms; }").unwrap();
        assert_eq!(
            t.settings["instrumental-wave-oscillation-duration"],
            "1.25s"
        );
        assert_eq!(t.settings["instrumental-fill-fade-duration"], "150ms");
        assert!(compile_rics("filter: blur(2px);").is_err());
    }

    #[test]
    fn compiles_the_upstream_sustain_and_karaoke_theme_sources() {
        let sustain = compile_rics(include_str!("../../../demo/theme-sustain.css")).unwrap();
        assert_eq!(sustain.font_family, "DM Sans");
        assert_eq!(sustain.font_weight, 600);
        assert_eq!(sustain.wobble_duration_ms, 1000.0);
        assert_eq!(sustain.letter_wave_y_em, -0.07);
        assert_eq!(sustain.glow_color.as_deref(), Some("#ffffff00"));
        assert_eq!(sustain.settings["letter-wave-duration"], "0.9s");

        let karaoke = compile_rics(include_str!("../../../demo/theme-karaoke.css")).unwrap();
        assert!(!karaoke.word_wobble);
        assert!(!karaoke.highlight_swipe);
        assert!(!karaoke.letter_wave);
        assert!(!karaoke.animate_line_scale);
    }

    #[test]
    fn rejects_invalid_native_values_and_transform_order() {
        for source in [
            "font-size: -1px;",
            "font-weight: 1000;",
            "active-scale: -2;",
            "line-spacing: 0;",
            "accent: invalid;",
            "transition-ms: 999999999;",
            ".blyrics-container { --blyrics-word-wobble-peak-offset: 0.8; --blyrics-word-wobble-settle-offset: 0.2; }",
            ".blyrics-container { --blyrics-word-wobble-transform-peak: scaleX(NaN); }",
        ] {
            assert!(
                compile_rics(source).is_err(),
                "accepted invalid theme: {source}"
            );
        }
    }

    #[test]
    fn converts_upstream_alpha_colors_without_double_parsing() {
        let t = compile_rics(
            ".blyrics-container { --blyrics-lyric-inactive-color: oklch(0.97 0.003 40 / 0.22); }",
        )
        .unwrap();
        assert_eq!(t.muted.len(), 9);
        assert_eq!(&t.muted[7..], "38");
    }
}
