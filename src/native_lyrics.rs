//! Vey adapter for the vendored Rust Braccato renderer. Source cues stay canonical;
//! wrapping, glow masks and glyph plans are retained across playback samples.
use crate::{
    braccato::engine::{
        PassiveScroll, PlaybackClock, ScrollPlan, ScrollPolicy, ScrollState, Timeline,
    },
    native_lyric_focus::FocusPaint,
    native_lyric_follow::FollowIndex,
    native_lyric_fonts as fonts,
    native_lyric_paint::WordPaint,
    BraccatoGlyph, BraccatoInstrumental, BraccatoRow, BraccatoWord, LyricRow, MainWindow,
};
use braccato_slint::{LayoutOptions, Lyric, LyricPart, StageRole};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc, time::Duration};

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}

// Do not replace a sung line with a future cue. Preserve genuine vocal overlaps.
fn sung_stage(timeline: &Timeline, active: &[usize], time: i64) -> braccato_slint::StageSelection {
    let current = timeline.latest_started(time);
    let previous = active
        .iter()
        .rev()
        .copied()
        .find(|i| current.is_some_and(|c| *i < c));
    braccato_slint::StageSelection { current, previous }
}

fn part(p: crate::WordRow) -> LyricPart {
    LyricPart {
        start_time_ms: p.start_ms as f64,
        duration_ms: (p.end_ms - p.start_ms).max(0) as f64,
        words: p.text.to_string(),
        is_background: Some(p.background),
        explicit: Some(p.explicit),
    }
}

fn document(source: &ModelRc<LyricRow>, disable_words: bool) -> Vec<Lyric> {
    (0..source.row_count())
        .filter_map(|i| source.row_data(i))
        .map(|r| Lyric {
            start_time_ms: r.start_ms as f64,
            duration_ms: (r.end_ms - r.start_ms).max(0) as f64,
            words: r.text.to_string(),
            parts: (!disable_words && r.words.row_count() > 0).then(|| {
                (0..r.words.row_count())
                    .filter_map(|i| r.words.row_data(i))
                    .map(part)
                    .collect()
            }),
            agent: Some(r.agent.to_string()),
            translation: (!r.translation.is_empty()).then(|| braccato_slint::Translation {
                text: r.translation.to_string(),
                lang: "translation".into(),
            }),
            romanization: (!r.romanization.is_empty()).then(|| r.romanization.to_string()),
            timed_romanization: (!disable_words && r.roman_words.row_count() > 0).then(|| {
                (0..r.roman_words.row_count())
                    .filter_map(|i| r.roman_words.row_data(i))
                    .map(part)
                    .collect()
            }),
            is_instrumental: Some(r.instrumental),
            ..Default::default()
        })
        .collect()
}

fn word(p: braccato_slint::LyricWord) -> BraccatoWord {
    // Native Text trims trailing ASCII spaces from preferred-width. Preserve
    // supplied word boundaries in display runs without inserting syllable gaps.
    let text: slint::SharedString = p.text.replace(' ', "\u{a0}").into();
    let family = fonts::family(&text);
    BraccatoWord {
        text,
        active: false,
        background: p.background,
        start_ms: p.start_ms,
        duration_ms: p.duration_ms,
        cue_offset: p.cue_offset,
        cue_span: p.cue_span,
        motion_start_ms: p.motion_start_ms,
        motion_duration_ms: p.motion_duration_ms,
        motion_origin_em: p.motion_origin_em,
        source_part: p.source_part,
        font_family: family.into(),
        glow_wide: p.glow_wide,
        glow_tight: p.glow_tight,
        glow_padding: p.glow_padding,
        glow_levels: p.glow_levels,
        sustain_blurred: p.sustain_blurred,
        glyphs: ModelRc::<BraccatoGlyph>::default(),
    }
}

