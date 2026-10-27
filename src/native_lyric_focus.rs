//! Bounded, lazy inactive-line paint. Never rasterize a whole song on a frame.
use crate::native_lyric_masks::{Job, MaskWorker};
use crate::BraccatoRow;
use slint::{Model, ModelRc};

#[derive(Default)]
struct Blend {
    value: f32,
    from: f32,
    target: f32,
    started: f64,
}
impl Blend {
    fn tick(&mut self, target: f32, now: f64, snap: bool) -> f32 {
        if snap {
            self.value = target;
            self.from = target;
            self.target = target;
            self.started = now;
        } else {
            // Sample the old trajectory before retargeting, including a reversal.
            let duration = if self.target > self.from { 280. } else { 180. };
            let t = ((now - self.started) / duration).clamp(0., 1.) as f32;
            self.value = self.from + (self.target - self.from) * t * t * (3. - 2. * t);
            if target != self.target {
                self.from = self.value;
                self.target = target;
                self.started = now;
            }
        }
        self.value
    }
}

#[derive(Default)]
pub struct FocusPaint {
    key: Option<(usize, usize, bool)>,
    near: Vec<usize>,
    painted: Vec<(usize, u16)>,
    cursor: usize,
    generation: u64,
    worker: Option<MaskWorker>,
    in_flight: bool,
    builds: usize,
    blends: Vec<(usize, Blend)>,
    cleared: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_frames_change_only_float_cells_and_manual_scroll_restores_sharp_ink() {
        use slint::{Rgba8Pixel, SharedPixelBuffer, VecModel};
        use std::rc::Rc;
        let rows: ModelRc<BraccatoRow> = Rc::new(VecModel::from(vec![BraccatoRow {
            sustain_blurred: slint::Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::new(1, 1)),
            ..Default::default()
        }]))
        .into();
        let values: ModelRc<f32> = Rc::new(VecModel::from(vec![0.])).into();
        let mut focus = FocusPaint::default();
        focus.sync(&rows, 0, 1, true);
        focus.painted.push((0, 300));
        focus.blends.push((0, Blend::default()));
        focus.animate(&rows, &values, 0., false);
        focus.animate(&rows, &values, 140., false);
        assert_eq!(values.row_data(0), Some(0.5));
        assert_eq!(
            rows.row_data(0).unwrap().focus_mix,
            0.,
            "frame updates must not rewrite the word-bearing row"
        );
        focus.sync(&rows, 0, 1, false);
        focus.animate(&rows, &values, 150., false);
        assert_eq!(values.row_data(0), Some(0.));
    }
    use slint::VecModel;
    use std::rc::Rc;
    #[test]
    fn focus_blend_survives_retarget_and_snaps_for_seek_or_reduced_motion() {
        let mut blend = Blend::default();
        assert_eq!(blend.tick(1., 100., false), 0.);
        assert!((blend.tick(1., 240., false) - 0.5).abs() < 0.0001);
        // A new target begins at the sampled old position, without jumping.
        assert!((blend.tick(0., 240., false) - 0.5).abs() < 0.0001);
        assert!((blend.tick(0., 330., false) - 0.25).abs() < 0.0001);
        assert_eq!(blend.tick(0., 420., false), 0.);
        assert_eq!(blend.tick(1., 421., true), 1.);
        assert_eq!(blend.tick(0., 422., true), 0.);
        assert_eq!(blend.tick(0., 1000., false), 0.);
    }
    #[test]
    fn inactive_masks_are_bounded_cached_and_released_outside_the_view() {
        let rows: ModelRc<BraccatoRow> = Rc::new(VecModel::from(
            (0..100)
                .map(|i| BraccatoRow {
                    text: format!("Native focus line {i}").into(),
                    font_family: "Satoshi".into(),
                    source_index: i,
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        ))
        .into();
        let mut focus = FocusPaint::default();
        focus.sync(&rows, 0, 0, true);
        while focus.pending() {
            focus.poll(&rows);
            focus.prepare_one(&rows, 48., 700, 2.);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(rows.row_data(1).unwrap().sustain_blurred.size().width > 0);
        let builds = focus.builds;
        for _ in 0..100 {
            focus.sync(&rows, 0, 0, true);
            assert!(!focus.pending());
        }
        assert_eq!(focus.builds, builds);
        focus.sync(&rows, 80, 80, true);
        assert_eq!(rows.row_data(1).unwrap().sustain_blurred.size().width, 0);
        assert!(focus.near.len() <= 14);
        focus.sync(&rows, 80, 80, false);
        assert!(!focus.pending());
    }
}
impl FocusPaint {
    pub fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.key = None;
        self.near.clear();
        self.painted.clear();
        self.cursor = 0;
        self.blends.clear();
        self.cleared.clear();
    }
    pub fn poll(&mut self, rows: &ModelRc<BraccatoRow>) {
        let Some(result) = self.worker.as_ref().and_then(MaskWorker::take) else {
            return;
        };
        self.in_flight = false;
        if result.generation != self.generation || !self.near.contains(&result.index) {
            return;
        }
        let Some(mut row) = rows.row_data(result.index) else {
            return;
        };
        if result.width > 0 && result.height > 0 {
            let mut pixels =
                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(result.width, result.height);
            pixels.make_mut_bytes().copy_from_slice(&result.pixels);
            row.sustain_blurred = slint::Image::from_rgba8(pixels);
        } else {
            row.sustain_blurred = Default::default();
        }
        row.sustain_blurred_width = result.width as f32 * result.size / 64.;
        row.sustain_blurred_height = result.height as f32 * result.size / 64.;
        row.sustain_blurred_padding = result.padding * result.size / 64.;
        rows.set_row_data(result.index, row);
        self.painted.retain(|(i, _)| *i != result.index);
        self.painted.push((result.index, result.key));
        self.builds += 1;
    }
    pub fn sync(
        &mut self,
        rows: &ModelRc<BraccatoRow>,
        visual: usize,
        source: usize,
        enabled: bool,
    ) {
        let key = (visual, source, enabled);
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.cursor = 0;
        let range = if enabled {
            visual.saturating_sub(5)..(visual + 9).min(rows.row_count())
        } else {
            0..0
        };
        for &(index, _) in &self.painted {
            if !range.contains(&index) {
                if let Some(mut row) = rows.row_data(index) {
                    row.sustain_blurred = Default::default();
                    row.sustain_blurred_width = 0.;
                    row.sustain_blurred_height = 0.;
                    row.focus_mix = 0.;
                    rows.set_row_data(index, row);
                    self.cleared.push(index);
                }
            }
        }
        self.painted.retain(|(i, _)| range.contains(i));
        self.blends.retain(|(i, _)| range.contains(i));
        self.near.clear();
        for index in range {
            let Some(mut row) = rows.row_data(index) else {
                continue;
            };
            let distance = (row.source_index as usize).abs_diff(source).min(4) as i32;
            if row.sustain_distance != distance {
                row.sustain_distance = distance;
                rows.set_row_data(index, row.clone());
            }
            if !row.instrumental {
                self.near.push(index);
            }
        }
        // Paint the next line first; it is the main visual handoff in Braccato.
        self.near.sort_by_key(|i| {
            (
                rows.row_data(*i)
                    .is_some_and(|r| r.source_index as usize == source),
                i.abs_diff(visual),
            )
        });
    }
    pub fn prepare_one(
        &mut self,
        rows: &ModelRc<BraccatoRow>,
        size: f32,
        weight: u16,
        blur: f32,
    ) -> bool {
        if self.in_flight {
            return false;
        }
        while self.cursor < self.near.len() {
            let index = self.near[self.cursor];
            self.cursor += 1;
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            let radius =
                [3., 3., 4.5, 6., 7.][row.sustain_distance as usize] * size / 48. * blur / 2.;
            let key = (radius * 100.).round() as u16;
            if self.painted.contains(&(index, key)) {
                continue;
            }
            let background = row.parts.row_data(0).is_some_and(|p| p.background);
            let font_size = size
                * if row.secondary {
                    0.48
                } else if background {
                    0.72
                } else {
                    1.
                };
            self.in_flight = self.worker.get_or_insert_with(MaskWorker::new).submit(Job {
                index,
                key,
                generation: self.generation,
                text: row.text.to_string(),
                family: row.font_family.to_string(),
                weight: if background { 500 } else { weight },
                size: font_size,
                radius,
            });
            return self.in_flight;
        }
        false
    }
    pub fn pending(&self) -> bool {
        self.cursor < self.near.len() || self.in_flight
    }
    /// Retained frame state survives ordinary VecModel row/word updates. No
    /// rasterization, source reads or allocation once the nearby band is warm.
    pub fn animate(
        &mut self,
        rows: &ModelRc<BraccatoRow>,
        values: &ModelRc<f32>,
        now: f64,
        snap: bool,
    ) {
        for index in self.cleared.drain(..) {
            values.set_row_data(index, 0.);
        }
        for &index in &self.near {
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            let target = if !row.active
                && row.sustain_distance > 0
                && row.sustain_blurred.size().width > 0
            {
                1.
            } else {
                0.
            };
            let position = match self.blends.iter().position(|(i, _)| *i == index) {
                Some(i) => i,
                None => {
                    self.blends.push((
                        index,
                        Blend {
                            value: target,
                            from: target,
                            target,
                            started: now,
                        },
                    ));
                    self.blends.len() - 1
                }
            };
            let value = self.blends[position].1.tick(target, now, snap);
            if values.row_data(index) != Some(value) {
                values.set_row_data(index, value);
            }
        }
    }
}
