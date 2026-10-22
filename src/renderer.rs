//! Native Braccato view driver. Only active/changed rows are written per frame.
use crate::{
    braccato::{engine::*, render::PreparedText},
    *,
};
use slint::{ComponentHandle, Model, ModelRc};
use std::{cell::RefCell, rc::Rc, time::Duration};
use web_time::Instant;
#[derive(Default)]
struct View {
    model: ModelRc<LyricRow>,
    timeline: Timeline,
    lead: Vec<PreparedText>,
    backing: Vec<PreparedText>,
    romanized: Vec<PreparedText>,
    active: Vec<usize>,
    previous: Vec<usize>,
    clock: PlaybackClock,
    scroll: ScrollState,
    stage: Vec<StageItem>,
    enter: Vec<i64>,
    stage_key: usize,
    roles: Vec<StageRole>,
    style: (u32, u32, i32, bool),
    geometry: Vec<LineGeometry>,
    geometry_dirty: bool,
    passive: PassiveScroll,
    richsync: bool,
}
pub fn install(w: &MainWindow) {
    let view = Rc::new(RefCell::new(View::default()));
    let origin = Instant::now();
    let geometry_events = Rc::new(RefCell::new(Vec::<(usize, LineGeometry)>::new()));
    let pending = geometry_events.clone();
    w.on_lyric_geometry(move |index, top, height| {
        if index >= 0 {
            pending
                .borrow_mut()
                .push((index as usize, LineGeometry { top, height }));
        }
    });
    let weak = w.as_weak();
    let state = view.clone();
    w.on_lyric_user_scroll(move || {
        if let Some(w) = weak.upgrade() {
            state.borrow_mut().scroll.note_user_scroll(
                origin.elapsed().as_millis() as i64,
                !w.get_lyrics_timed(),
                &ScrollPolicy::default(),
            );
        }
    });
    let state = view.clone();
    w.on_lyric_resume(move || {
        let mut view = state.borrow_mut();
        view.scroll.resume();
        view.geometry_dirty = true;
    });
    let weak = w.as_weak();
    let owner: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));
    let keep = owner.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(32),
        move || {
            let Some(w) = weak.upgrade() else {
                keep.borrow_mut().take();
                return;
            };
            if w.get_active_view() != "lyrics"
                || w.get_lyrics_web_ready()
                || w.get_lyrics_web_required()
            {
                return;
            }
            let mut v = view.borrow_mut();
            let model = w.get_lyric_lines();
            let plain = !w.get_lyrics_timed();
            let now = origin.elapsed().as_secs_f64() * 1000.;
            let playing = w.get_player_state() == "playing";
            v.clock
                .sample(w.get_position() as f64, now, playing, w.get_rate() as f64);
            let foreground = w.get_lyric_active();
            let background = w.get_lyric_inactive();
            let weight = w.get_lyric_weight();
            let reduced = w.get_lyrics_reduced_motion();
            let style = (
                foreground.as_argb_encoded(),
                background.as_argb_encoded(),
                weight,
                reduced,
            );
            let replaced = v.model != model;
            if replaced {
                v.model = model.clone();
                v.lead.clear();
                v.backing.clear();
                v.romanized.clear();
                v.stage.clear();
                v.previous.clear();
                v.active.clear();
                v.scroll = ScrollState::default();
                v.passive = PassiveScroll::default();
                v.richsync = false;
                let mut spans = vec![];
                for i in 0..model.row_count() {
                    if let Some(row) = model.row_data(i) {
                        spans.push((row.start_ms as i64, row.end_ms as i64));
                        v.stage.push(StageItem {
                            start: row.start_ms as i64,
                            end: row.end_ms as i64,
                            kind: if row.instrumental {
                                StageKind::Instrumental
                            } else {
                                StageKind::Line
                            },
                        });
                        let words = (0..row.words.row_count())
                            .filter_map(|n| row.words.row_data(n))
                            .collect::<Vec<_>>();
                        v.richsync |= words.iter().filter(|w| !w.background).count() > 1;
                        v.lead.push(PreparedText::new(words.clone()));
                        v.backing.push(PreparedText::new(words));
                        v.romanized.push(PreparedText::new(
                            (0..row.roman_words.row_count())
                                .filter_map(|i| row.roman_words.row_data(i)),
                        ));
                    }
                }
                v.timeline = Timeline::from_spans(spans);
                v.enter = stage_enter_times(&v.stage);
                v.stage_key = usize::MAX;
                v.geometry
                    .resize(model.row_count(), LineGeometry::default());
                v.geometry_dirty = true;
            }
            let theme_offset = if v.richsync && !w.get_lyrics_disable_richsync() {
                w.get_lyrics_richsync_offset_ms()
            } else {
                w.get_lyrics_line_offset_ms()
            };
            w.set_lyrics_effective_offset_ms(theme_offset);
            let ms = v
                .clock
                .position_ms(now, (w.get_duration() as f64 * 1000.) as i64)
                + w.get_lyrics_offset() as i64
                + theme_offset as i64;
            let restyle = replaced || v.style != style;
            v.style = style;
            let mut active = std::mem::take(&mut v.active);
            if !plain {
                v.timeline.active_into(ms, &mut active);
            } else {
                active.clear();
            }
            let anchor = v.timeline.anchor(ms, &active);
            let mut dirty = if restyle {
                (0..model.row_count()).collect::<Vec<_>>()
            } else {
                v.previous
                    .iter()
                    .chain(active.iter())
                    .copied()
                    .collect::<Vec<_>>()
            };
            let stage_mode = w.get_lyrics_stage() && !plain;
            let stage_key = if stage_mode {
                v.enter.partition_point(|t| *t <= ms)
            } else {
                usize::MAX - 1
            };
            if v.stage_key != stage_key {
                let mut roles = std::mem::take(&mut v.roles);
                plan_stage(&v.stage, &v.enter, ms, &mut roles);
                v.roles = roles;
                v.stage_key = stage_key;
                dirty.extend(0..model.row_count());
            }
            dirty.sort_unstable();
            dirty.dedup();
            for i in dirty {
                if let Some(mut row) = model.row_data(i) {
                    let prior = (
                        row.active,
                        row.stage_role,
                        v.lead[i].updates(),
                        v.backing[i].updates(),
                        v.romanized[i].updates(),
                    );
                    let on = plain || active.binary_search(&i).is_ok();
                    row.active = on;
                    row.stage_role = if stage_mode {
                        match v.roles.get(i) {
                            Some(StageRole::Current) => 2,
                            Some(StageRole::Previous) => 1,
                            _ => 0,
                        }
                    } else {
                        -1
                    };
                    row.rendered =
                        v.lead[i].render(ms, on, foreground, background, weight, false, reduced);
                    row.background_rendered =
                        v.backing[i].render(ms, on, foreground, background, 600, true, reduced);
                    row.roman_rendered =
                        v.romanized[i].render(ms, on, foreground, background, 500, false, reduced);
                    if prior
                        != (
                            row.active,
                            row.stage_role,
                            v.lead[i].updates(),
                            v.backing[i].updates(),
                            v.romanized[i].updates(),
                        )
                    {
                        model.set_row_data(i, row);
                    }
                }
            }
            for (index, geometry) in geometry_events.borrow_mut().drain(..) {
                if index < v.geometry.len() {
                    v.geometry[index] = geometry;
                    v.geometry_dirty = true;
                }
            }
            if !plain {
                let policy = ScrollPolicy {
                    target_ratio: w.get_lyrics_scroll_ratio(),
                    lookahead_ms: w.get_lyrics_lookahead_ms() as i64,
                    ..Default::default()
                };
                let changed = {
                    let View {
                        scroll, timeline, ..
                    } = &mut *v;
                    scroll.anchor_changed(timeline, &active, ms, now as i64, &policy)
                };
                if let Some(index) = changed {
                    w.set_active_lyric_index(index as i32);
                }
                if (changed.is_some() || v.geometry_dirty) && !v.scroll.user_scrolling(now as i64) {
                    if let Some(anchor) = anchor {
                        let end = v.timeline.lookahead_end(ms, policy.lookahead_ms);
                        let mut indices = active.clone();
                        indices.extend(anchor + 1..end);
                        indices.sort_unstable();
                        indices.dedup();
                        let geometry = indices
                            .iter()
                            .filter_map(|i| v.geometry.get(*i).copied())
                            .filter(|g| g.height > 0.)
                            .collect::<Vec<_>>();
                        if let Some(anchor_geometry) =
                            v.geometry.get(anchor).copied().filter(|g| g.height > 0.)
                        {
                            let target = scroll_top(
                                &geometry,
                                anchor_geometry,
                                w.get_lyrics_viewport_height(),
                                0.,
                                0.,
                                policy.target_ratio,
                            );
                            w.set_lyrics_scroll_target(target);
                        }
                    }
                    v.geometry_dirty = false;
                }
                // The anchor remains during explicit singing gaps; overlapping rows stay lit independently.
                if anchor.is_none() {
                    w.set_active_lyric_index(-1);
                }
            } else {
                w.set_active_lyric_index(-1);
                let extent = v.geometry.last().map(|g| g.top + g.height).unwrap_or(0.);
                let max_scroll = (extent - w.get_lyrics_viewport_height()).max(0.);
                let manual = v.scroll.user_scrolling(now as i64);
                let target = v.passive.tick(
                    now,
                    playing && w.get_lyrics_passive_scroll(),
                    manual,
                    model.row_count(),
                    max_scroll,
                    w.get_lyrics_passive_seconds(),
                );
                if w.get_lyrics_passive_scroll() && !manual {
                    w.set_lyrics_scroll_target(target);
                }
            }
            w.set_lyrics_user_scrolling(v.scroll.user_scrolling(now as i64));
            w.set_lyrics_position(ms.clamp(0, i32::MAX as i64) as i32);
            v.previous.clear();
            v.previous.extend_from_slice(&active);
            v.active = active;
        },
    );
    *owner.borrow_mut() = Some(timer);
}