#[cfg(test)]
fn layout(
    source: &ModelRc<LyricRow>,
    lyrics: &[Lyric],
    width: f32,
    size: f32,
    stage: bool,
) -> (Vec<BraccatoRow>, Vec<f32>) {
    styled_layout(
        source,
        lyrics,
        width,
        size,
        stage,
        &crate::themes::NativeTheme::default().style(),
        700,
    )
}
fn styled_layout(
    source: &ModelRc<LyricRow>,
    lyrics: &[Lyric],
    width: f32,
    size: f32,
    stage: bool,
    style: &crate::LyricStyle,
    weight: i32,
) -> (Vec<BraccatoRow>, Vec<f32>) {
    let raw = braccato_slint::lyric_rows(lyrics, None, -1.);
    let mut normalized = vec![];
    for i in 0..raw.row_count() {
        let Some(mut row) = raw.row_data(i) else {
            continue;
        };
        let original = source.row_data(row.source_index as usize).unwrap();
        // Upstream's optional visual stagger creates 50ms word starts for line
        // lyrics. Vey explicitly does not use it. One whole-line fragment allows
        // native wrapping while keeping every fragment on the actual line cue.
        if !row.instrumental
            && (row.parts.row_count() == 0
                || !row.secondary && lyrics[row.source_index as usize].parts.is_none())
        {
            row.parts = model(vec![braccato_slint::LyricWord {
                text: row.text.clone(),
                start_ms: row.start_ms,
                source_part: -1,
                ..Default::default()
            }]);
        }
        normalized.push(row.clone());
        if !row.secondary && original.words.row_count() == 0 && !original.background_text.is_empty()
        {
            row.text = original.background_text.clone();
            row.secondary = true;
            row.parts = model(vec![braccato_slint::LyricWord {
                text: row.text.clone(),
                start_ms: row.start_ms,
                source_part: -1,
                background: true,
                ..Default::default()
            }]);
            normalized.push(row);
        }
    }
    let wrapped = braccato_slint::wrap_rows(
        &model(normalized),
        LayoutOptions {
            width: (width - if stage || size >= 40. { 80. } else { 56. }).max(80.),
            font_size: size,
            line_spacing: style.line_spacing,
            background_font_scale: 0.72,
            line_padding: if stage { 0. } else { fonts::padding(size) },
            show_translations: true,
            show_romanizations: true,
        },
        |text, size| fonts::measure_with(text, size, &style.family, weight as u16),
    );
    let mut rows = (0..wrapped.rows.row_count())
        .filter_map(|i| wrapped.rows.row_data(i))
        .map(|r| {
            let family = fonts::family_for(&r.text, &style.family);
            let source_index = r.source_index;
            let line_only = if r.romanization {
                lyrics[source_index as usize].timed_romanization.is_none()
            } else {
                lyrics[source_index as usize].parts.is_none()
            };
            BraccatoRow {
                text: r.text,
                active: false,
                secondary: r.secondary,
                translation: r.translation,
                romanization: r.romanization,
                agent: r.agent,
                instrumental: r.instrumental,
                stage_role: r.stage_role,
                source_index,
                // Text stays in a single shaping run for line sync, translations and
                // untimed romanization. No visual fragments become canonical cues.
                parts: if line_only || r.translation {
                    ModelRc::default()
                } else {
                    model(
                        (0..r.parts.row_count())
                            .filter_map(|i| r.parts.row_data(i))
                            .map(|p| {
                                let mut mapped = word(p);
                                mapped.font_family =
                                    fonts::family_for(&mapped.text, &style.family).into();
                                if r.romanization {
                                    mapped.source_part = -2 - mapped.source_part;
                                }
                                mapped
                            })
                            .collect(),
                    )
                },
                start_ms: r.start_ms,
                end_ms: r.end_ms,
                font_family: family.into(),
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
    for row in &mut rows {
        if row.parts.row_count() == 0 {
            continue;
        }
        let font_size = size
            * if row.secondary {
                0.48
            } else if row.parts.row_data(0).is_some_and(|p| p.background) {
                0.72
            } else {
                1.
            };
        let mut parts = (0..row.parts.row_count())
            .filter_map(|i| row.parts.row_data(i))
            .collect::<Vec<_>>();
        plan_word_motion(&mut parts, font_size, weight as u16);
        row.parts = model(parts);
    }
    let mut offset = 0.;
    let offsets = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let before = if i == 0 || rows[i - 1].source_index != r.source_index {
                fonts::padding(size)
            } else {
                0.
            };
            let after = if rows
                .get(i + 1)
                .is_none_or(|n| n.source_index != r.source_index)
            {
                fonts::padding(size)
            } else {
                0.
            };
            let at = offset + before;
            offset += before
                + after
                + size
                    * if r.secondary {
                        0.7
                    } else if r.parts.row_data(0).is_some_and(|p| p.background) {
                        0.95
                    } else {
                        style.line_spacing
                    };
            at
        })
        .collect();
    (rows, offsets)
}

// Presentation-only groups: separately timed syllables share one word transform.
// Canonical cue times and fill fragments are deliberately not rewritten.
fn plan_word_motion(parts: &mut [BraccatoWord], size: f32, weight: u16) {
    let boundary = |text: &str| {
        text.ends_with(|c: char| {
            c.is_whitespace() || matches!(c, '-' | '\u{2010}' | '\u{00ad}' | '\u{200b}')
        })
    };
    let mut first = 0;
    while first < parts.len() {
        if parts[first].text.chars().all(char::is_whitespace) {
            first += 1;
            continue;
        }
        let mut end = first + 1;
        while end < parts.len()
            && !boundary(&parts[end - 1].text)
            && !parts[end].text.starts_with(char::is_whitespace)
            && parts[end].background == parts[first].background
        {
            end += 1;
        }
        let group = &mut parts[first..end];
        if group.iter().all(|p| p.duration_ms > 0.) {
            let start = group
                .iter()
                .map(|p| p.start_ms + p.duration_ms * p.cue_offset)
                .fold(f32::INFINITY, f32::min);
            let until = group
                .iter()
                .map(|p| {
                    p.start_ms
                        + p.duration_ms
                            * (p.cue_offset + if p.cue_span > 0. { p.cue_span } else { 1. })
                })
                .fold(start, f32::max);
            let measure = |p: &BraccatoWord, text: &str| {
                fonts::measure_with(
                    text,
                    size,
                    &p.font_family,
                    if p.background { 500 } else { weight },
                )
            };
            let widths = group
                .iter()
                .map(|p| measure(p, &p.text))
                .collect::<Vec<_>>();
            let last = group.last().unwrap();
            let trailing = widths.last().unwrap() - measure(last, last.text.trim_end());
            let center = (widths.iter().sum::<f32>() - trailing) / 2.;
            let mut offset = 0.;
            for (part, width) in group.iter_mut().zip(widths) {
                part.motion_start_ms = start;
                part.motion_duration_ms = (until - start).max(0.);
                part.motion_origin_em = (center - offset) / size.max(1.);
                offset += width;
            }
        }
        first = end;
    }
}

/// Install once per native window; the browser retains its upstream iframe.
pub fn install(w: &MainWindow) {
    w.set_native_lyrics(true);
    w.set_lyrics_web_required(false);
    w.set_lyrics_web_ready(false);
    let frames = braccato_slint::instrumental_frames();
    w.set_native_instrumental(BraccatoInstrumental {
        silhouette: frames.silhouette,
        fill: frames.fill,
        glow: frames.glow,
        tile_size: frames.tile_size,
        columns: frames.columns,
        levels: frames.levels,
        phases: frames.phases,
    });
    let origin = web_time::Instant::now();
    let scroll = Rc::new(RefCell::new(ScrollState::default()));
    let weak = w.as_weak();
    let state = scroll.clone();
    w.on_lyric_user_scroll(move || {
        if let Some(w) = weak.upgrade() {
            state.borrow_mut().note_user_scroll(
                origin.elapsed().as_millis() as i64,
                !w.get_lyrics_timed(),
                &ScrollPolicy::default(),
            );
        }
    });
    let state = scroll.clone();
    w.on_lyric_resume(move || state.borrow_mut().resume());
    let weak = w.as_weak();
    w.on_native_lyric_seek(move |line, index| {
        if let Some(w) = weak.upgrade() {
            if !w.get_lyrics_timed() {
                return;
            }
            if let Some(row) = w.get_lyric_lines().row_data(line.max(0) as usize) {
                let cue = if index < -1 && !w.get_lyrics_disable_richsync() {
                    row.roman_words
                        .row_data((-2 - index) as usize)
                        .map_or(row.start_ms, |p| p.start_ms)
                } else if index >= 0 && !w.get_lyrics_disable_richsync() {
                    row.words
                        .row_data(index as usize)
                        .map_or(row.start_ms, |p| p.start_ms)
                } else {
                    row.start_ms
                };
                w.invoke_seek_to(
                    (cue - w.get_lyrics_offset() - w.get_lyrics_effective_offset_ms()).max(0),
                );
            }
        }
    });
    let weak = w.as_weak();
    let mut source = ModelRc::<LyricRow>::default();
    let mut timeline = Timeline::default();
    let mut clock = PlaybackClock::default();
    let mut key = (0u32, 0u32, 0, false, false, 0u64);
    let mut style_key = w.get_lyric_style();
    let mut rows = ModelRc::<BraccatoRow>::default();
    let mut offsets = vec![];
    let mut prepared = vec![];
    let mut word_paint = WordPaint::default();
    let mut active = vec![];
    let mut previous_active = vec![];
    let mut resident = Vec::<usize>::new();
    let mut dirty = Vec::<usize>::new();
    let mut warm = Vec::<usize>::new();
    let mut follow = FollowIndex::default();
    let mut has_roman = false;
    let mut stage_selection = None;
    let mut roles = vec![];
    let mut focus = FocusPaint::default();
    let mut last_ms = -1;
    let mut snap_until = 0.;
    let mut passive = PassiveScroll::default();
    let mut scroll_plan: Option<ScrollPlan> = None;
    let mut scroll_key = (0, 0, 0, 0, 0);
    let mut last_scroll_visual = None;
    let owner: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));
    let keep = owner.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(w) = weak.upgrade() else {
                keep.borrow_mut().take();
                return;
            };
            if w.get_active_view() != "lyrics"
                || w.get_player_tab() != "lyrics"
                || w.window().is_minimized()
            {
                return;
            }
            let now = origin.elapsed().as_secs_f64() * 1000.;
            let playing = w.get_player_state() == "playing";
            clock.sample(w.get_position() as f64, now, playing, w.get_rate() as f64);
            let current = w.get_lyric_lines();
            let replaced = source != current;
            if replaced {
                has_roman = (0..current.row_count()).any(|i| {
                    current
                        .row_data(i)
                        .is_some_and(|r| r.roman_words.row_count() > 0)
                });
            }
            let rich = !w.get_lyrics_disable_richsync() && (w.get_lyrics_word_timed() || has_roman);
            let shift = if rich {
                w.get_lyrics_richsync_offset_ms()
            } else {
                w.get_lyrics_line_offset_ms()
            };
            w.set_lyrics_effective_offset_ms(shift);
            let ms = clock.position_ms(now, (w.get_duration() * 1000.) as i64)
                + w.get_lyrics_offset() as i64
                + shift as i64;
            let style = w.get_lyric_style();
            let requested = w.get_lyric_size().clamp(12., 72.);
            let size = if w.get_mobile() && requested == 48. {
                32.
            } else {
                requested
            };
            let next_key = (
                w.get_lyrics_widget_width().to_bits(),
                size.to_bits(),
                w.get_lyric_weight(),
                w.get_lyrics_stage(),
                !rich,
                fonts::revision(),
            );
            let rebuilt = replaced || key != next_key || style_key != style;
            if rebuilt || last_ms >= 0 && (ms < last_ms || ms - last_ms > 500) {
                snap_until = now + 200.;
            }
            w.set_native_scroll_snap(now < snap_until);
            if rebuilt {
                source = w.get_lyric_lines();
                key = next_key;
                style_key = style.clone();
                let lyrics = document(&source, !rich);
                timeline = Timeline::new(
                    &lyrics
                        .iter()
                        .map(|l| crate::braccato::types::Lyric {
                            start_time_ms: l.start_time_ms as i64,
                            duration_ms: l.duration_ms as i64,
                            ..Default::default()
                        })
                        .collect::<Vec<_>>(),
                );
                let (new_rows, new_offsets) = styled_layout(
                    &source,
                    &lyrics,
                    w.get_lyrics_widget_width(),
                    size,
                    w.get_lyrics_stage(),
                    &style,
                    w.get_lyric_weight(),
                );
                prepared = new_rows
                    .iter()
                    .map(|r| vec![false; r.parts.row_count()])
                    .collect::<Vec<_>>();
                rows = model(new_rows);
                follow = FollowIndex::new(&rows, lyrics.len());
                focus.reset();
                word_paint.reset();
                previous_active.clear();
                resident.clear();
                roles = vec![StageRole::Queued; lyrics.len()];
                stage_selection = None;
                offsets = new_offsets;
                w.set_native_lyric_lines(rows.clone());
                if replaced {
                    scroll.borrow_mut().resume();
                    passive = PassiveScroll::default();
                }
                last_ms = -1;
            }
            timeline.active_into(ms, &mut active);
            let anchor = timeline.anchor(ms, &active);
            let manual = scroll.borrow().user_scrolling(now as i64);
            let scroll_ms = FollowIndex::scroll_time(
                ms,
                playing && !w.get_lyrics_stage(),
                w.get_lyrics_reduced_motion(),
                w.get_lyrics_scroll_offset_ms(),
            );
            let lookahead = if playing && !w.get_lyrics_reduced_motion() && !w.get_lyrics_stage() {
                w.get_lyrics_lookahead_ms()
            } else {
                0
            };
            let next_scroll_key = (
                lookahead,
                w.get_lyrics_ending_threshold_ms(),
                w.get_lyrics_scroll_offset_ms(),
                w.get_lyrics_viewport_height().to_bits(),
                w.get_lyrics_scroll_ratio().to_bits(),
            );
            let policy = ScrollPolicy {
                lookahead_ms: lookahead as i64,
                ending_threshold_ms: w.get_lyrics_ending_threshold_ms() as i64,
                ..Default::default()
            };
            let plan = scroll.borrow_mut().plan(
                &timeline,
                scroll_ms,
                now as i64,
                &policy,
                rebuilt || scroll_key != next_scroll_key,
            );
            scroll_key = next_scroll_key;
            let scroll_source = scroll.borrow().current_anchor();
            let focused = anchor.and_then(|s| follow.visual(s, ms));
            focus.sync(
                &rows,
                focused.unwrap_or(0),
                anchor.unwrap_or(0),
                w.get_lyrics_timed()
                    && !w.get_lyrics_stage()
                    && !manual
                    && anchor.is_some()
                    && style.inactive_blur > 0.,
            );
            focus.poll(&rows);
            w.set_lyrics_user_scrolling(manual);
            let committed = plan.is_some();
            if let Some(plan) = plan {
                scroll_plan = Some(plan);
            }
            w.set_active_lyric_index(if w.get_lyrics_timed() {
                anchor.map_or(-1, |i| i as i32)
            } else {
                -1
            });
            if !w.get_lyrics_timed() {
                w.set_native_active_line(-1);
            } else if let Some(index) = scroll_source {
                if !manual {
                    if let Some(visual) = follow.visual(index, scroll_ms) {
                        let wrapped_pin = scroll.borrow().pinned() == Some(index)
                            && last_scroll_visual != Some(visual);
                        w.set_native_active_line(focused.map_or(-1, |i| i as i32));
                        if committed || wrapped_pin {
                            if let Some(plan) = &scroll_plan {
                                let target_source = if committed { plan.anchor } else { index };
                                let target_visual =
                                    follow.visual(target_source, scroll_ms).unwrap_or(visual);
                                let (position, overflows) = follow.target(
                                    &plan.sources,
                                    target_source,
                                    target_visual,
                                    &offsets,
                                    size,
                                    style.line_spacing,
                                    w.get_lyrics_viewport_height(),
                                    w.get_lyrics_scroll_ratio(),
                                );
                                w.set_native_scroll_position(position);
                                scroll.borrow_mut().pin(overflows.then_some(target_source));
                            }
                        }
                        last_scroll_visual = Some(visual);
                    }
                }
            } else if anchor.is_none() {
                w.set_native_active_line(-1);
                w.set_native_scroll_position(0.);
            }
            if ms != last_ms {
                let stage = if w.get_lyrics_stage() && w.get_lyrics_timed() {
                    Some(sung_stage(&timeline, &active, ms))
                } else {
                    None
                };
                let stage_changed = rebuilt || stage != stage_selection;
                if stage_changed {
                    if let Some(stage) = stage {
                        for (i, role) in roles.iter_mut().enumerate() {
                            *role = stage.role(i);
                        }
                    }
                    stage_selection = stage;
                }
                dirty.clear();
                if rebuilt || stage_changed {
                    dirty.extend(0..rows.row_count());
                } else if w.get_lyrics_timed() {
                    follow.extend_rows(&active, &mut dirty);
                    follow.extend_rows(&previous_active, &mut dirty);
                    dirty.extend(resident.iter().copied());
                }
                dirty.sort_unstable();
                dirty.dedup();
                for &i in &dirty {
                    let Some(mut row) = rows.row_data(i) else {
                        continue;
                    };
                    let index = row.source_index as usize;
                    let on = !w.get_lyrics_timed() || active.binary_search(&index).is_ok();
                    let role = match roles[index] {
                        StageRole::Current => "current",
                        StageRole::Previous => "previous",
                        StageRole::Queued => "queued",
                        StageRole::Gone => "gone",
                    };
                    if row.active != on || row.stage_role != role {
                        row.active = on;
                        row.stage_role = role.into();
                        rows.set_row_data(i, row.clone());
                    }
                    for n in 0..row.parts.row_count() {
                        let Some(mut word) = row.parts.row_data(n) else {
                            continue;
                        };
                        let sung = on
                            && ms as f32 >= word.start_ms
                            && (word.duration_ms <= 0.
                                || (ms as f32) < word.start_ms + word.duration_ms);
                        let release = prepared[i][n]
                            && (word.start_ms as i64 > ms + 2000
                                || ((word.start_ms + word.duration_ms.max(1200.)) as i64)
                                    < ms - 1500);
                        if release {
                            word.glyphs = ModelRc::default();
                            word.glow_wide = Default::default();
                            word.glow_tight = Default::default();
                            word.glow_levels = 0;
                            prepared[i][n] = false;
                        }
                        if word.active != sung || release {
                            word.active = sung;
                            row.parts.set_row_data(n, word);
                        }
                    }
                }
                w.set_lyrics_position(ms.clamp(0, i32::MAX as i64) as i32);
                previous_active.clone_from(&active);
                resident.retain(|i| prepared[*i].iter().any(|p| *p));
                last_ms = ms;
            }
            // Prepare at most one word per frame, prioritizing audible cues and then
            // the next two seconds. Raster work never rebuilds the lyric document.
            warm.clear();
            let effects = !w.get_lyrics_reduced_motion() && rich && (style.glow || style.wave);
            if let Some((i, n)) = word_paint.poll(&rows, ms, effects) {
                prepared[i][n] = true;
                if !resident.contains(&i) {
                    resident.push(i);
                }
            }
            if effects && !word_paint.busy() {
                // After a seek, prepare the visible sung fragment before old
                // words from the same long source line.
                if let Some(source) = anchor {
                    if let Some(visual) = follow.visual(source, ms) {
                        warm.push(visual);
                        if visual + 1 < follow.sources[source].end {
                            warm.push(visual + 1);
                        }
                    }
                }
                follow.extend_rows(&active, &mut warm);
                let first = timeline.latest_started(ms).map_or(0, |i| i + 1);
                for source in first..timeline.lookahead_end(ms, 2000) {
                    warm.extend(follow.sources[source].clone());
                }
            }
            'prepare: {
                for &i in &warm {
                    let Some(row) = rows.row_data(i) else {
                        continue;
                    };
                    for n in 0..row.parts.row_count() {
                        if prepared[i][n] {
                            continue;
                        }
                        let Some(word) = row.parts.row_data(n) else {
                            continue;
                        };
                        if word.duration_ms <= 0.
                            || word.start_ms as i64 > ms + 2000
                            || ((word.start_ms + word.duration_ms.max(1200.)) as i64) < ms - 1500
                        {
                            continue;
                        }
                        word_paint.submit_style(
                            &rows,
                            i,
                            n,
                            size,
                            w.get_lyric_weight() as u16,
                            &style,
                        );
                        break 'prepare;
                    }
                }
            }
            focus.animate(
                &rows,
                now,
                w.get_lyrics_reduced_motion() || now < snap_until,
            );
            if focus.pending() {
                focus.prepare_one(
                    &rows,
                    size,
                    w.get_lyric_weight() as u16,
                    style.inactive_blur,
                );
            }
            if !w.get_lyrics_timed() {
                let progress = passive.tick(
                    now,
                    playing && w.get_lyrics_passive_scroll() && !w.get_lyrics_reduced_motion(),
                    manual,
                    source.row_count(),
                    1.,
                    w.get_lyrics_passive_seconds(),
                );
                if !manual {
                    w.set_native_passive_progress(progress);
                }
            }
        },
    );
    *owner.borrow_mut() = Some(timer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::LyricsAlternative;
    #[test]
    fn word_motion_groups_syllables_without_touching_their_cues() {
        let mut parts = vec![
            BraccatoWord {
                text: "Il".into(),
                start_ms: 1000.,
                duration_ms: 200.,
                cue_span: 1.,
                ..Default::default()
            },
            BraccatoWord {
                text: "luminate\u{a0}".into(),
                start_ms: 1200.,
                duration_ms: 600.,
                cue_span: 1.,
                ..Default::default()
            },
            BraccatoWord {
                text: "next".into(),
                start_ms: 2000.,
                duration_ms: 400.,
                cue_span: 1.,
                ..Default::default()
            },
        ];
        plan_word_motion(&mut parts, 48., 700);
        for p in &parts[..2] {
            assert_eq!((p.motion_start_ms, p.motion_duration_ms), (1000., 800.));
        }
        assert_eq!(
            (parts[2].motion_start_ms, parts[2].motion_duration_ms),
            (2000., 400.)
        );
        assert!(parts[0].motion_origin_em > parts[1].motion_origin_em);
        assert_eq!((parts[0].start_ms, parts[0].duration_ms), (1000., 200.));
        assert_eq!((parts[1].start_ms, parts[1].duration_ms), (1200., 600.));
        // A continuation starts moving only as the actual fill reaches it.
        parts[0].cue_offset = 0.5;
        parts[0].cue_span = 0.5;
        parts.truncate(1);
        plan_word_motion(&mut parts, 48., 700);
        assert_eq!(
            (parts[0].motion_start_ms, parts[0].motion_duration_ms),
            (1100., 100.)
        );
    }

    #[test]
    fn display_spaces_keep_word_gaps_without_separating_syllables() {
        let a = word(braccato_slint::LyricWord {
            text: "забулися ".into(),
            start_ms: 1400.,
            ..Default::default()
        });
        let b = word(braccato_slint::LyricWord {
            text: "за".into(),
            start_ms: 1000.,
            ..Default::default()
        });
        assert_eq!(a.text, "забулися\u{a0}");
        assert_eq!(b.text, "за");
        assert_eq!(a.start_ms, 1400.);
    }

    fn imported(text: &str) -> ModelRc<LyricRow> {
        let (lyrics, format, sync_type) = crate::lyrics::parse_payload(text).unwrap();
        model(crate::braccato::rows(
            &LyricsAlternative {
                provider: "fixture".into(),
                format: format.into(),
                lyrics,
                sync_type,
            },
            12000,
        ))
    }

    #[test]
    fn line_cues_wrap_without_upstream_visual_word_stagger() {
        let source = imported("[00:01.00]A very long line with many words\n[00:04.00]Next line");
        let lyrics = document(&source, false);
        assert!(lyrics.iter().all(|l| l.parts.is_none()));
        let (rows, _) = layout(&source, &lyrics, 180., 40., false);
        let first = rows
            .iter()
            .filter(|r| r.source_index == 0)
            .collect::<Vec<_>>();
        assert!(first.len() > 1);
        assert!(first
            .iter()
            .all(|r| r.parts.row_count() == 0 && r.start_ms == 1000.));
        assert_eq!(
            first.iter().map(|r| r.text.as_str()).collect::<String>(),
            "A very long line with many words"
        );
    }

    #[test]
    fn copied_renderer_keeps_real_syllable_background_and_romanization_clocks() {
        let source=imported("<tt xmlns:ttm='http://www.w3.org/ns/ttml#metadata'><body><p begin='1s' end='4s'><span begin='1s' end='1.4s'>Sin</span><span begin='1.4s' end='2s'>gle </span><span begin='2s' end='3s'>word</span><span ttm:role='x-bg' begin='2.5s' end='3.5s'>(echo)</span></p></body></tt>");
        let lyrics = document(&source, false);
        let lead = lyrics.iter().find(|l| l.words.contains("Single")).unwrap();
        let parts = lead.parts.as_ref().unwrap();
        assert_eq!(parts[0].start_time_ms, 1000.);
        assert_eq!(parts[1].start_time_ms, 1400.);
        assert!(parts
            .iter()
            .any(|p| p.is_background == Some(true) && p.start_time_ms == 2500.));
        let (rows, _) = layout(&source, &lyrics, 220., 32., false);
        assert!(rows
            .iter()
            .flat_map(|r| (0..r.parts.row_count()).filter_map(|i| r.parts.row_data(i)))
            .any(|p| p.background && p.start_ms == 2500.));
    }

    #[test]
    fn a_background_phrase_keeps_one_cue_across_mobile_wrapping() {
        let source = imported("<tt xmlns:ttm='http://www.w3.org/ns/ttml#metadata'><body><p begin='1s' end='6s'><span begin='1s' end='3s'>Lead </span><span begin='3s' end='6s'>continues</span><span ttm:role='x-bg' begin='2s' end='5s'>(куди йти, коли дорога кличе нас додому)</span></p></body></tt>");
        let lyrics = document(&source, false);
        let backgrounds = |width| {
            let (rows, _) = layout(&source, &lyrics, width, 32., false);
            rows.into_iter()
                .flat_map(|r| {
                    (0..r.parts.row_count())
                        .filter_map(|i| r.parts.row_data(i))
                        .collect::<Vec<_>>()
                })
                .filter(|p| p.background)
                .collect::<Vec<_>>()
        };
        let wide = backgrounds(1000.);
        assert_eq!(
            wide.len(),
            1,
            "a phrase that fits stays a single shaping run"
        );
        assert_eq!(wide[0].cue_offset, 0.);
        assert!((wide[0].cue_span - 1.).abs() < 0.0001);
        let narrow = backgrounds(250.);
        assert!(narrow.len() > 1);
        let mut edge = 0.;
        for part in narrow {
            assert!((part.cue_offset - edge).abs() < 0.0001);
            edge += part.cue_span;
            assert_eq!((part.start_ms, part.duration_ms), (2000., 3000.));
            assert_eq!(part.source_part, wide[0].source_part);
        }
        assert!((edge - 1.).abs() < 0.0001);
    }

    #[test]
    fn stage_waits_for_real_cues_and_retains_real_overlaps() {
        let timeline = Timeline::from_spans([(1000, 6000), (5000, 8000)]);
        let mut active = vec![];
        for (time, current, previous) in [
            (999, None, None),
            (4999, Some(0), None),
            (5000, Some(1), Some(0)),
            (6000, Some(1), None),
        ] {
            timeline.active_into(time, &mut active);
            let selection = sung_stage(&timeline, &active, time);
            assert_eq!((selection.current, selection.previous), (current, previous));
        }
    }

    #[test]
    fn disabling_word_effects_does_not_mutate_the_source() {
        let source = imported("[00:01.00]<00:01.00>One <00:02.00>two<00:03.00>");
        assert!(document(&source, true).iter().all(|l| l.parts.is_none()));
        assert!(document(&source, false)
            .iter()
            .any(|l| l.parts.as_ref().is_some_and(|p| p.len() == 2)));
    }

    #[test]
    fn instrumental_rows_survive_wrapping_without_empty_text_fragments() {
        let source = imported("[00:08.00]After the break");
        assert!(source.row_data(0).unwrap().instrumental);
        let lyrics = document(&source, false);
        let (rows, _) = layout(&source, &lyrics, 340., 28., false);
        assert!(rows[0].instrumental);
        assert_eq!(rows[0].parts.row_count(), 0);
        assert_eq!((rows[0].start_ms, rows[0].end_ms), (0., 8000.));
    }
}
