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
    /// Enumerate geometry groups once, rather than discovering their layout at
    /// a cue boundary. Changes can only occur at starts, ends or policy edges.
    pub fn scroll_groups(&self, policy: &ScrollPolicy) -> Vec<Vec<usize>> {
        let mut times = Vec::with_capacity(self.len() * 4);
        for (&start, &end) in self.starts.iter().zip(&self.ends) {
            times.extend([
                start,
                end,
                start.saturating_sub(policy.lookahead_ms.max(0)),
                end.saturating_sub(policy.ending_threshold_ms.max(0)),
            ]);
        }
        times.sort_unstable();
        times.dedup();
        let mut groups = vec![];
        let mut candidates = vec![];
        let mut budget = 32768usize;
        for time in times {
            self.scroll_candidates(time, policy.lookahead_ms, &mut candidates);
            if candidates.len() > budget {
                break;
            }
            budget -= candidates.len();
            let last = candidates.last().copied();
            let group = candidates
                .iter()
                .copied()
                .filter(|i| {
                    Some(*i) == last
                        || time < self.ends[*i].saturating_sub(policy.ending_threshold_ms.max(0))
                })
                .collect::<Vec<_>>();
            if !group.is_empty() {
                groups.push(group);
            }
        }
        groups.sort_unstable();
        groups.dedup();
        groups
    }
    /// Braccato's active scroll group: running tails, the latest started line,
    /// and nearby future lines. Merely entering this window does not commit it.
    fn scroll_candidates(&self, time: i64, lead: i64, out: &mut Vec<usize>) {
        self.active_into(time, out);
        let started = self.starts.partition_point(|start| *start <= time);
        if let Some(latest) = started.checked_sub(1) {
            if out.last() != Some(&latest) {
                out.push(latest);
            }
        }
        out.extend(started..self.lookahead_end(time, lead.max(0)));
        if out.is_empty() && !self.is_empty() {
            out.push(0);
        }
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
    smoothed: f64,
    wall: f64,
    rate: f64,
    slew: f64,
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
            let predicted = self.position_ms(wall_ms, 0) as f64 / 1000.;
            let error = seconds - predicted;
            // Compare actual samples, not a late sample against extrapolated
            // presentation time. Two 100ms pollers can differ by almost 100ms
            // without the media ever seeking backwards.
            let late = self.initialized
                && playing
                && self.playing
                && rate == self.rate
                && error < 0.
                && seconds >= self.sample;
            self.smoothed = if late { predicted } else { seconds.max(0.) };
            // Correct future speed rather than moving already-painted time
            // backwards. Limit the correction so time always advances.
            self.slew = if late { error.max(-rate * 0.1) } else { 0. };
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
        let value = (self.smoothed * 1000. + elapsed * (self.rate + self.slew)).max(0.) as i64;
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
    pub ending_threshold_ms: i64,
    pub manual_resume_ms: i64,
    pub passive_resume_ms: i64,
}
impl Default for ScrollPolicy {
    fn default() -> Self {
        Self {
            target_ratio: 0.37,
            lookahead_ms: 540,
            ending_threshold_ms: 500,
            manual_resume_ms: 25000,
            passive_resume_ms: 5000,
        }
    }
}
#[derive(Default, Debug)]
pub struct ScrollState {
    until: i64,
    committed: Vec<usize>,
    candidates: Vec<usize>,
    last_time: Option<i64>,
    current_anchor: Option<usize>,
    pinned: Option<usize>,
    was_manual: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollPlan {
    pub anchor: usize,
    pub sources: Vec<usize>,
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
        self.committed.clear();
        self.last_time = None;
        self.pinned = None;
        self.was_manual = false;
    }
    pub fn user_scrolling(&self, wall: i64) -> bool {
        wall < self.until
    }
    pub fn anchor_changed(
        &mut self,
        index: &Timeline,
        _active: &[usize],
        time: i64,
        wall: i64,
        policy: &ScrollPolicy,
    ) -> Option<usize> {
        self.plan(index, time, wall, policy, false)
            .map(|plan| plan.anchor)
    }
    pub fn current_anchor(&self) -> Option<usize> {
        self.current_anchor
    }
    pub fn pin(&mut self, source: Option<usize>) {
        self.pinned = source;
    }
    pub fn pinned(&self) -> Option<usize> {
        self.pinned
    }
    pub fn plan(
        &mut self,
        index: &Timeline,
        time: i64,
        wall: i64,
        policy: &ScrollPolicy,
        force: bool,
    ) -> Option<ScrollPlan> {
        let jumped = self
            .last_time
            .is_some_and(|last| time < last || time - last > 500);
        self.last_time = Some(time);
        index.scroll_candidates(time, policy.lookahead_ms, &mut self.candidates);
        let last = self.candidates.last().copied();
        let retained = |i: usize| {
            Some(i) == last
                || time < index.ends[i].saturating_sub(policy.ending_threshold_ms.max(0))
        };
        let geometry_anchor = self
            .candidates
            .iter()
            .copied()
            .rev()
            .filter(|i| retained(*i))
            .find(|i| index.starts[*i] <= time && time < index.ends[*i])
            .or_else(|| {
                self.candidates
                    .iter()
                    .copied()
                    .rev()
                    .find(|i| retained(*i) && index.starts[*i] <= time)
            })
            .or_else(|| self.candidates.iter().copied().find(|i| retained(*i)));
        // Focus follows the scroll clock. Geometry can omit a nearly finished
        // tail to make room for the next line without advancing word timing.
        self.current_anchor = self
            .candidates
            .iter()
            .copied()
            .rev()
            .find(|i| index.starts[*i] <= time && time < index.ends[*i])
            .or_else(|| {
                self.candidates
                    .iter()
                    .copied()
                    .rev()
                    .find(|i| index.starts[*i] <= time)
            })
            .or_else(|| self.candidates.first().copied());
        if self.user_scrolling(wall) {
            self.was_manual = true;
            return None;
        }
        let resumed = std::mem::take(&mut self.was_manual);
        let anchor = geometry_anchor?;
        let new_started = self
            .candidates
            .iter()
            .any(|i| index.starts[*i] <= time && self.committed.binary_search(i).is_err());
        let unpin = self.pinned.is_some_and(|i| {
            i != anchor && (!self.candidates.contains(&i) || !retained(i) || time >= index.ends[i])
        });
        if force || jumped || resumed || self.committed.is_empty() || new_started || unpin {
            self.committed.clone_from(&self.candidates);
            Some(ScrollPlan {
                anchor,
                sources: self
                    .candidates
                    .iter()
                    .copied()
                    .filter(|i| retained(*i))
                    .collect(),
            })
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
    fn clock_slews_late_samples_without_rewinding_and_keeps_seeks_exact() {
        let mut c = PlaybackClock::default();
        c.sample(1., 0., true, 1.);
        let previous = c.position_ms(96., 0);
        c.sample(1.08, 100., true, 1.);
        assert!(c.position_ms(100., 0) >= previous);
        assert!((1098..=1100).contains(&c.position_ms(100., 0)));
        c.sample(0.5, 116., true, 1.);
        assert_eq!(c.position_ms(116., 0), 500);
        c.sample(0.51, 132., false, 1.);
        assert_eq!(c.position_ms(500., 0), 510);
    }
    #[test]
    fn out_of_phase_audio_and_ui_polling_never_rewind_presentation_time() {
        for rate in [0.5, 1., 2.] {
            let mut clock = PlaybackClock::default();
            clock.sample(31., 0., true, rate);
            let mut last = 31000;
            let mut poll = 0;
            for wall in (16..8000).step_by(16) {
                if wall / 112 > poll {
                    poll = wall / 112;
                    let audio_ms = wall / 100 * 100;
                    clock.sample(
                        31. + audio_ms as f64 / 1000. * rate,
                        wall as f64,
                        true,
                        rate,
                    );
                }
                let now = clock.position_ms(wall as f64, 300000);
                assert!(
                    now >= last,
                    "late status sample rewound the lyric clock at {wall}ms"
                );
                last = now;
            }
            clock.sample(4., 8000., true, rate);
            assert_eq!(
                clock.position_ms(8000., 300000),
                4000,
                "a real backwards seek remains exact"
            );
        }
    }
    #[test]
    fn lookahead_groups_commit_only_at_a_new_scroll_cue() {
        let index = Timeline::from_spans([(0, 1000), (400, 1500), (1200, 1800), (1600, 2300)]);
        let policy = ScrollPolicy::default();
        let mut state = ScrollState::default();
        let first = state.plan(&index, 0, 0, &policy, false).unwrap();
        assert_eq!(first.sources, vec![0, 1]);
        assert_eq!(first.anchor, 0);
        assert!(
            state.plan(&index, 400, 400, &policy, false).is_none(),
            "already included cue must not scroll again"
        );
        assert_eq!(state.current_anchor(), Some(1));
        assert!(
            state.plan(&index, 700, 700, &policy, false).is_none(),
            "entering lookahead alone cannot scroll"
        );
        assert!(state.plan(&index, 1000, 1000, &policy, false).is_none());
        let next = state.plan(&index, 1200, 1200, &policy, false).unwrap();
        assert_eq!(next.anchor, 2);
        assert_eq!(next.sources, vec![2, 3]);
        assert!(state.plan(&index, 1600, 1600, &policy, false).is_none());
    }

    #[test]
    fn pinned_overlap_hands_back_to_a_running_tail_as_soon_as_the_pin_ends() {
        let index = Timeline::from_spans([(0, 10000), (2000, 2600), (6000, 7000)]);
        let policy = ScrollPolicy::default();
        let mut state = ScrollState::default();
        assert_eq!(
            state.plan(&index, 2000, 0, &policy, false).unwrap().anchor,
            1
        );
        state.pin(Some(1));
        assert!(state.plan(&index, 2400, 400, &policy, false).is_none());
        assert_eq!(
            state
                .plan(&index, 2600, 600, &policy, false)
                .unwrap()
                .anchor,
            0
        );
    }

    #[test]
    fn seek_relayout_and_manual_timeout_recommit_without_waiting_for_a_line() {
        let index = Timeline::from_spans([(0, 4000), (4000, 8000)]);
        let policy = ScrollPolicy::default();
        let mut state = ScrollState::default();
        state.plan(&index, 4000, 0, &policy, false).unwrap();
        assert_eq!(
            state.plan(&index, 1000, 16, &policy, false).unwrap().anchor,
            0
        );
        assert_eq!(
            state.plan(&index, 1100, 32, &policy, true).unwrap().anchor,
            0
        );
        state.note_user_scroll(32, false, &policy);
        assert!(state.plan(&index, 1200, 100, &policy, false).is_none());
        assert_eq!(
            state
                .plan(&index, 1300, 25032, &policy, false)
                .unwrap()
                .anchor,
            0
        );
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
