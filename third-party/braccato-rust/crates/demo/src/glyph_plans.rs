//! Immutable Slint models shared across cue updates, seeks and glow/color changes.
use crate::LyricGlyph;
use braccato_slint::WordGlow;
use braccato_themes::LyricsTheme;
use slint::{Model, ModelRc, SharedString, VecModel};
use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
};

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    text: SharedString,
    family: String,
    weight: u16,
    settings: [u32; 8],
}

#[derive(Default)]
pub struct GlyphPlans {
    entries: HashMap<Key, ModelRc<LyricGlyph>>,
    order: VecDeque<Key>,
    glyphs: usize,
}

impl GlyphPlans {
    pub fn get(
        &mut self,
        text: &SharedString,
        family: &str,
        weight: u16,
        duration: f32,
        theme: &LyricsTheme,
        glow: &WordGlow,
    ) -> ModelRc<LyricGlyph> {
        if duration <= 0.0 || text.trim().is_empty() {
            return ModelRc::default();
        }
        let key = Key {
            text: text.clone(),
            family: family.to_owned(),
            weight,
            settings: [
                duration,
                theme.letter_wave_duration_ms,
                theme.swipe_lead_ratio,
                theme.swipe_duration_ratio,
                theme.highlight_swipe_start_from,
                theme.highlight_swipe_end_from,
                theme.highlight_swipe_start_to,
                theme.highlight_swipe_end_to,
            ]
            .map(f32::to_bits),
        };
        if let Some(plan) = self.entries.get(&key) {
            return plan.clone();
        }
        let planned = glow.letters_with_settings(
            duration,
            theme.letter_wave_duration_ms,
            theme.swipe_lead_ratio,
            theme.swipe_duration_ratio,
            theme.highlight_swipe_start_from,
            theme.highlight_swipe_end_from,
            theme.highlight_swipe_start_to,
            theme.highlight_swipe_end_to,
        );
        let count = planned.row_count();
        let plan = if count == 0 {
            ModelRc::default()
        } else {
            ModelRc::new(Rc::new(VecModel::from(
                (0..count)
                    .filter_map(|i| planned.row_data(i))
                    .map(|g| LyricGlyph {
                        text: g.text,
                        start: g.start,
                        width: g.width,
                        delay_ms: g.delay_ms,
                        wave_ms: g.wave_ms,
                        sweep_delay_ms: g.sweep_delay_ms,
                        sweep_duration_ms: g.sweep_duration_ms,
                        sweep_from: g.sweep_from,
                        sweep_to: g.sweep_to,
                        sweep_feather: g.sweep_feather,
                    })
                    .collect::<Vec<_>>(),
            )))
        };
        while self.entries.len() >= 1024 || self.glyphs + count > 32768 {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&old) {
                self.glyphs -= evicted.row_count();
            }
        }
        self.glyphs += count;
        self.order.push_back(key.clone());
        self.entries.insert(key, plan.clone());
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braccato_slint::GlowCache;

    #[test]
    fn stable_models_survive_color_and_glow_changes_but_timing_invalidates() {
        let mut shadows = GlowCache::new(
            [include_bytes!("../assets/fonts/Satoshi-700.otf").to_vec()],
            2,
        );
        let mut plans = GlyphPlans::default();
        let text = SharedString::from("waves");
        let theme = LyricsTheme::default();
        let metrics = shadows.word_with_radii(&text, "Satoshi", 700, 0.0, 0.0, 36.0);
        let first = plans.get(&text, "Satoshi", 700, 400.0, &theme, &metrics);
        let shadow = shadows.word(&text, "Satoshi", 700);
        let recolored = LyricsTheme {
            foreground: "#f0b0c0".into(),
            highlight_glow: false,
            ..theme.clone()
        };
        assert_eq!(
            first,
            plans.get(&text, "Satoshi", 700, 400.0, &recolored, &shadow)
        );
        assert_ne!(
            first,
            plans.get(&text, "Satoshi", 700, 800.0, &theme, &metrics)
        );
        let faster = LyricsTheme {
            swipe_duration_ratio: 2.0,
            ..theme
        };
        assert_ne!(
            first,
            plans.get(&text, "Satoshi", 700, 400.0, &faster, &metrics)
        );
    }

    #[test]
    fn plan_storage_is_bounded() {
        let mut plans = GlyphPlans::default();
        let theme = LyricsTheme::default();
        for index in 0..1300 {
            plans.get(
                &format!("word{index}").into(),
                "Satoshi",
                700,
                400.0,
                &theme,
                &WordGlow::default(),
            );
        }
        assert_eq!(plans.entries.len(), 1024);
        assert!(plans.glyphs <= 32768);
    }
}
