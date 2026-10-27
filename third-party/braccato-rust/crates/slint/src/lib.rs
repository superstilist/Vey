//! Rust model and Slint source for embeddable native lyrics views.
//!
//! Import `ui/lyrics.slint` as a Slint library in the consuming application's `build.rs`, then
//! instantiate `LyricsView`. The source file is included in this crate's published package.

pub use braccato_engine::{
    FrameState, Playback, StageRole, StageSelection, frame, line_sync_phase, seek_time,
    stage_roles, stage_selection,
};
pub use braccato_themes::LyricsTheme;
pub use braccato_types::{Lyric, LyricPart, LyricSyncType, Translation};
use slint::{ModelRc, VecModel};
mod glow;
mod instrumental;
mod layout;
mod sweep;
pub use glow::{GlowCache, GlowCacheStats, WordGlow};
pub use instrumental::instrumental_frames;
pub use layout::{LayoutOptions, ViewLayout, wrap_rows};
pub use sweep::{LetterSweep, SwipeRamp, plan_letter_mask_sweep};

slint::include_modules!();
pub type LyricRow = BraccatoRow;

pub const SLINT_LIBRARY_SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui/lyrics.slint");

/// Convert parsed lyrics and the current clock selection into the Slint model expected by `LyricsView`.
pub fn lyric_rows(
    lyrics: &[Lyric],
    active: Option<(usize, Option<usize>)>,
    playback_time_ms: f64,
) -> ModelRc<LyricRow> {
    let mut rows = Vec::new();
    let roles = stage_roles(lyrics, playback_time_ms);
    for (line_index, lyric) in lyrics.iter().enumerate() {
        let is_active = active.is_some_and(|(index, _)| index == line_index);
        let stage_role = match roles[line_index] {
            StageRole::Current => "current",
            StageRole::Previous => "previous",
            StageRole::Queued => "queued",
            StageRole::Gone => "gone",
        };
        let parts = lyric
            .parts
            .as_deref()
            .filter(|parts| !parts.is_empty())
            .map(|parts| parts.to_vec())
            .unwrap_or_else(|| {
                if lyric.sync_type() == LyricSyncType::None {
                    return Vec::new();
                }
                let mut words = Vec::new();
                let mut token = String::new();
                let mut whitespace = None;
                for character in lyric.words.chars() {
                    let is_space = character.is_whitespace();
                    if whitespace.is_some_and(|previous| previous != is_space) {
                        words.push(std::mem::take(&mut token));
                    }
                    whitespace = Some(is_space);
                    token.push(character);
                }
                if !token.is_empty() {
                    words.push(token);
                }
                words
                    .into_iter()
                    .map(|words| braccato_types::LyricPart {
                        words,
                        start_time_ms: lyric.start_time_ms,
                        duration_ms: 0.0,
                        ..Default::default()
                    })
                    .collect()
            });
        rows.push(LyricRow {
            text: lyric.words.clone().into(),
            active: is_active,
            secondary: false,
            translation: false,
            romanization: false,
            agent: lyric.agent.clone().unwrap_or_default().into(),
            instrumental: lyric.is_instrumental(),
            stage_role: stage_role.into(),
            source_index: line_index as i32,
            sustain_blurred: Default::default(),
            sustain_blurred_width: 0.0,
            sustain_blurred_height: 0.0,
            sustain_blurred_padding: 0.0,
            sustain_distance: 0,
            focus_mix: 0.0,
            start_ms: lyric.start_time_ms as f32,
            end_ms: if lyric.duration_ms > 0.0 {
                (lyric.start_time_ms + lyric.duration_ms) as f32
            } else {
                lyrics
                    .get(line_index + 1)
                    .map_or(f32::MAX, |next| next.start_time_ms as f32)
            },
            font_family: "".into(),
            parts: ModelRc::from(std::rc::Rc::new(VecModel::from(
                parts
                    .iter()
                    .enumerate()
                    .map(|(part_index, part)| {
                        let line_synced = lyric.parts.as_ref().is_none_or(|parts| parts.is_empty());
                        let token_word_index = parts[..part_index]
                            .iter()
                            .filter(|p| !p.words.chars().all(char::is_whitespace))
                            .count();
                        let start_ms =
                            if line_synced && !part.words.chars().all(char::is_whitespace) {
                                lyric.start_time_ms + token_word_index as f64 * 50.0
                            } else {
                                part.start_time_ms
                            };
                        LyricWord {
                            cue_offset: 0.0,
                            cue_span: 1.0,
                            motion_start_ms: 0.0,
                            motion_duration_ms: 0.0,
                            motion_origin_em: 0.0,
                            source_part: part_index as i32,
                            glow_wide: Default::default(),
                            glow_tight: Default::default(),
                            glow_padding: 32.0,
                            glow_levels: 0,
                            glow_precolored: false,
                            sustain_blurred: Default::default(),
                            glyphs: Default::default(),
                            text: part.words.clone().into(),
                            active: is_active
                                && if line_synced {
                                    !part.words.chars().all(char::is_whitespace)
                                        && playback_time_ms >= start_ms
                                        && playback_time_ms < start_ms + 330.0
                                } else {
                                    active.is_some_and(|(_, word)| word == Some(part_index))
                                },
                            background: part.is_background.unwrap_or(false),
                            start_ms: start_ms as f32,
                            duration_ms: if line_synced {
                                0.0
                            } else {
                                part.duration_ms as f32
                            },
                            font_family: "".into(),
                        }
                    })
                    .collect::<Vec<_>>(),
            ))),
        });
        let has_language_translations = lyric
            .translations
            .as_ref()
            .is_some_and(|translations| !translations.is_empty());
        if !has_language_translations && let Some(translation) = &lyric.translation {
            rows.push(secondary_row(
                lyric,
                line_index,
                is_active,
                &translation.text,
                true,
                false,
                stage_role,
            ));
        }
        if let Some(translations) = &lyric.translations {
            rows.extend(translations.values().map(|text| {
                secondary_row(lyric, line_index, is_active, text, true, false, stage_role)
            }));
        }
        if let Some(romanization) = &lyric.romanization {
            let mut row = secondary_row(
                lyric,
                line_index,
                is_active,
                romanization,
                false,
                true,
                stage_role,
            );
            row.parts = ModelRc::from(std::rc::Rc::new(VecModel::from(
                lyric
                    .timed_romanization
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                    .map(|(part_index, part)| LyricWord {
                        cue_offset: 0.0,
                        cue_span: 1.0,
                        motion_start_ms: 0.0,
                        motion_duration_ms: 0.0,
                        motion_origin_em: 0.0,
                        source_part: part_index as i32,
                        glow_wide: Default::default(),
                        glow_tight: Default::default(),
                        glow_padding: 32.0,
                            glow_levels: 0,
                            glow_precolored: false,
                        sustain_blurred: Default::default(),
                        glyphs: Default::default(),
                        text: part.words.clone().into(),
                        active: is_active
                            && active.is_some_and(|(_, word)| word == Some(part_index)),
                        background: part.is_background.unwrap_or(false),
                        start_ms: part.start_time_ms as f32,
                        duration_ms: part.duration_ms as f32,
                        font_family: "".into(),
                    })
                    .collect::<Vec<_>>(),
            )));
            rows.push(row);
        }
        // Decorations share the source line's exit fade, including open-ended line cues.
        let end_ms = if lyric.duration_ms > 0.0 {
            (lyric.start_time_ms + lyric.duration_ms) as f32
        } else {
            lyrics
                .get(line_index + 1)
                .map_or(f32::MAX, |next| next.start_time_ms as f32)
        };
        for row in rows
            .iter_mut()
            .rev()
            .take_while(|row| row.source_index == line_index as i32)
        {
            row.end_ms = end_ms;
        }
    }
    ModelRc::from(std::rc::Rc::new(VecModel::from(rows)))
}

