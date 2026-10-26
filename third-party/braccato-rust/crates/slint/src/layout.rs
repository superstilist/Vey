//! Native wrapping retains the original cue index on every displayed fragment.
use crate::{LyricRow, LyricWord};
use slint::{Model, ModelRc, VecModel};
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

pub struct ViewLayout {
    pub rows: ModelRc<LyricRow>,
    pub active_row: i32,
    pub active_offset: f32,
}

#[derive(Clone, Copy)]
pub struct LayoutOptions {
    pub width: f32,
    pub font_size: f32,
    pub line_spacing: f32,
    pub background_font_scale: f32,
    /// Vertical room on each side of a source lyric, shared by its wrapped/decorated rows.
    pub line_padding: f32,
    pub show_translations: bool,
    pub show_romanizations: bool,
}

// Keep nonbreaking hyphens intact unless emergency wrapping is required.
fn break_after(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '\u{2010}' | '\u{00ad}' | '\u{200b}')
}
#[derive(Clone)]
struct Fragment {
    word: LyricWord,
    width: f32,
    size: f32,
    cue: usize,
}
struct RowWrap<'a, F> {
    row: &'a LyricRow,
    rows: &'a mut Vec<LyricRow>,
    current: Vec<LyricWord>,
    unit: Vec<Fragment>,
    used: f32,
    last_cue: Option<usize>,
    limit: f32,
    measure: &'a F,
}
impl<F: Fn(&str, f32) -> f32> RowWrap<'_, F> {
    fn flush(&mut self) {
        if self.current.is_empty() {
            return;
        }
        let mut row = self.row.clone();
        row.text = self
            .current
            .iter()
            .map(|p| p.text.as_str())
            .collect::<String>()
            .into();
        row.parts = ModelRc::new(Rc::new(VecModel::from(std::mem::take(&mut self.current))));
        self.rows.push(row);
        self.used = 0.;
        self.last_cue = None;
    }
    fn append(&mut self, fragment: Fragment) {
        if self.last_cue == Some(fragment.cue) {
            let previous = self.current.last_mut().unwrap();
            let joined = format!("{}{}", previous.text, fragment.word.text);
            self.used += (self.measure)(&joined, fragment.size)
                - (self.measure)(&previous.text, fragment.size);
            previous.text = joined.into();
            previous.cue_span += fragment.word.cue_span;
        } else {
            self.used += fragment.width;
            self.current.push(fragment.word);
        }
        self.last_cue = Some(fragment.cue);
    }
    fn place_unit(&mut self) {
        if self.unit.is_empty() {
            return;
        }
        let width = self.unit.iter().map(|f| f.width).sum::<f32>();
        let background = self.unit[0].word.background;
        if !self.current.is_empty()
            && (self.used + width > self.limit
                || self.current.last().unwrap().background != background)
        {
            self.flush();
        }
        // A word may contain several separately timed syllables. Move it as a
        // unit when it fits; only oversized units need grapheme-safe splitting.
        for fragment in std::mem::take(&mut self.unit) {
            if width <= self.limit {
                self.append(fragment);
                continue;
            }
            let graphemes = fragment
                .word
                .text
                .graphemes(true)
                .map(|text| (text, (self.measure)(text, fragment.size).max(0.)))
                .collect::<Vec<_>>();
            let total = graphemes.iter().map(|(_, w)| w).sum::<f32>().max(0.001);
            let mut consumed = 0.;
            for (text, advance) in graphemes {
                if !self.current.is_empty() && self.used + advance > self.limit {
                    self.flush();
                }
                let mut piece = fragment.clone();
                piece.word.text = text.into();
                piece.word.cue_offset += consumed / total * fragment.word.cue_span;
                piece.word.cue_span *= advance / total;
                piece.width = advance;
                self.append(piece);
                consumed += advance;
            }
        }
    }
}

