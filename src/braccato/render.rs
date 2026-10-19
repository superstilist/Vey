//! Prepared, grapheme-safe karaoke text. No DOM, JavaScript, or per-frame parsing
//! for dormant lines. Joining scripts stay in a single shaping run.
use super::{engine::word_progress, text};
use crate::WordRow;
use slint::{Color, StyledText};
use std::fmt::Write;
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '*' | '_' | '[' | ']' | '`' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}
#[derive(Debug)]
struct PreparedWord {
    start: i64,
    duration: i64,
    background: bool,
    runs: Vec<String>,
}
#[derive(Default)]
pub struct PreparedText {
    words: Vec<PreparedWord>,
    key: Vec<u16>,
    scratch: Vec<u16>,
    cached: StyledText,
    updates: u64,
}
impl PreparedText {
    pub fn new(words: impl IntoIterator<Item = WordRow>) -> Self {
        Self {
            words: words
                .into_iter()
                .map(|p| {
                    let runs = if text::is_joining(&p.text) {
                        vec![escape(&p.text)]
                    } else {
                        text::graphemes(&p.text).map(escape).collect()
                    };
                    PreparedWord {
                        start: p.start_ms as i64,
                        duration: (p.end_ms as i64 - p.start_ms as i64).max(0),
                        background: p.background,
                        runs,
                    }
                })
                .collect(),
            ..Default::default()
        }
    }
    pub fn render(
        &mut self,
        time: i64,
        active: bool,
        foreground: Color,
        background: Color,
        weight: i32,
        backing: bool,
        reduced: bool,
    ) -> StyledText {
        let mut key = std::mem::take(&mut self.scratch);
        key.clear();
        for c in [foreground.as_argb_encoded(), background.as_argb_encoded()] {
            key.push((c >> 16) as u16);
            key.push(c as u16);
        }
        key.extend([weight as u16, active as u16, backing as u16]);
        key.extend(
            self.words
                .iter()
                .filter(|w| w.background == backing)
                .map(|w| {
                    if !active {
                        0
                    } else if reduced {
                        if time >= w.start {
                            4096
                        } else {
                            0
                        }
                    } else {
                        (word_progress(w.start, w.duration, time) * 4096.).round() as u16
                    }
                }),
        );
        if key == self.key {
            self.scratch = key;
            return self.cached.clone();
        }
        std::mem::swap(&mut self.key, &mut key);
        self.scratch = key;
        let mut out = String::new();
        for w in self.words.iter().filter(|w| w.background == backing) {
            let progress = if !active {
                0.
            } else if reduced {
                if time >= w.start {
                    1.
                } else {
                    0.
                }
            } else {
                word_progress(w.start, w.duration, time)
            };
            let n = w.runs.len().max(1) as f32;
            for (i, run) in w.runs.iter().enumerate() {
                let mix = (progress * n - i as f32).clamp(0., 1.);
                let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * mix).round() as u8;
                let color = Color::from_rgb_u8(
                    blend(background.red(), foreground.red()),
                    blend(background.green(), foreground.green()),
                    blend(background.blue(), foreground.blue()),
                );
                let _ = write!(
                    out,
                    "<font color=\"#{:06x}\">{}</font>",
                    color.as_argb_encoded() & 0xffffff,
                    run
                );
            }
        }
        if weight >= 600 {
            out = format!("**{out}**");
        }
        self.cached = StyledText::from_markdown(&out).unwrap_or_default();
        self.updates += 1;
        self.cached.clone()
    }
    pub fn updates(&self) -> u64 {
        self.updates
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_markup() {
        assert_eq!(escape("<hello> & [x] *"), "&lt;hello&gt; &amp; \\[x\\] \\*");
    }
    #[test]
    fn paused_and_dormant_text_is_cached() {
        let mut p = PreparedText::new([WordRow {
            text: "Hello".into(),
            start_ms: 1000,
            end_ms: 2000,
            ..Default::default()
        }]);
        let fg = Color::from_rgb_u8(255, 255, 255);
        let bg = Color::from_rgb_u8(90, 90, 90);
        p.render(0, false, fg, bg, 700, false, false);
        for time in 0..1000 {
            p.render(time, false, fg, bg, 700, false, false);
        }
        assert_eq!(p.updates(), 1);
        p.render(1500, true, fg, bg, 700, false, false);
        assert_eq!(p.updates(), 2);
        p.render(1500, true, fg, bg, 700, false, false);
        assert_eq!(p.updates(), 2);
    }
}
