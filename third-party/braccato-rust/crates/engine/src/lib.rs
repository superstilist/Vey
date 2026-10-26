//! Renderer-independent lyric clock and stage-state logic.

use braccato_types::{Lyric, LyricSyncType};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Playback {
    pub time_ms: f64,
    pub playing: bool,
    pub rate: f64,
    pub offset_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameState {
    pub line: Option<usize>,
    pub part: Option<usize>,
    pub previous_line: Option<usize>,
    pub next_line: Option<usize>,
    pub sync_type: LyricSyncType,
}

pub fn frame(lyrics: &[Lyric], playback: Playback) -> FrameState {
    let time = playback.time_ms - playback.offset_ms;
    let line = lyrics
        .iter()
        .rposition(|l| l.start_time_ms <= time && time < l.start_time_ms + l.duration_ms);
    let active = line.and_then(|index| lyrics.get(index));
    let part = active.and_then(|l| l.parts.as_ref()).and_then(|parts| {
        parts
            .iter()
            .rposition(|p| p.start_time_ms <= time && time < p.start_time_ms + p.duration_ms)
    });
    FrameState {
        line,
        part,
        previous_line: line.and_then(|i| i.checked_sub(1)),
        next_line: line.and_then(|i| (i + 1 < lyrics.len()).then_some(i + 1)),
        sync_type: if active.is_some_and(|l| l.parts.as_ref().is_some_and(|p| !p.is_empty())) {
            LyricSyncType::Richsync
        } else if active.is_some() {
            LyricSyncType::Synced
        } else {
            LyricSyncType::None
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageRole {
    Current,
    Previous,
    Queued,
    Gone,
}

/// Stack-only stage change key for playback loops; all other roles follow from these indices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageSelection {
    pub current: Option<usize>,
    pub previous: Option<usize>,
}

impl StageSelection {
    pub fn role(self, index: usize) -> StageRole {
        if self.previous == Some(index) {
            StageRole::Previous
        } else {
            match self.current {
                Some(current) if index == current => StageRole::Current,
                Some(current) if index < current => StageRole::Gone,
                _ => StageRole::Queued,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageItemKind {
    Line,
    Blank,
    Instrumental,
    Credits,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageItem {
    pub kind: StageItemKind,
    pub start_time_ms: f64,
    pub end_time_ms: f64,
}

const STAGE_LEAD_MS: f64 = 500.0;
const STAGE_HANDOFF_MS: f64 = 120.0;
const STAGE_OVERLAP_MS: f64 = 150.0;

fn stage_overlap(previous: StageItem, item: StageItem) -> bool {
    previous.kind == StageItemKind::Line
        && item.kind != StageItemKind::Blank
        && item.start_time_ms < previous.end_time_ms - STAGE_OVERLAP_MS
}

fn select_stage(len: usize, get: impl Fn(usize) -> StageItem, time_ms: f64) -> StageSelection {
    let current = (0..len).rfind(|&index| {
        let item = get(index);
        let early = item.start_time_ms - STAGE_LEAD_MS;
        let previous = index.checked_sub(1).map(&get);
        let enter = if item.kind == StageItemKind::Credits
            && previous.is_some_and(|p| p.kind == StageItemKind::Instrumental)
        {
            (previous.unwrap().start_time_ms + previous.unwrap().end_time_ms) / 2.0
        } else if previous.is_none_or(|p| p.kind != StageItemKind::Line || stage_overlap(p, item)) {
            early
        } else {
            early.max(previous.unwrap().end_time_ms.min(item.start_time_ms) - STAGE_HANDOFF_MS)
        };
        enter <= time_ms
    });
    let previous = current
        .and_then(|index| index.checked_sub(1))
        .filter(|&index| {
            let previous = get(index);
            previous.kind == StageItemKind::Line
                && stage_overlap(previous, get(index + 1))
                && time_ms < previous.end_time_ms
        });
    StageSelection { current, previous }
}

/// Plans stage entry and handoff times, keeping overlapping duet lines together.
pub fn plan_stage(items: &[StageItem], time_ms: f64) -> Vec<StageRole> {
    let selection = select_stage(items.len(), |index| items[index], time_ms);
    (0..items.len())
        .map(|index| selection.role(index))
        .collect()
}

/// Compute stage transitions without allocating temporary item/time/role vectors.
pub fn stage_selection(lyrics: &[Lyric], time_ms: f64) -> StageSelection {
    select_stage(
        lyrics.len(),
        |index| {
            let lyric = &lyrics[index];
            StageItem {
                kind: if lyric.is_instrumental() {
                    StageItemKind::Instrumental
                } else if lyric.words.trim().is_empty() {
                    StageItemKind::Blank
                } else {
                    StageItemKind::Line
                },
                start_time_ms: lyric.start_time_ms,
                end_time_ms: lyric.start_time_ms + lyric.duration_ms,
            }
        },
        time_ms,
    )
}

/// Plans the stage view while preserving timing overlaps between concurrently sung lines.
pub fn stage_roles(lyrics: &[Lyric], time_ms: f64) -> Vec<StageRole> {
    let selection = stage_selection(lyrics, time_ms);
    (0..lyrics.len())
        .map(|index| selection.role(index))
        .collect()
}

/// State changes for the existing 50ms line-sync word stagger and 330ms highlight.
/// Visual interpolation still follows the song clock directly in Slint.
pub fn line_sync_phase(lyric: &Lyric, time_ms: f64) -> (usize, usize) {
    let count = lyric.words.split_whitespace().count();
    let elapsed = time_ms - lyric.start_time_ms;
    let crossed = |delay| {
        if elapsed < delay {
            0
        } else {
            (((elapsed - delay) / 50.0).floor() as usize).saturating_add(1)
        }
        .min(count)
    };
    (crossed(0.0), crossed(330.0))
}

pub fn seek_time(lyric: &Lyric, part_index: Option<usize>) -> f64 {
    part_index
        .and_then(|i| lyric.parts.as_ref()?.get(i))
        .map_or(lyric.start_time_ms, |p| p.start_time_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use braccato_types::LyricPart;

    #[test]
    fn line_sync_change_key_tracks_only_real_stagger_boundaries_in_both_directions() {
        let lyric = Lyric {
            words: "one two three".into(),
            start_time_ms: 1000.0,
            duration_ms: 5000.0,
            ..Default::default()
        };
        for time in [
            999.0, 1000.0, 1049.0, 1050.0, 1100.0, 1329.0, 1330.0, 1380.0, 1430.0, 5000.0, 1380.0,
            1050.0, 999.0,
        ] {
            let (started, finished) = line_sync_phase(&lyric, time);
            for index in 0..3 {
                let start = 1000.0 + index as f64 * 50.0;
                assert_eq!(
                    index < started && index >= finished,
                    time >= start && time < start + 330.0
                );
            }
        }
        assert_eq!(
            line_sync_phase(&lyric, 2000.0),
            line_sync_phase(&lyric, 5000.0)
        );
        assert_eq!(line_sync_phase(&lyric, f64::MAX), (3, 3));
    }
    fn fixture() -> Vec<Lyric> {
        vec![Lyric {
            start_time_ms: 1000.0,
            duration_ms: 1000.0,
            words: "hello world".into(),
            parts: Some(vec![
                LyricPart {
                    start_time_ms: 1000.0,
                    duration_ms: 500.0,
                    words: "hello".into(),
                    ..Default::default()
                },
                LyricPart {
                    start_time_ms: 1500.0,
                    duration_ms: 500.0,
                    words: " world".into(),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }]
    }
    #[test]
    fn tracks_active_word_and_line() {
        let state = frame(
            &fixture(),
            Playback {
                time_ms: 1700.0,
                playing: true,
                rate: 1.0,
                ..Default::default()
            },
        );
        assert_eq!(state.line, Some(0));
        assert_eq!(state.part, Some(1));
        assert_eq!(state.sync_type, LyricSyncType::Richsync);
    }

    #[test]
    fn word_sync_switches_on_timing_boundaries_without_guessing() {
        let lyrics = fixture();
        let before = frame(
            &lyrics,
            Playback {
                time_ms: 1499.0,
                playing: true,
                rate: 1.0,
                ..Default::default()
            },
        );
        let boundary = frame(
            &lyrics,
            Playback {
                time_ms: 1500.0,
                playing: true,
                rate: 1.0,
                ..Default::default()
            },
        );
        let after_last_word = frame(
            &lyrics,
            Playback {
                time_ms: 2000.0,
                playing: true,
                rate: 1.0,
                ..Default::default()
            },
        );
        assert_eq!(before.part, Some(0));
        assert_eq!(boundary.part, Some(1));
        assert_eq!(after_last_word.line, None);
        assert_eq!(after_last_word.part, None);
    }
    #[test]
    fn line_click_seeks_to_line_or_word() {
        let l = &fixture()[0];
        assert_eq!(seek_time(l, None), 1000.0);
        assert_eq!(seek_time(l, Some(1)), 1500.0);
    }
    #[test]
    fn pauses_do_not_change_the_sampled_frame() {
        let a = frame(
            &fixture(),
            Playback {
                time_ms: 1200.0,
                playing: false,
                ..Default::default()
            },
        );
        assert_eq!(a.line, Some(0));
    }

    #[test]
    fn stage_enters_early_and_keeps_overlapping_lines_together() {
        let items = [
            StageItem {
                kind: StageItemKind::Line,
                start_time_ms: 1_000.0,
                end_time_ms: 3_000.0,
            },
            StageItem {
                kind: StageItemKind::Line,
                start_time_ms: 2_600.0,
                end_time_ms: 4_000.0,
            },
        ];
        assert_eq!(
            plan_stage(&items, 2_500.0),
            vec![StageRole::Previous, StageRole::Current]
        );
        assert_eq!(
            plan_stage(&items, 3_100.0),
            vec![StageRole::Gone, StageRole::Current]
        );
        assert_eq!(
            plan_stage(&items, 400.0),
            vec![StageRole::Queued, StageRole::Queued]
        );
    }
}