/// Measure with the host's native font metrics. No timing is inferred or changed.
pub fn wrap_rows(
    source: &ModelRc<LyricRow>,
    options: LayoutOptions,
    measure: impl Fn(&str, f32) -> f32,
) -> ViewLayout {
    let mut rows = Vec::new();
    for index in 0..source.row_count() {
        let Some(row) = source.row_data(index) else {
            continue;
        };
        if row.translation && !options.show_translations
            || row.romanization && !options.show_romanizations
        {
            continue;
        }
        if row.parts.row_count() == 0 {
            rows.push(row);
            continue;
        }
        let mut layout = RowWrap {
            row: &row,
            rows: &mut rows,
            current: vec![],
            unit: vec![],
            used: 0.,
            last_cue: None,
            limit: options.width.max(1.),
            measure: &measure,
        };
        for part_index in 0..row.parts.row_count() {
            let Some(part) = row.parts.row_data(part_index) else {
                continue;
            };
            if layout
                .unit
                .last()
                .is_some_and(|f| f.word.background != part.background)
            {
                layout.place_unit();
            }
            let size = options.font_size
                * if row.secondary {
                    0.48
                } else if part.background {
                    options.background_font_scale
                } else {
                    1.
                };
            let pieces = part
                .text
                .split_inclusive(break_after)
                .map(|text| (text, measure(text, size).max(0.)))
                .collect::<Vec<_>>();
            let total = pieces
                .iter()
                .map(|(_, width)| width)
                .sum::<f32>()
                .max(0.001);
            let span = if part.cue_span > 0. {
                part.cue_span
            } else {
                1.
            };
            let mut consumed = 0.;
            for (text, width) in pieces {
                let mut word = part.clone();
                word.text = text.into();
                word.cue_offset = part.cue_offset + consumed / total * span;
                word.cue_span = width / total * span;
                layout.unit.push(Fragment {
                    word,
                    width,
                    size,
                    cue: part_index,
                });
                consumed += width;
                if text.ends_with(break_after) {
                    layout.place_unit();
                }
            }
        }
        layout.place_unit();
        layout.flush();
    }
    let mut offset = 0.0;
    let mut active_row = -1;
    let mut active_offset = 0.0;
    for (index, row) in rows.iter().enumerate() {
        let before = if index == 0 || rows[index - 1].source_index != row.source_index {
            options.line_padding.max(0.0)
        } else {
            0.0
        };
        let after = if rows
            .get(index + 1)
            .is_none_or(|next| next.source_index != row.source_index)
        {
            options.line_padding.max(0.0)
        } else {
            0.0
        };
        if row.active && active_row < 0 {
            active_row = index as i32;
            active_offset = offset + before;
        }
        let background = row.parts.row_data(0).is_some_and(|part| part.background);
        offset += before
            + after
            + options.font_size
                * if row.secondary {
                    0.7
                } else if background {
                    0.95
                } else {
                    options.line_spacing
                };
    }
    ViewLayout {
        rows: ModelRc::from(Rc::new(VecModel::from(rows))),
        active_row,
        active_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap_parts(parts: Vec<LyricWord>, width: f32) -> ViewLayout {
        let source = ModelRc::new(Rc::new(VecModel::from(vec![LyricRow {
            parts: ModelRc::new(Rc::new(VecModel::from(parts))),
            ..Default::default()
        }])));
        wrap_rows(
            &source,
            LayoutOptions {
                width,
                font_size: 32.,
                line_spacing: 1.333,
                background_font_scale: 0.72,
                line_padding: 0.,
                show_translations: true,
                show_romanizations: true,
            },
            |text, _| text.graphemes(true).count() as f32,
        )
    }

    #[test]
    fn hyphenated_chants_wrap_without_losing_text_or_restarting_the_cue() {
        let text = "Ha-ha-ha-ha-ha-ha-ha-ha-ha-ha-ha-ha-ha";
        let view = wrap_parts(
            vec![LyricWord {
                text: text.into(),
                start_ms: 1000.,
                duration_ms: 4000.,
                source_part: 7,
                ..Default::default()
            }],
            7.,
        );
        assert!(view.rows.row_count() > 1);
        let mut joined = String::new();
        let mut edge = 0.;
        for i in 0..view.rows.row_count() {
            let row = view.rows.row_data(i).unwrap();
            assert!(row.text.graphemes(true).count() <= 7);
            joined.push_str(&row.text);
            for j in 0..row.parts.row_count() {
                let part = row.parts.row_data(j).unwrap();
                assert_eq!(
                    (part.start_ms, part.duration_ms, part.source_part),
                    (1000., 4000., 7)
                );
                assert!((part.cue_offset - edge).abs() < 0.0001);
                edge += part.cue_span;
            }
        }
        assert_eq!(joined, text);
        assert!((edge - 1.).abs() < 0.0001);
    }

    #[test]
    fn emergency_wrap_preserves_combining_marks_and_joined_emoji() {
        let text = format!("{}{}", "e\u{301}".repeat(20), "👩‍💻".repeat(3));
        let view = wrap_parts(
            vec![LyricWord {
                text: text.clone().into(),
                ..Default::default()
            }],
            7.,
        );
        let mut actual = vec![];
        for i in 0..view.rows.row_count() {
            let row = view.rows.row_data(i).unwrap();
            assert!(row.text.graphemes(true).count() <= 7);
            actual.extend(row.text.graphemes(true).map(str::to_owned));
        }
        assert_eq!(
            actual,
            text.graphemes(true).map(str::to_owned).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_word_with_separate_syllable_cues_moves_as_a_word_when_it_fits() {
        let view = wrap_parts(
            ["prefix ", "Sin", "gle", " end"]
                .into_iter()
                .enumerate()
                .map(|(i, text)| LyricWord {
                    text: text.into(),
                    source_part: i as i32,
                    start_ms: 1000. + i as f32 * 500.,
                    duration_ms: 500.,
                    ..Default::default()
                })
                .collect(),
            10.,
        );
        assert_eq!(view.rows.row_count(), 2);
        assert_eq!(view.rows.row_data(0).unwrap().text, "prefix ");
        assert_eq!(view.rows.row_data(1).unwrap().text, "Single end");
        let parts = view.rows.row_data(1).unwrap().parts;
        assert_eq!(
            (
                parts.row_data(0).unwrap().source_part,
                parts.row_data(0).unwrap().start_ms
            ),
            (1, 1500.)
        );
        assert_eq!(
            (
                parts.row_data(1).unwrap().source_part,
                parts.row_data(1).unwrap().start_ms
            ),
            (2, 2000.)
        );
    }

    fn background(width: f32) -> ViewLayout {
        let source = ModelRc::new(Rc::new(VecModel::from(vec![LyricRow {
            parts: ModelRc::new(Rc::new(VecModel::from(vec![LyricWord {
                text: "one two three".into(),
                background: true,
                start_ms: 1000.,
                duration_ms: 4000.,
                source_part: 7,
                ..Default::default()
            }]))),
            ..Default::default()
        }])));
        wrap_rows(
            &source,
            LayoutOptions {
                width,
                font_size: 32.,
                line_spacing: 1.333,
                background_font_scale: 0.72,
                line_padding: 0.,
                show_translations: true,
                show_romanizations: true,
            },
            |text, _| text.chars().count() as f32,
        )
    }

    #[test]
    fn a_multiword_cue_has_one_fill_edge() {
        let view = background(20.);
        assert_eq!(view.rows.row_count(), 1);
        let row = view.rows.row_data(0).unwrap();
        assert_eq!(row.parts.row_count(), 1, "spaces must not restart the fill");
        let part = row.parts.row_data(0).unwrap();
        assert_eq!(part.text, "one two three");
        assert_eq!((part.cue_offset, part.cue_span), (0., 1.));
    }

    #[test]
    fn separately_timed_words_remain_separate_cues() {
        let first = background(20.)
            .rows
            .row_data(0)
            .unwrap()
            .parts
            .row_data(0)
            .unwrap();
        let mut next = first.clone();
        next.text = " next".into();
        next.start_ms = 5000.;
        next.source_part = 8;
        let source = ModelRc::new(Rc::new(VecModel::from(vec![LyricRow {
            parts: ModelRc::new(Rc::new(VecModel::from(vec![first, next]))),
            ..Default::default()
        }])));
        let view = wrap_rows(
            &source,
            LayoutOptions {
                width: 100.,
                font_size: 32.,
                line_spacing: 1.333,
                background_font_scale: 0.72,
                line_padding: 0.,
                show_translations: true,
                show_romanizations: true,
            },
            |text, _| text.chars().count() as f32,
        );
        let parts = view.rows.row_data(0).unwrap().parts;
        assert_eq!(parts.row_count(), 2);
        for (index, start) in [1000., 5000.].into_iter().enumerate() {
            let part = parts.row_data(index).unwrap();
            assert_eq!((part.start_ms, part.source_part), (start, 7 + index as i32));
            assert_eq!((part.cue_offset, part.cue_span), (0., 1.));
        }
    }

    #[test]
    fn a_wrapped_cue_continues_the_same_sweep_without_retiming() {
        let view = background(8.);
        assert_eq!(view.rows.row_count(), 2);
        let first = view.rows.row_data(0).unwrap().parts.row_data(0).unwrap();
        let next = view.rows.row_data(1).unwrap().parts.row_data(0).unwrap();
        assert_eq!(
            (first.text.as_str(), next.text.as_str()),
            ("one two ", "three")
        );
        assert!((first.cue_span - 8. / 13.).abs() < 0.0001);
        assert!((next.cue_offset - first.cue_span).abs() < 0.0001);
        assert!((next.cue_offset + next.cue_span - 1.).abs() < 0.0001);
        for part in [first, next] {
            assert!(part.background);
            assert_eq!(
                (part.start_ms, part.duration_ms, part.source_part),
                (1000., 4000., 7)
            );
        }
    }
}
