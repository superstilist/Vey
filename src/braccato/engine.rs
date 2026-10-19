//! Native port of Braccato's clock, word states, overlap selection and scroll policy.
//! Reference: better-lyrics/braccato@1e6b695 (MIT), packages/core/src/engine.ts.
use super::types::Lyric;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordState {
    Upcoming,
    Active,
    Past,
}
pub fn word_state(start: i64, duration: i64, next: i64, line_end: i64, time: i64) -> WordState {
    let end = if duration > 0 {
        start.saturating_add(duration)
    } else {
        next.max(start).min(line_end)
    };
    if time < start {
        WordState::Upcoming
    } else if time < end {
        WordState::Active
    } else {
        WordState::Past
    }
}
pub fn word_progress(start: i64, duration: i64, time: i64) -> f32 {
    if time < start {
        0.
    } else if duration <= 0 {
        1.
    } else {
        ((time - start) as f64 / duration as f64).clamp(0., 1.) as f32
    }
}

/// Sorted starts and prefix maximum ends form an interval index. A frame visits
/// only the suffix that can overlap it, rather than cloning every UI row.
#[derive(Default, Debug)]
pub struct Timeline {
    starts: Vec<i64>,
    ends: Vec<i64>,
    max_ends: Vec<i64>,
}
impl Timeline {
    pub fn new(lyrics: &[Lyric]) -> Self {
        Self::from_spans(lyrics.iter().map(|l| {
            (
                l.start_time_ms,
                l.start_time_ms.saturating_add(l.duration_ms),
            )
        }))
    }
    pub fn from_spans(spans: impl IntoIterator<Item = (i64, i64)>) -> Self {
        let mut index = Self::default();
        let mut max_end = i64::MIN;
        for (start, end) in spans {
            debug_assert!(index.starts.last().is_none_or(|last| *last <= start));
            max_end = max_end.max(end);
            index.starts.push(start);
            index.ends.push(end);
            index.max_ends.push(max_end);
        }
        index
    }
    pub fn active_into(&self, time: i64, out: &mut Vec<usize>) {
        out.clear();
        let mut i = self.starts.partition_point(|start| *start <= time);
        while i > 0 {
            i -= 1;
            if self.max_ends[i] <= time {
                break;
            }
            if time < self.ends[i] {
                out.push(i);
            }
        }
        out.reverse();
    }
    pub fn latest_started(&self, time: i64) -> Option<usize> {
        self.starts.partition_point(|s| *s <= time).checked_sub(1)
    }
    pub fn anchor(&self, time: i64, active: &[usize]) -> Option<usize> {
        active.last().copied().or_else(|| self.latest_started(time))
    }
    pub fn lookahead_end(&self, time: i64, lead: i64) -> usize {
        self.starts
            .partition_point(|s| *s <= time.saturating_add(lead))
    }
    pub fn len(&self) -> usize {
        self.starts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TickOptions {
    pub playing: bool,
    pub playback_rate: f64,
    pub global_offset_ms: i64,
    pub lyric_offset_ms: i64,
    pub richsync_offset_ms: i64,
    pub line_offset_ms: i64,
    pub richsync: bool,
    pub visible: bool,
}
impl Default for TickOptions {
    fn default() -> Self {
        Self {
            playing: false,
            playback_rate: 1.,
            global_offset_ms: 0,
            lyric_offset_ms: 0,
            richsync_offset_ms: 0,
            line_offset_ms: 0,
            richsync: false,
            visible: true,
        }
    }
}
impl TickOptions {
    pub fn adjusted(&self, time: i64) -> i64 {
        time.saturating_sub(self.global_offset_ms)
            .saturating_sub(self.lyric_offset_ms)
            .saturating_sub(if self.richsync {
                self.richsync_offset_ms
            } else {
                self.line_offset_ms
            })
    }
}

/// Playback samples arrive at 10Hz; the view interpolates between them. Pauses,
/// seeks, speed changes and non-finite media durations are explicit boundaries.
#[derive(Default, Debug)]
pub struct PlaybackClock {
    sample: f64,
    wall: f64,
    rate: f64,
    playing: bool,
    initialized: bool,
}
impl PlaybackClock {
    pub fn sample(&mut self, seconds: f64, wall_ms: f64, playing: bool, rate: f64) {
        if !seconds.is_finite() || !wall_ms.is_finite() {
            return;
        }
        if !self.initialized
            || seconds != self.sample
            || playing != self.playing
            || rate != self.rate
        {
            self.sample = seconds.max(0.);
            self.wall = wall_ms;
            self.playing = playing;
            self.rate = if rate.is_finite() && rate > 0. {
                rate
            } else {
                1.
            };
            self.initialized = true;
        }
    }
    pub fn position_ms(&self, wall_ms: f64, duration_ms: i64) -> i64 {
        let elapsed = if self.playing {
            (wall_ms - self.wall).clamp(0., 250.)
        } else {
            0.
        };
        let value = (self.sample * 1000. + elapsed * self.rate).max(0.) as i64;
        if duration_ms > 0 {
            value.min(duration_ms)
        } else {
            value
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrollPolicy {
    pub target_ratio: f32,
    pub lookahead_ms: i64,
    pub manual_resume_ms: i64,
    pub passive_resume_ms: i64,
}
impl Default for ScrollPolicy {
    fn default() -> Self {
        Self {
            target_ratio: 0.37,
            lookahead_ms: 540,
            manual_resume_ms: 25000,
            passive_resume_ms: 5000,
        }
    }
}
#[derive(Default, Debug)]
pub struct ScrollState {
    until: i64,
    committed_end: usize,
    last_time: Option<i64>,
    last_anchor: Option<usize>,
}
impl ScrollState {
    pub fn note_user_scroll(&mut self, wall: i64, plain: bool, policy: &ScrollPolicy) {
        self.until = wall.saturating_add(if plain {
            policy.passive_resume_ms
        } else {
            policy.manual_resume_ms
        });
    }
    pub fn resume(&mut self) {
        self.until = 0;
        self.committed_end = 0;
        self.last_anchor = None;
    }
    pub fn user_scrolling(&self, wall: i64) -> bool {
        wall < self.until
    }
    pub fn anchor_changed(
        &mut self,
        index: &Timeline,
        active: &[usize],
        time: i64,
        wall: i64,
        policy: &ScrollPolicy,
    ) -> Option<usize> {
        let jumped = self
            .last_time
            .is_some_and(|last| time < last || time - last > 500);
        self.last_time = Some(time);
        if self.user_scrolling(wall) {
            return None;
        }
        let anchor = index.anchor(time, active)?;
        if jumped
            || self.last_anchor != Some(anchor)
                && (anchor >= self.committed_end || self.last_anchor.is_none())
        {
            self.committed_end = index.lookahead_end(time, policy.lookahead_ms);
            self.last_anchor = Some(anchor);
            Some(anchor)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct LineGeometry {
    pub top: f32,
    pub height: f32,
}
/// Upstream's active-band solver: the latest sung line wins when a duet's
/// combined height exceeds the available viewport.
pub fn scroll_top(
    lines: &[LineGeometry],
    anchor: LineGeometry,
    viewport: f32,
    top_inset: f32,
    bottom_inset: f32,
    ratio: f32,
) -> f32 {
    if lines.is_empty() {
        return 0.;
    }
    let first = lines[0];
    let last = lines[lines.len() - 1];
    let bottom = viewport - bottom_inset;
    let mean = lines.iter().map(|l| l.top + l.height / 2.).sum::<f32>() / lines.len() as f32;
    (mean - viewport * ratio.clamp(0., 1.))
        .max(last.top + last.height - bottom)
        .min(last.top - top_inset)
        .min(first.top - top_inset)
        .max(anchor.top + anchor.height - bottom)
        .min(anchor.top - top_inset)
        .max(0.)
}

/// Unsynced lyrics use a wall-clock cycle, never invented song timestamps.
#[derive(Default, Debug)]
pub struct PassiveScroll {
    elapsed: f64,
    last_wall: Option<f64>,
}
impl PassiveScroll {
    pub fn tick(
        &mut self,
        wall_ms: f64,
        playing: bool,
        user_scrolling: bool,
        lines: usize,
        max_scroll: f32,
        seconds_per_line: f32,
    ) -> f32 {
        if let Some(last) = self.last_wall {
            if playing && !user_scrolling {
                self.elapsed += (wall_ms - last).clamp(0., 250.);
            }
        }
        self.last_wall = Some(wall_ms);
        let travel = (lines.max(1) as f64 * seconds_per_line.max(0.1) as f64 * 1000.).max(1.);
        let cycle = 800. + travel + 1500. + 600.;
        let t = self.elapsed % cycle;
        if t < 800. {
            0.
        } else if t < 800. + travel {
            max_scroll * ((t - 800.) / travel) as f32
        } else if t < 800. + travel + 1500. {
            max_scroll
        } else {
            max_scroll * (1. - ((t - 800. - travel - 1500.) / 600.) as f32)
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
/// Alt-click resolution consumes host-measured word rectangles, just as the
/// upstream host adapter consumes DOM rectangles. Times remain in milliseconds.
pub fn seek_from_click(
    line_time: i64,
    richsync: bool,
    alt: bool,
    point: (f32, f32),
    words: &[(i64, Rect)],
) -> i64 {
    if !richsync || !alt {
        return line_time;
    }
    words
        .iter()
        .min_by(|(_, a), (_, b)| {
            let distance = |r: &Rect| {
                (point.0 - r.x - r.width / 2.).powi(2) + (point.1 - r.y - r.height / 2.).powi(2)
            };
            distance(a).total_cmp(&distance(b))
        })
        .map(|(time, _)| *time)
        .unwrap_or(line_time)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageKind {
    Line,
    Blank,
    Instrumental,
    Credits,
}
#[derive(Clone, Copy, Debug)]
pub struct StageItem {
    pub start: i64,
    pub end: i64,
    pub kind: StageKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageRole {
    Current,
    Previous,
    Queued,
    Gone,
}
pub fn stage_enter_times(items: &[StageItem]) -> Vec<i64> {
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let early = item.start - 500;
            match i.checked_sub(1).map(|p| items[p]) {
                Some(prev)
                    if item.kind == StageKind::Credits && prev.kind == StageKind::Instrumental =>
                {
                    prev.start + (prev.end - prev.start) / 2
                }
                Some(prev)
                    if prev.kind == StageKind::Line
                        && !(item.kind != StageKind::Blank && item.start < prev.end - 150) =>
                {
                    early.max(prev.end.min(item.start) - 120)
                }
                _ => early,
            }
        })
        .collect()
}
pub fn plan_stage(items: &[StageItem], enter: &[i64], time: i64, out: &mut Vec<StageRole>) {
    out.clear();
    let current = enter.iter().rposition(|e| *e <= time);
    out.extend((0..items.len()).map(|i| match current {
        Some(c) if i < c => StageRole::Gone,
        Some(c) if i == c => StageRole::Current,
        _ => StageRole::Queued,
    }));
    if let Some(c) = current.filter(|c| *c > 0) {
        let prev = items[c - 1];
        if prev.kind == StageKind::Line
            && items[c].kind != StageKind::Blank
            && items[c].start < prev.end - 150
            && time < prev.end
        {
            out[c - 1] = StageRole::Previous;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn word_boundaries_and_zero_duration_are_seek_safe() {
        assert_eq!(word_state(0, 0, 100, 1000, 50), WordState::Active);
        assert_eq!(word_state(0, 0, 100, 1000, 100), WordState::Past);
        assert_eq!(word_state(500, 500, 1000, 1000, 200), WordState::Upcoming);
        assert_eq!(word_progress(500, 1000, 1000), 0.5);
    }
    #[test]
    fn interval_index_preserves_duets_and_long_held_notes() {
        let index = Timeline::from_spans([(0, 10000), (1000, 1500), (2000, 3000), (2000, 4000)]);
        let mut active = vec![];
        index.active_into(2500, &mut active);
        assert_eq!(active, [0, 2, 3]);
        index.active_into(8000, &mut active);
        assert_eq!(active, [0]);
        index.active_into(10000, &mut active);
        assert!(active.is_empty());
    }
    #[test]
    fn clock_interpolates_rate_and_stops_at_pause_and_seek() {
        let mut c = PlaybackClock::default();
        c.sample(1., 0., true, 2.);
        assert_eq!(c.position_ms(100., 10000), 1200);
        c.sample(1.2, 100., false, 2.);
        assert_eq!(c.position_ms(400., 10000), 1200);
        c.sample(9., 400., true, 1.);
        assert_eq!(c.position_ms(450., 10000), 9050);
    }
    #[test]
    fn user_scroll_timeout_and_lookahead_groups() {
        let index = Timeline::from_spans([(0, 1000), (400, 2000), (3000, 4000)]);
        let p = ScrollPolicy::default();
        let mut s = ScrollState::default();
        assert_eq!(s.anchor_changed(&index, &[0], 0, 0, &p), Some(0));
        assert_eq!(s.anchor_changed(&index, &[0, 1], 400, 400, &p), None);
        s.note_user_scroll(500, false, &p);
        assert!(s.user_scrolling(25499));
        assert!(!s.user_scrolling(25500));
        s.resume();
        assert_eq!(s.anchor_changed(&index, &[2], 3100, 26000, &p), Some(2));
    }
    #[test]
    fn stage_keeps_overlapping_previous_line() {
        let items = [
            StageItem {
                start: 0,
                end: 3000,
                kind: StageKind::Line,
            },
            StageItem {
                start: 2000,
                end: 4000,
                kind: StageKind::Line,
            },
        ];
        let enter = stage_enter_times(&items);
        let mut roles = vec![];
        plan_stage(&items, &enter, 2100, &mut roles);
        assert_eq!(roles, [StageRole::Previous, StageRole::Current]);
        plan_stage(&items, &enter, 3100, &mut roles);
        assert_eq!(roles, [StageRole::Gone, StageRole::Current]);
    }
}
