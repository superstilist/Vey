//! Immutable Slint models shared across cue updates, seeks and glow/color changes.
use crate::BraccatoGlyph as LyricGlyph;
use braccato_slint::LyricsTheme;
use braccato_slint::WordGlow;
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
        let mut byte = 0;
        let raw = (0..planned.row_count())
            .filter_map(|i| planned.row_data(i))
            .map(|glyph| {
                let first = byte;
                byte += glyph.text.len();
                (first..byte, glyph)
            })
            .collect::<Vec<_>>();
        let shaped = if raw.is_empty() {
            None
        } else {
            crate::native_lyric_fonts::letter_geometry(text, family, weight)
        };
        let glyphs = shaped
            .and_then(|geometry| {
                geometry
                    .into_iter()
                    .map(|cluster| {
                        let timed = raw
                            .iter()
                            .filter(|(range, _)| {
                                range.start < cluster.byte_end && range.end > cluster.byte_start
                            })
                            .collect::<Vec<_>>();
                        let first = &timed.first()?.1;
                        let delay = timed
                            .iter()
                            .map(|(_, g)| g.delay_ms)
                            .fold(f32::INFINITY, f32::min);
                        let until = timed
                            .iter()
                            .map(|(_, g)| g.delay_ms + g.wave_ms)
                            .fold(delay, f32::max);
                        Some(LyricGlyph {
                            text: cluster.text.into(),
                            start: cluster.start,
                            width: cluster.width,
                            start_em: cluster.start_em,
                            width_em: cluster.width_em,
                            isolated_advance_em: cluster.isolated_advance_em,
                            delay_ms: delay,
                            wave_ms: until - delay,
                            sweep_delay_ms: first.sweep_delay_ms,
                            sweep_duration_ms: first.sweep_duration_ms,
                            sweep_from: first.sweep_from,
                            sweep_to: first.sweep_to,
                            sweep_feather: first.sweep_feather,
                        })
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or_default();
        let count = glyphs.len();
        if count > 32768 {
            return ModelRc::default();
        }
        let plan = if count == 0 {
            ModelRc::default()
        } else {
            ModelRc::new(Rc::new(VecModel::from(glyphs)))
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
        let mut shadows =
            GlowCache::new([include_bytes!("../assets/fonts/Roboto.ttf").to_vec()], 2);
        let mut plans = GlyphPlans::default();
        let text = SharedString::from("waves");
        let theme = LyricsTheme::default();
        let metrics = shadows.word_with_radii(&text, "Roboto", 700, 0.0, 0.0, 36.0);
        let first = plans.get(&text, "Roboto", 700, 400.0, &theme, &metrics);
        let shadow = shadows.word(&text, "Roboto", 700);
        let recolored = LyricsTheme {
            foreground: "#f0b0c0".into(),
            highlight_glow: false,
            ..theme.clone()
        };
        assert_eq!(
            first,
            plans.get(&text, "Roboto", 700, 400.0, &recolored, &shadow)
        );
        assert_ne!(
            first,
            plans.get(&text, "Roboto", 700, 800.0, &theme, &metrics)
        );
        let faster = LyricsTheme {
            swipe_duration_ratio: 2.0,
            ..theme
        };
        assert_ne!(
            first,
            plans.get(&text, "Roboto", 700, 400.0, &faster, &metrics)
        );
    }

    #[test]
    fn plan_storage_is_bounded() {
        let mut plans = GlyphPlans::default();
        let theme = LyricsTheme::default();
        for index in 0..1300 {
            plans.get(
                &format!("word{index}").into(),
                "Roboto",
                700,
                400.0,
                &theme,
                &WordGlow::default(),
            );
        }
        assert_eq!(plans.entries.len(), 1024);
        assert!(plans.glyphs <= 32768);
    }

    #[test]
    fn animated_clusters_keep_shaped_kerning_ligatures_and_marks() {
        let mut shadows = crate::native_lyric_fonts::cache();
        let mut plans = GlyphPlans::default();
        let theme = LyricsTheme::default();
        for text in ["AVATAR", "office", "e\u{301}cole"] {
            let metrics = shadows.word(text, "Satoshi", 700);
            let plan = plans.get(&text.into(), "Satoshi", 700, 400., &theme, &metrics);
            let geometry = crate::native_lyric_fonts::letter_geometry(text, "Satoshi", 700)
                .expect("independently paintable shaped clusters");
            assert_eq!(plan.row_count(), geometry.len());
            let mut restored = String::new();
            for (i, cluster) in geometry.iter().enumerate() {
                let glyph = plan.row_data(i).unwrap();
                restored.push_str(&glyph.text);
                assert_eq!(glyph.start_em, cluster.start_em);
                assert_eq!(glyph.width_em, cluster.width_em);
                assert_eq!(glyph.isolated_advance_em, cluster.isolated_advance_em);
                assert!(glyph.wave_ms > 0. && glyph.delay_ms.is_finite());
            }
            assert_eq!(restored, text);
        }
    }
}
