//! Native translation of upstream engine.ts computeLetterSwipeWindows/planLetterMaskSweep.
//! Source: https://github.com/better-lyrics/braccato/blob/master/packages/core/src/engine.ts

#[derive(Clone, Copy, Debug)]
pub struct SwipeRamp {
    pub start_from: f32,
    pub end_from: f32,
    pub start_to: f32,
    pub end_to: f32,
}

impl Default for SwipeRamp {
    fn default() -> Self {
        Self {
            start_from: -0.2,
            end_from: -0.1,
            start_to: 1.4,
            end_to: 1.5,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LetterSweep {
    pub delay_ms: f32,
    pub duration_ms: f32,
    /// Opaque edge in this letter's content-box coordinates, held before/after its window.
    pub from: f32,
    pub to: f32,
    /// inject.ts sets --mask-fade so its width is exactly 0.1 * letter_count.
    pub feather: f32,
}

/// Geometry is indexed by grapheme count, not measured glyph advances. The latter
/// position the text but cannot determine when a letter's mask or float crests.
pub fn plan_letter_mask_sweep(ramp: SwipeRamp, count: usize, duration_ms: f32) -> Vec<LetterSweep> {
    let values = [
        ramp.start_from,
        ramp.end_from,
        ramp.start_to,
        ramp.end_to,
        duration_ms,
    ];
    if count == 0
        || values.iter().any(|v| !v.is_finite())
        || duration_ms <= 0.0
        || ramp.start_to <= ramp.start_from
        || ramp.end_to <= ramp.end_from
    {
        return Vec::new();
    }
    let n = count as f32;
    (0..count)
        .map(|index| {
            let i = index as f32;
            let begin = (duration_ms * (i / n - ramp.end_from) / (ramp.end_to - ramp.end_from))
                .clamp(0.0, duration_ms);
            let finish = (duration_ms * ((i + 1.0) / n - ramp.start_from)
                / (ramp.start_to - ramp.start_from))
                .clamp(0.0, duration_ms);
            let start_at =
                |time| ramp.start_from + (ramp.start_to - ramp.start_from) * time / duration_ms;
            LetterSweep {
                delay_ms: begin,
                duration_ms: (finish - begin).max(1.0),
                from: start_at(begin) * n - i,
                to: start_at(finish) * n - i,
                feather: 0.1 * n,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_five_letter_windows_hold_empty_then_fully_lit() {
        let windows = plan_letter_mask_sweep(SwipeRamp::default(), 5, 1000.0);
        for (index, sweep) in windows.iter().enumerate() {
            assert!((sweep.delay_ms - (62.5 + index as f32 * 125.0)).abs() < 0.001);
            assert!((sweep.duration_ms - 187.5).abs() < 0.001);
            assert!((sweep.from + 0.5).abs() < 0.001);
            assert!((sweep.to - 1.0).abs() < 0.001);
            assert_eq!(sweep.feather, 0.5);
            assert!(sweep.from + sweep.feather < 0.001);
            assert!(sweep.to >= 1.0 - 0.001);
        }
    }

    #[test]
    fn upstream_mask_position_formula_matches_native_local_edges() {
        for n in [1, 5, 18] {
            for (i, sweep) in plan_letter_mask_sweep(SwipeRamp::default(), n, 12800.0)
                .iter()
                .enumerate()
            {
                let global_start = (sweep.from + i as f32) / n as f32;
                let mask_span = (n + 2) as f32;
                let q =
                    (0.5 * mask_span - (global_start * n as f32 - i as f32)) / (mask_span - 1.0);
                let native_edge = (1.0 - mask_span) * q + 0.5 * mask_span;
                assert!((native_edge - sweep.from).abs() < 0.0001);
                // The CSS mask softness is 10*n/(n+2) percent of an (n+2)-wide mask.
                assert!((mask_span * (0.1 * n as f32 / mask_span) - sweep.feather).abs() < 0.0001);
            }
        }
    }

    #[test]
    fn invalid_ramps_or_empty_words_have_no_letter_plan() {
        assert!(plan_letter_mask_sweep(SwipeRamp::default(), 0, 1000.0).is_empty());
        assert!(plan_letter_mask_sweep(SwipeRamp::default(), 5, 0.0).is_empty());
        assert!(
            plan_letter_mask_sweep(
                SwipeRamp {
                    start_to: -0.4,
                    ..Default::default()
                },
                5,
                1000.0
            )
            .is_empty()
        );
    }
}