fn secondary_row(
    lyric: &Lyric,
    index: usize,
    active: bool,
    text: &str,
    translation: bool,
    romanization: bool,
    stage_role: &str,
) -> LyricRow {
    LyricRow {
        text: text.to_owned().into(),
        active,
        secondary: true,
        translation,
        romanization,
        agent: lyric.agent.clone().unwrap_or_default().into(),
        instrumental: false,
        stage_role: stage_role.into(),
        source_index: index as i32,
        start_ms: lyric.start_time_ms as f32,
        end_ms: (lyric.start_time_ms + lyric.duration_ms) as f32,
        font_family: "".into(),
        parts: ModelRc::default(),
        sustain_blurred: Default::default(),
        sustain_blurred_width: 0.0,
        sustain_blurred_height: 0.0,
        sustain_blurred_padding: 0.0,
        sustain_distance: 0,
        focus_mix: 0.0,
    }
}

/// Return the visible row index for a lyric, accounting for inserted translation and romanization rows.
pub fn visual_line_index(lyrics: &[Lyric], active: Option<usize>) -> i32 {
    let Some(active) = active.filter(|index| *index < lyrics.len()) else {
        return -1;
    };
    lyrics[..active]
        .iter()
        .map(|lyric| {
            1 + usize::from(lyric.translation.is_some()).saturating_sub(usize::from(
                lyric
                    .translations
                    .as_ref()
                    .is_some_and(|translations| !translations.is_empty()),
            )) + lyric.translations.as_ref().map_or(0, |values| values.len())
                + usize::from(lyric.romanization.is_some())
        })
        .sum::<usize>() as i32
}
