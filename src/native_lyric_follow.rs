//! Retained source-to-visual lookup. Wrapped rows never acquire inferred cues.
use crate::braccato::engine::{scroll_top, LineGeometry};
use crate::BraccatoRow;
use slint::{Model, ModelRc};
use std::ops::Range;

#[derive(Default)]
pub(crate) struct FollowIndex {
    pub sources: Vec<Range<usize>>,
    cues: Vec<Vec<(i64, usize)>>,
    row_types: Vec<u8>,
}
impl FollowIndex {
    // Scroll ahead independently of the sung source/word. This never changes
    // highlight timestamps, and paused/reduced-motion seeks remain exact.
    pub fn scroll_time(time: i64, playing: bool, reduced: bool, offset: i32) -> i64 {
        time.saturating_add(if playing && !reduced {
            offset.clamp(-60000, 60000) as i64
        } else {
            0
        })
    }
    pub fn new(rows: &ModelRc<BraccatoRow>, sources: usize) -> Self {
        let mut index = Self {
            sources: vec![0..0; sources],
            cues: vec![vec![]; sources],
            row_types: Vec::with_capacity(rows.row_count()),
        };
        for visual in 0..rows.row_count() {
            let Some(row) = rows.row_data(visual) else {
                continue;
            };
            index.row_types.push(if row.secondary {
                2
            } else if row.parts.row_data(0).is_some_and(|p| p.background) {
                1
            } else {
                0
            });
            let source = row.source_index as usize;
            let Some(range) = index.sources.get_mut(source) else {
                continue;
            };
            if range.start == range.end {
                range.start = visual;
            }
            range.end = visual + 1;
            if !row.secondary && !row.instrumental {
                for n in 0..row.parts.row_count() {
                    if let Some(part) = row.parts.row_data(n).filter(|p| !p.background) {
                        // A wrapped fragment shares its original cue. Follow the
                        // completed fill edge into that fragment, rather than
                        // selecting every continuation at the cue's first ms.
                        let phase = if part.cue_offset > 0. && part.duration_ms > 0. {
                            part.cue_offset.clamp(0., 1.)
                        } else {
                            0.
                        };
                        index.cues[source].push((
                            (part.start_ms + part.duration_ms * phase).round() as i64,
                            visual,
                        ));
                    }
                }
            }
        }
        for cues in &mut index.cues {
            cues.sort_unstable();
        }
        index
    }
    pub fn visual(&self, source: usize, time: i64) -> Option<usize> {
        let range = self.sources.get(source)?.clone();
        if range.start == range.end {
            return None;
        }
        let cues = &self.cues[source];
        let next = cues.partition_point(|(start, _)| *start <= time);
        Some(next.checked_sub(1).map_or(range.start, |i| cues[i].1))
    }
    pub fn extend_rows(&self, sources: &[usize], rows: &mut Vec<usize>) {
        for source in sources {
            if let Some(range) = self.sources.get(*source) {
                rows.extend(range.clone());
            }
        }
    }
    /// Original active-band geometry, with a visual-row pin for source lines
    /// taller than the native viewport. Metrics are retained, not read per tick.
    pub fn target(
        &self,
        sources: &[usize],
        anchor_source: usize,
        visual: usize,
        offsets: &[f32],
        size: f32,
        spacing: f32,
        viewport: f32,
        ratio: f32,
    ) -> (f32, bool) {
        let padding =
            (viewport * ratio - size * spacing / 2.).max(if size >= 40. { 36. } else { 24. });
        let height = |row: usize| {
            size * match self.row_types[row] {
                2 => 0.7,
                1 => 0.95,
                _ => spacing,
            }
        };
        let geometry = |source: usize| {
            let range = self.sources[source].clone();
            let last = range.end - 1;
            LineGeometry {
                top: padding + offsets[range.start],
                height: offsets[last] + height(last) - offsets[range.start],
            }
        };
        let mut group = sources
            .iter()
            .copied()
            .filter(|s| self.sources[*s].start < self.sources[*s].end)
            .map(geometry)
            .collect::<Vec<_>>();
        if group.is_empty() {
            return (0., false);
        }
        let overflows =
            group.last().unwrap().top + group.last().unwrap().height - group[0].top > viewport;
        let mut anchor = geometry(anchor_source);
        if anchor.height > viewport {
            anchor = LineGeometry {
                top: padding + offsets[visual],
                height: height(visual),
            };
            // A single long source is several native rows. Keep its current
            // word row at the target instead of dragging the full source box.
            for (source, item) in sources
                .iter()
                .filter(|s| self.sources[**s].start < self.sources[**s].end)
                .zip(&mut group)
            {
                if *source == anchor_source {
                    *item = anchor;
                }
            }
        }
        (
            scroll_top(&group, anchor, viewport, 0., 0., ratio),
            overflows,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BraccatoWord;
    use slint::VecModel;
    use std::rc::Rc;
    #[test]
    fn scroll_lead_is_bounded_and_never_changes_paused_or_reduced_time() {
        assert_eq!(FollowIndex::scroll_time(1000, true, false, 500), 1500);
        assert_eq!(FollowIndex::scroll_time(1000, true, false, 0), 1000);
        assert_eq!(FollowIndex::scroll_time(1000, false, false, 500), 1000);
        assert_eq!(FollowIndex::scroll_time(1000, true, true, 500), 1000);
        assert_eq!(FollowIndex::scroll_time(1000, true, false, -100), 900);
        assert_eq!(
            FollowIndex::scroll_time(i64::MAX, true, false, 540),
            i64::MAX
        );
    }
    #[test]
    fn one_long_cue_follows_its_fill_across_fragments_instead_of_jumping_to_the_last() {
        let rows: ModelRc<BraccatoRow> = Rc::new(VecModel::from(
            [0., 0.5]
                .into_iter()
                .map(|offset| BraccatoRow {
                    source_index: 0,
                    parts: Rc::new(VecModel::from(vec![BraccatoWord {
                        start_ms: 1000.,
                        duration_ms: 4000.,
                        cue_offset: offset,
                        cue_span: 0.5,
                        ..Default::default()
                    }]))
                    .into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        ))
        .into();
        let normal = FollowIndex::new(&rows, 1);
        assert_eq!(normal.visual(0, 1000), Some(0));
        assert_eq!(normal.visual(0, 2999), Some(0));
        assert_eq!(normal.visual(0, 3000), Some(1));
        assert_eq!(normal.visual(0, 1200), Some(0));
    }

    #[test]
    fn follows_real_word_cues_into_wrapped_rows_and_seeks_back() {
        let rows: ModelRc<BraccatoRow> = Rc::new(VecModel::from(vec![
            BraccatoRow {
                source_index: 0,
                parts: Rc::new(VecModel::from(vec![BraccatoWord {
                    start_ms: 1000.,
                    ..Default::default()
                }]))
                .into(),
                ..Default::default()
            },
            BraccatoRow {
                source_index: 0,
                parts: Rc::new(VecModel::from(vec![BraccatoWord {
                    start_ms: 2000.,
                    ..Default::default()
                }]))
                .into(),
                ..Default::default()
            },
            BraccatoRow {
                source_index: 0,
                secondary: true,
                ..Default::default()
            },
            BraccatoRow {
                source_index: 1,
                ..Default::default()
            },
        ]))
        .into();
        let index = FollowIndex::new(&rows, 2);
        assert_eq!(index.visual(0, 1800), Some(0));
        assert_eq!(index.visual(0, 2500), Some(1));
        assert_eq!(index.visual(0, 1000), Some(0));
        assert_eq!(index.visual(1, 9000), Some(3));
        assert_eq!(index.visual(2, 9000), None);
    }
}
