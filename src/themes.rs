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
    pub errors: Vec<String>,
    #[serde(default)]
    pub player: crate::player_themes::Overrides,
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
            errors: vec![],
            player: Default::default(),
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
pub(crate) fn color(raw: &str) -> Option<u32> {
    let lower = raw.trim().to_ascii_lowercase();
    let value = lower.as_str();
    if let Some(value) = crate::theme_colors::parse(value) {
        return Some(value);
    }
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
        if !(3..=4).contains(&parts.len()) {
            return None;
        }
        let channel = |s: &str| -> Option<u32> {
            if let Some(v) = s.strip_suffix('%') {
                Some(
                    (v.parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite())?
                        .clamp(0., 100.)
                        * 2.55)
                        .round() as u32,
                )
            } else {
                Some(
                    s.parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite())?
                        .clamp(0., 255.)
                        .round() as u32,
                )
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
            .unwrap_or(Some(1.))?;
        if !alpha.is_finite() {
            return None;
        }
        let alpha = alpha.clamp(0., 1.);
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
        3 | 4 => {
            let mut chars = raw.chars();
            let (r, g, b) = (
                chars.next()?.to_digit(16)?,
                chars.next()?.to_digit(16)?,
                chars.next()?.to_digit(16)?,
            );
            let a = chars.next().map(|c| c.to_digit(16)).unwrap_or(Some(15))?;
            Some(a * 17 << 24 | r * 17 << 16 | g * 17 << 8 | b * 17)
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
    crate::theme_presets::install(w, &settings.language, &settings.lyrics_theme_css, &theme);
    w.set_lyric_theme_source(settings.lyrics_theme_css.clone().into());
    w.set_lyric_theme_applied(settings.lyrics_theme_css.clone().into());
    w.set_lyric_font_family(settings.lyrics_font_family.clone().into());
    w.set_lyric_font_applied(settings.lyrics_font_family.clone().into());
    w.set_theme_entire_player(settings.theme_entire_player);
    w.set_theme_entire_player_draft(settings.theme_entire_player);
    w.set_player_theme_style(crate::player_themes::resolve(
        &theme,
        settings.theme_entire_player,
    ));
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
    crate::lyric_appearance::preview(w);
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
    theme.errors = compiled.errors;
    if !theme.errors.is_empty() {
        return theme;
    }
    let comments = regex::Regex::new(r"(?s)/\*.*?\*/").unwrap();
    let css = comments.replace_all(css, "");
    let rules = native_rules(&css, &mut theme.unsupported);
    let mut variables = std::collections::BTreeMap::new();
    for (selector, body) in &rules {
        if *selector == ":root" || *selector == ":host" || *selector == ".blyrics-container" {
            for (name, value) in declarations(body) {
                if name.starts_with("--") {
                    variables.insert(name.to_string(), value.to_string());
                }
            }
        }
    }
    for (selector, body) in rules {
        let player_selector = matches!(
            selector,
            "body"
                | "html"
                | "ytmusic-app"
                | "ytmusic-player-page"
                | "ytmusic-player-bar"
                | "ytmusic-nav-bar"
        );
        let valid = selector == ":root"
            || selector == ":host"
            || selector.contains("blyrics")
            || selector.contains("braccato")
            || player_selector;
        if !valid {
            theme.unsupported.push(selector.into());
            continue;
        }
        // Secondary and interactive rules cannot describe the main lyric ink.
        if ["translation", "romanization", ":hover", ":focus", "::"]
            .iter()
            .any(|s| selector.contains(s))
        {
            theme.unsupported.push(selector.into());
            continue;
        }
        for (name, raw) in declarations(body) {
            let resolved = resolve_value(raw, &variables, 0);
            let value = resolved.as_str();
            let supported = match name {
                "--blyrics-active-color" | "--blyrics-lyric-active-color" => {
                    color(value).map(|v| theme.active = v).is_some()
                }
                "--blyrics-inactive-color"
                | "--blyrics-color"
                | "--blyrics-lyric-inactive-color" => {
                    color(value).map(|v| theme.inactive = v).is_some()
                }
                "color" if player_selector => {
                    color(value).map(|v| theme.player.ink = Some(v)).is_some()
                }
                "background" | "background-color"
                    if player_selector || selector == ":root" || selector == ":host" =>
                {
                    color(value)
                        .map(|v| {
                            if selector.ends_with("-bar") {
                                theme.player.surface = Some(v)
                            } else {
                                theme.player.background = Some(v)
                            }
                        })
                        .is_some()
                }
                "--ytmusic-background" => color(value)
                    .map(|v| theme.player.background = Some(v))
                    .is_some(),
                "--ytmusic-player-bar-background" => color(value)
                    .map(|v| theme.player.surface = Some(v))
                    .is_some(),
                "font-size" if player_selector => false,
                "font-weight" if player_selector => false,
                "border-radius" if player_selector => number(value, "px", 0., 40.)
                    .map(|v| theme.player.corner = Some(v))
                    .is_some(),
                "color" => color(value)
                    .map(|v| {
                        if selector.contains("inactive") || !selector.contains("active") {
                            theme.inactive = v
                        } else {
                            theme.active = v
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
                        if player_selector {
                            theme.player.family = Some(family.into())
                        } else {
                            theme.appearance.family = family.into();
                        }
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
                "--vey-accent" => color(value)
                    .map(|v| theme.player.accent = Some(v))
                    .is_some(),
                "--vey-font-family" => {
                    let value = value.trim_matches(|c| c == '\'' || c == '"');
                    if !value.is_empty() && value.len() <= 128 {
                        theme.player.family = Some(value.into());
                        true
                    } else {
                        false
                    }
                }
                "--vey-background" => color(value)
                    .map(|v| theme.player.background = Some(v))
                    .is_some(),
                "--vey-surface" => color(value)
                    .map(|v| theme.player.surface = Some(v))
                    .is_some(),
                "--vey-text" => color(value).map(|v| theme.player.ink = Some(v)).is_some(),
                "--vey-muted" => color(value).map(|v| theme.player.muted = Some(v)).is_some(),
                "--vey-corner-radius" => number(value, "px", 0., 40.)
                    .map(|v| theme.player.corner = Some(v))
                    .is_some(),
                "--vey-artwork-radius" => number(value, "px", 0., 40.)
                    .map(|v| theme.player.artwork_corner = Some(v))
                    .is_some(),
                "--vey-control-radius" => number(value, "px", 0., 999.)
                    .map(|v| theme.player.control_corner = Some(v))
                    .is_some(),
                "--vey-artwork-share" => number(value, "", 0.32, 0.5)
                    .map(|v| theme.player.artwork_share = Some(v))
                    .is_some(),
                "--vey-ambient-strength" => number(value, "", 0., 1.5)
                    .map(|v| theme.player.ambient = Some(v))
                    .is_some(),
                "--vey-theme-duration" => crate::braccato::settings::milliseconds(value)
                    .filter(|v| *v >= 0 && *v <= 2000)
                    .map(|v| theme.player.motion_ms = Some(v as f32))
                    .is_some(),
                // Alias variables are retained for var() resolution above.
                _ => {
                    name.starts_with("--")
                        && variables.contains_key(name)
                        && !name.starts_with("--blyrics-")
                        && !name.starts_with("--vey-")
                }
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

fn declarations(body: &str) -> impl Iterator<Item = (&str, &str)> {
    body.split(';')
        .filter_map(|d| d.split_once(':'))
        .map(|(n, v)| {
            (
                n.trim(),
                v.trim()
                    .strip_suffix("!important")
                    .unwrap_or(v.trim())
                    .trim(),
            )
        })
}

// Keep conditional browser rules out of the unconditional native style. The
// previous leaf regex applied every @media branch to every window size.
fn native_rules<'a>(css: &'a str, unsupported: &mut Vec<String>) -> Vec<(&'a str, &'a str)> {
    let mut rules = vec![];
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let selector = rest[..open].trim();
        let mut depth = 1;
        let mut quote = '\0';
        let mut escaped = false;
        let mut end = None;
        for (offset, c) in rest[open + 1..].char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if c == '\\' {
                escaped = true;
                continue;
            }
            if quote != '\0' {
                if c == quote {
                    quote = '\0';
                }
                continue;
            }
            if c == '\'' || c == '"' {
                quote = c;
                continue;
            }
            if c == '{' {
                depth += 1;
            }
            if c == '}' {
                depth -= 1;
                if depth == 0 {
                    end = Some(open + 1 + offset);
                    break;
                }
            }
        }
        let Some(end) = end else {
            break;
        };
        if selector.starts_with('@') {
            unsupported.push(selector.into());
        } else {
            rules.push((selector, &rest[open + 1..end]));
        }
        rest = &rest[end + 1..];
    }
    rules
}

fn resolve_value(
    value: &str,
    vars: &std::collections::BTreeMap<String, String>,
    depth: u8,
) -> String {
    if depth > 16 {
        return value.into();
    }
    let Some(start) = value.find("var(") else {
        return value.into();
    };
    let mut level = 1;
    let mut end = None;
    for (i, c) in value[start + 4..].char_indices() {
        if c == '(' {
            level += 1;
        }
        if c == ')' {
            level -= 1;
            if level == 0 {
                end = Some(start + 4 + i);
                break;
            }
        }
    }
    let Some(end) = end else {
        return value.into();
    };
    let body = &value[start + 4..end];
    let (name, fallback) = body.split_once(',').unwrap_or((body, ""));
    let replacement = vars
        .get(name.trim())
        .map(String::as_str)
        .unwrap_or(fallback.trim());
    if replacement.is_empty() {
        return value.into();
    }
    resolve_value(
        &format!("{}{}{}", &value[..start], replacement, &value[end + 1..]),
        vars,
        depth + 1,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_player_css_roles_and_color_spaces_preserve_lyric_timing() {
        let t=convert("$accent:color(display-p3 1 0 0); :root{--vey-accent:$accent;--vey-background:#121216;--vey-corner-radius:12px;--vey-artwork-share:0.46;--vey-ambient-strength:0.5;--vey-theme-duration:0.6s;} ytmusic-player-bar{background-color:#24242a;} body{color:hsl(0 0% 90%);font-family:'Segoe UI';}");
        assert!(t.errors.is_empty() && t.unsupported.is_empty(), "{t:?}");
        assert_eq!(t.player.accent, Some(0xffff0000));
        assert_eq!(t.player.surface, Some(0xff24242a));
        assert_eq!(t.player.corner, Some(12.));
        assert_eq!(t.player.artwork_share, Some(0.46));
        assert_eq!(t.player.motion_ms, Some(600.));
        assert_eq!(t.player.family.as_deref(), Some("Segoe UI"));
        assert_eq!(
            serde_json::to_value(t.settings).unwrap(),
            serde_json::to_value(NativeTheme::default().settings).unwrap()
        );
        let old: crate::services::Settings = serde_json::from_str("{}").unwrap();
        assert!(old.theme_entire_player);
    }
    #[test]
    fn inactive_selectors_aliases_and_important_do_not_corrupt_active_ink() {
        let t = convert(":root{--accent:#8efc;--blyrics-active-color:var(--accent) !important;} .blyrics--inactive {color: rgb(25 50 75 / 40%);} .blyrics--active {color:var(--missing, #aabbcc);}");
        assert!(t.errors.is_empty() && t.unsupported.is_empty(), "{t:?}");
        assert_eq!(t.active, 0xffaabbcc);
        assert_eq!(t.inactive, 0x6619324b);
        assert_eq!(
            convert(":root{--accent:#8efc;--blyrics-active-color:var(--accent);}").active,
            0xcc88eeff
        );
        assert!(color("rgb(NaN 0 0)").is_none());
        assert!(color("rgba(0 0 0 / NaN)").is_none());
    }
    #[test]
    fn conditional_and_secondary_rules_do_not_override_main_typography() {
        let t=convert(":root {font-size:40px;} @media(max-width:600px){.blyrics-container {font-size:20px;}} .blyrics-translation {font-size:12px;} .blyrics--inactive{color:#345;}");
        assert!(t.errors.is_empty(), "{t:?}");
        assert_eq!(t.size, 40.);
        assert_eq!(t.inactive, 0xff334455);
        assert_eq!(t.unsupported.len(), 2);
        let t = convert("$broken: $undefined; :root {color:#fff;}");
        assert!(!t.errors.is_empty());
    }
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
        assert_eq!(t.unsupported.len(), 1);
        assert_eq!(t.player.background, Some(0xffff0000));
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
