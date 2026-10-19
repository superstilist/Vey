//! Upstream's comment-only theme configuration, per native view rather than global.
use regex::Regex;
use std::{collections::BTreeMap, sync::LazyLock};
static COMMENTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*(.*?)\*/").unwrap());
static SETTINGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(blyrics-[\w-]+)\s*=\s*([^;]+);").unwrap());
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    pub target_ratio: f32,
    pub lookahead_ms: i64,
    pub long_word_ms: i64,
    pub reduced_motion: bool,
    pub disable_richsync: bool,
    pub passive_scroll: bool,
    pub passive_seconds_per_line: f32,
    pub scroll_duration_ms: u64,
    pub richsync_offset_ms: i64,
    pub line_offset_ms: i64,
    pub extras: BTreeMap<String, String>,
}
impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            target_ratio: 0.37,
            lookahead_ms: 540,
            long_word_ms: 1500,
            reduced_motion: false,
            disable_richsync: false,
            passive_scroll: false,
            passive_seconds_per_line: 3.5,
            scroll_duration_ms: 750,
            richsync_offset_ms: 0,
            line_offset_ms: 0,
            extras: BTreeMap::new(),
        }
    }
}
pub fn parse_config(css: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for c in COMMENTS.captures_iter(css) {
        for c in SETTINGS.captures_iter(&c[1]) {
            out.insert(c[1].into(), c[2].trim().into());
        }
    }
    out
}
pub fn milliseconds(raw: &str) -> Option<i64> {
    let (v, m) = if let Some(v) = raw.strip_suffix("ms") {
        (v, 1.)
    } else if let Some(v) = raw.strip_suffix('s') {
        (v, 1000.)
    } else {
        (raw, 1.)
    };
    v.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .map(|n| (n * m) as i64)
}
impl ThemeSettings {
    pub fn from_css(css: &str) -> Self {
        let mut s = Self::default();
        for (key, value) in parse_config(css) {
            let number = value.parse::<f32>().ok().filter(|n| n.is_finite());
            match key.as_str() {
                "blyrics-target-scroll-pos-ratio" => {
                    if let Some(n) = number {
                        s.target_ratio = n.clamp(0., 1.);
                    }
                }
                "blyrics-early-scroll-consider-s" => {
                    if let Some(n) = number {
                        s.lookahead_ms = (n.max(0.) * 1000.) as i64;
                    }
                }
                "blyrics-long-word-threshold" => {
                    if let Some(n) = milliseconds(&value) {
                        s.long_word_ms = n.max(0);
                    }
                }
                "blyrics-disable-richsync" => {
                    s.disable_richsync = value.eq_ignore_ascii_case("true")
                }
                "blyrics-reduced-motion" => s.reduced_motion = value.eq_ignore_ascii_case("true"),
                "blyrics-passive-scroll-enabled" => {
                    s.passive_scroll = value.eq_ignore_ascii_case("true")
                }
                "blyrics-passive-scroll-seconds-per-line" => {
                    if let Some(n) = number.filter(|n| *n > 0.) {
                        s.passive_seconds_per_line = n;
                    }
                }
                "blyrics-line-scroll-duration" => {
                    if let Some(n) = milliseconds(&value).filter(|n| *n > 0) {
                        s.scroll_duration_ms = n as u64;
                    }
                }
                "blyrics-richsync-timing-offset" => {
                    if let Some(n) = milliseconds(&value) {
                        s.richsync_offset_ms = n;
                    }
                }
                "blyrics-timing-offset" => {
                    if let Some(n) = milliseconds(&value) {
                        s.line_offset_ms = n;
                    }
                }
                _ => {
                    s.extras.insert(key, value);
                }
            }
        }
        s
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comment_only_last_wins_and_reset() {
        let s=ThemeSettings::from_css("blyrics-target-scroll-pos-ratio = 0.9; /* blyrics-target-scroll-pos-ratio = 0.2; */ /* blyrics-target-scroll-pos-ratio = 0.5; blyrics-line-scroll-duration=0.8s; */");
        assert_eq!(s.target_ratio, 0.5);
        assert_eq!(s.scroll_duration_ms, 800);
        assert_eq!(ThemeSettings::from_css("").target_ratio, 0.37);
    }
}
