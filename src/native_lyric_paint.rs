//! Rasterize word glows and plan glyph waves away from the native event loop.
//! A single outstanding job bounds both CPU work and the result queue.
use crate::{native_lyric_fonts, native_lyric_glyphs::GlyphPlans, BraccatoGlyph, BraccatoRow};
use slint::{Model, ModelRc, VecModel};
use std::sync::mpsc::{self, Receiver, SyncSender};

/// Keep lead-word masks for the active line's filled prefix. Background cues
/// and secondary layers keep their shorter cue-based residency.
pub(crate) fn residency_end(line: &BraccatoRow, word: &crate::BraccatoWord) -> i64 {
    let cue_end = word.start_ms + word.duration_ms.max(1200.);
    let end = if !line.secondary && !word.background {
        cue_end.max(line.end_ms)
    } else {
        cue_end
    };
    end as i64 + 1500
}

struct Job {
    generation: u64,
    row: usize,
    part: usize,
    text: String,
    family: String,
    weight: u16,
    size: f32,
    duration: f32,
    appearance: [f32; 4],
    effects: [bool; 2],
    color: [u8; 3],
}
struct Pixels {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}
impl Pixels {
    fn tint(mut self, color: [u8; 3]) -> Self {
        for pixel in self.bytes.chunks_exact_mut(4) {
            // Uniform RGB in transparent texels also avoids bilinear color
            // fringes when sampling a straight-alpha texture.
            pixel[..3].copy_from_slice(&color);
        }
        self
    }
    fn atlas(frames: &[(&slint::Image, f32)], padding: f32) -> Self {
        if frames.is_empty() {
            return Self {
                width: 0,
                height: 0,
                bytes: vec![],
            };
        }
        let first = Self::read(frames[0].0);
        let tile_width = first.width;
        let height = first.height;
        let width = tile_width * frames.len() as u32;
        let mut bytes = vec![0; width as usize * height as usize * 4];
        for (index, (image, inset)) in frames.iter().enumerate() {
            let frame = Self::read(image);
            let offset = (padding - inset).max(0.).round() as u32;
            for y in 0..frame.height.min(height.saturating_sub(offset)) {
                let count = frame.width.min(tile_width.saturating_sub(offset)) as usize * 4;
                let from = (y * frame.width) as usize * 4;
                let to = ((y + offset) * width + index as u32 * tile_width + offset) as usize * 4;
                bytes[to..to + count].copy_from_slice(&frame.bytes[from..from + count]);
            }
        }
        Self {
            width,
            height,
            bytes,
        }
    }
    fn read(image: &slint::Image) -> Self {
        image.to_rgba8().map_or(
            Self {
                width: 0,
                height: 0,
                bytes: vec![],
            },
            |p| Self {
                width: p.width(),
                height: p.height(),
                bytes: p.as_bytes().to_vec(),
            },
        )
    }
    fn upload(self) -> slint::Image {
        if self.width == 0 || self.height == 0 {
            return Default::default();
        }
        let mut pixels =
            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(self.width, self.height);
        pixels.make_mut_bytes().copy_from_slice(&self.bytes);
        slint::Image::from_rgba8(pixels)
    }
}
struct Ready {
    generation: u64,
    row: usize,
    part: usize,
    wide: Pixels,
    padding: f32,
    levels: i32,
    // Only owned strings and numbers cross threads; Slint models stay thread local.
    glyphs: Vec<(String, [f32; 12])>,
}
#[derive(Default)]
pub struct WordPaint {
    generation: u64,
    worker: Option<(SyncSender<Job>, Receiver<Ready>)>,
    pending: bool,
}
impl WordPaint {
    pub fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn busy(&self) -> bool {
        self.pending
    }
    fn start(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let (jobs, incoming) = mpsc::sync_channel::<Job>(1);
        let (done, ready) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("vey-lyric-words".into())
            .spawn(move || {
                let mut cache = native_lyric_fonts::cache();
                let mut plans = GlyphPlans::default();
                let mut font_revision = 0;
                while let Ok(job) = incoming.recv() {
                    if native_lyric_fonts::refresh_cache(&mut cache, &mut font_revision) {
                        plans = GlyphPlans::default();
                    }
                    let text: slint::SharedString = job.text.into();
                    let [requested_radius, wave, lead, duration] = job.appearance;
                    let radius = if job.effects[0] { requested_radius } else { 0. };
                    let levels = if radius <= 0. {
                        0
                    } else if radius <= 8. {
                        2
                    } else {
                        4
                    };
                    let theme = braccato_slint::LyricsTheme {
                        letter_wave_duration_ms: wave,
                        swipe_lead_ratio: lead,
                        swipe_duration_ratio: duration,
                        ..Default::default()
                    };
                    let glow = cache.word_with_radii(
                        &text,
                        &job.family,
                        job.weight,
                        radius,
                        radius * if levels == 2 { 0.5 } else { 0.75 },
                        job.size,
                    );
                    let inner = if levels == 4 {
                        Some(cache.word_with_radii(
                            &text,
                            &job.family,
                            job.weight,
                            radius * 0.5,
                            radius * 0.25,
                            job.size,
                        ))
                    } else {
                        None
                    };
                    let plan = if job.effects[1] {
                        plans.get(&text, &job.family, job.weight, job.duration, &theme, &glow)
                    } else {
                        ModelRc::default()
                    };
                    let glyphs = (0..plan.row_count())
                        .filter_map(|i| plan.row_data(i))
                        .map(|g| {
                            (
                                g.text.to_string(),
                                [
                                    g.start,
                                    g.width,
                                    g.delay_ms,
                                    g.wave_ms,
                                    g.sweep_delay_ms,
                                    g.sweep_duration_ms,
                                    g.sweep_from,
                                    g.sweep_to,
                                    g.sweep_feather,
                                    g.start_em,
                                    g.width_em,
                                    g.isolated_advance_em,
                                ],
                            )
                        })
                        .collect();
                    if done
                        .send(Ready {
                            generation: job.generation,
                            row: job.row,
                            part: job.part,
                            wide: (if let Some(inner) = inner.as_ref() {
                                Pixels::atlas(
                                    &[
                                        (&glow.wide, glow.padding),
                                        (&glow.tight, glow.padding),
                                        (&inner.wide, inner.padding),
                                        (&inner.tight, inner.padding),
                                    ],
                                    glow.padding,
                                )
                            } else if levels == 2 {
                                Pixels::atlas(
                                    &[(&glow.wide, glow.padding), (&glow.tight, glow.padding)],
                                    glow.padding,
                                )
                            } else {
                                Pixels::atlas(&[], 0.)
                            })
                            .tint(job.color),
                            padding: glow.padding,
                            levels,
                            glyphs,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("native word paint worker");
        self.worker = Some((jobs, ready));
    }
    #[cfg(test)]
    pub fn submit(
        &mut self,
        rows: &ModelRc<BraccatoRow>,
        row: usize,
        part: usize,
        size: f32,
        weight: u16,
    ) -> bool {
        self.submit_style(
            rows,
            row,
            part,
            size,
            weight,
            &crate::LyricStyle {
                wave: true,
                glow_radius: 12.8,
                ..crate::themes::NativeTheme::default().style()
            },
        )
    }
    pub fn submit_style(
        &mut self,
        rows: &ModelRc<BraccatoRow>,
        row: usize,
        part: usize,
        size: f32,
        weight: u16,
        style: &crate::LyricStyle,
    ) -> bool {
        if self.pending {
            return false;
        }
        let Some(line) = rows.row_data(row) else {
            return false;
        };
        let Some(word) = line.parts.row_data(part) else {
            return false;
        };
        self.start();
        let job = Job {
            generation: self.generation,
            row,
            part,
            text: word.text.to_string(),
            family: word.font_family.to_string(),
            weight: if word.background { 500 } else { weight },
            size: size
                * if line.secondary {
                    0.48
                } else if word.background {
                    0.72
                } else {
                    1.
                },
            duration: word.duration_ms,
            effects: [style.glow, style.wave],
            color: [
                style.glow_color.red(),
                style.glow_color.green(),
                style.glow_color.blue(),
            ],
            appearance: [
                style.glow_radius,
                style.wave_duration,
                style.swipe_lead,
                style.swipe_duration,
            ],
        };
        self.pending = self.worker.as_ref().unwrap().0.try_send(job).is_ok();
        self.pending
    }
    pub fn poll(
        &mut self,
        rows: &ModelRc<BraccatoRow>,
        ms: i64,
        enabled: bool,
    ) -> Option<(usize, usize)> {
        let result = self.worker.as_ref()?.1.try_recv().ok()?;
        self.pending = false;
        if result.generation != self.generation || !enabled {
            return None;
        }
        let line = rows.row_data(result.row)?;
        let mut word = line.parts.row_data(result.part)?;
        // A seek can finish before the worker. Do not resurrect expired glyphs.
        if word.start_ms as i64 > ms + 2000 || residency_end(&line, &word) < ms {
            return None;
        }
        word.glow_wide = result.wide.upload();
        word.glow_tight = Default::default();
        word.glow_levels = result.levels;
        word.glow_precolored = result.levels > 0;
        word.glow_padding = result.padding;
        word.glyphs = std::rc::Rc::new(VecModel::from(
            result
                .glyphs
                .into_iter()
                .map(|(text, g)| BraccatoGlyph {
                    text: text.into(),
                    start: g[0],
                    width: g[1],
                    delay_ms: g[2],
                    wave_ms: g[3],
                    sweep_delay_ms: g[4],
                    sweep_duration_ms: g[5],
                    sweep_from: g[6],
                    sweep_to: g[7],
                    sweep_feather: g[8],
                    start_em: g[9],
                    width_em: g[10],
                    isolated_advance_em: g[11],
                })
                .collect::<Vec<_>>(),
        ))
        .into();
        line.parts.set_row_data(result.part, word);
        Some((result.row, result.part))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BraccatoWord;
    use std::{
        rc::Rc,
        time::{Duration, Instant},
    };

    #[test]
    fn filled_lead_masks_live_through_the_line_but_background_cues_expire() {
        let line = BraccatoRow {
            end_ms: 9000.,
            ..Default::default()
        };
        let mut word = BraccatoWord {
            start_ms: 0.,
            duration_ms: 300.,
            ..Default::default()
        };
        assert_eq!(residency_end(&line, &word), 10500);
        word.background = true;
        assert_eq!(residency_end(&line, &word), 2700);
        word.background = false;
        assert_eq!(
            residency_end(
                &BraccatoRow {
                    secondary: true,
                    ..line
                },
                &word
            ),
            2700
        );
    }

    fn rows() -> ModelRc<BraccatoRow> {
        Rc::new(VecModel::from(vec![BraccatoRow {
            parts: Rc::new(VecModel::from(vec![BraccatoWord {
                text: "забулися".into(),
                font_family: "Noto Sans".into(),
                start_ms: 1000.,
                duration_ms: 2000.,
                ..Default::default()
            }]))
            .into(),
            ..Default::default()
        }]))
        .into()
    }
    fn finish(paint: &mut WordPaint, rows: &ModelRc<BraccatoRow>) -> Option<(usize, usize)> {
        finish_at(paint, rows, 1500, true)
    }
    fn finish_at(
        paint: &mut WordPaint,
        rows: &ModelRc<BraccatoRow>,
        ms: i64,
        enabled: bool,
    ) -> Option<(usize, usize)> {
        let start = Instant::now();
        loop {
            let result = paint.poll(rows, ms, enabled);
            if !paint.busy() {
                return result;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "word worker stalled"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn light_glow_uses_two_radii_without_letter_models() {
        let rows = rows();
        let mut worker = WordPaint::default();
        let style = crate::themes::NativeTheme::default().style();
        assert!(worker.submit_style(&rows, 0, 0, 48., 700, &style));
        assert_eq!(finish(&mut worker, &rows), Some((0, 0)));
        let word = rows.row_data(0).unwrap().parts.row_data(0).unwrap();
        assert_eq!(word.glow_levels, 2);
        assert!(word.glow_precolored);
        assert_eq!(word.glyphs.row_count(), 0);
        assert!(word.glow_wide.size().width > 0);
        let off = crate::LyricStyle {
            glow: false,
            wave: false,
            ..style
        };
        assert!(worker.submit_style(&rows, 0, 0, 48., 700, &off));
        assert_eq!(finish(&mut worker, &rows), Some((0, 0)));
        let word = rows.row_data(0).unwrap().parts.row_data(0).unwrap();
        assert_eq!(word.glow_levels, 0);
        assert!(!word.glow_precolored);
        assert_eq!(word.glow_wide.size().width, 0);
        assert_eq!(word.glyphs.row_count(), 0);
    }
    #[test]
    fn worker_preserves_glow_pixels_and_glyph_timing() {
        let rows = rows();
        let mut worker = WordPaint::default();
        assert!(worker.submit(&rows, 0, 0, 48., 700));
        assert!(!worker.submit(&rows, 0, 0, 48., 700));
        assert_eq!(finish(&mut worker, &rows), Some((0, 0)));
        let word = rows.row_data(0).unwrap().parts.row_data(0).unwrap();
        let mut cache = native_lyric_fonts::cache();
        let expected = cache.word_with_radii(&word.text, &word.font_family, 700, 12.8, 0., 48.);
        let atlas = word.glow_wide.to_rgba8().unwrap();
        let reference = expected.wide.to_rgba8().unwrap();
        assert_eq!(word.glow_levels, 4);
        assert_eq!(atlas.width(), reference.width() * 4);
        assert_eq!(atlas.height(), reference.height());
        for y in 0..atlas.height() as usize {
            let start = y * atlas.width() as usize * 4;
            let stride = reference.width() as usize * 4;
            for (paint, mask) in atlas.as_bytes()[start..start + stride]
                .chunks_exact(4)
                .zip(reference.as_bytes()[y * stride..(y + 1) * stride].chunks_exact(4))
            {
                assert_eq!(paint[3], mask[3], "precolor must preserve full mask alpha");
                if paint[3] > 0 {
                    assert_eq!(&paint[..3], &[245, 245, 245]);
                }
            }
        }
        let tile_width = reference.width() as usize;
        let peaks = (0..4)
            .map(|tile| {
                (0..atlas.height() as usize)
                    .flat_map(|y| {
                        let pixels = atlas.as_bytes();
                        (0..tile_width).map(move |x| {
                            pixels[(y * tile_width * 4 + tile * tile_width + x) * 4 + 3]
                        })
                    })
                    .max()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();
        assert!(
            peaks[3] > peaks[0],
            "tighter radii must concentrate the glow: {peaks:?}"
        );
        let glyphs = expected.letters(2000.);
        assert_eq!(word.glyphs.row_count(), glyphs.row_count());
        for i in 0..glyphs.row_count() {
            let actual = word.glyphs.row_data(i).unwrap();
            let expected = glyphs.row_data(i).unwrap();
            assert_eq!(actual.text, expected.text);
            assert_eq!(actual.delay_ms, expected.delay_ms);
            assert_eq!(actual.sweep_delay_ms, expected.sweep_delay_ms);
            assert_eq!(actual.sweep_duration_ms, expected.sweep_duration_ms);
            assert!(actual.width_em > 0. && actual.isolated_advance_em > 0.);
        }
    }
    #[test]
    fn replaced_documents_reject_in_flight_paint_and_resume() {
        let rows = rows();
        let mut worker = WordPaint::default();
        assert!(worker.submit(&rows, 0, 0, 48., 700));
        worker.reset();
        assert_eq!(finish(&mut worker, &rows), None);
        assert_eq!(
            rows.row_data(0)
                .unwrap()
                .parts
                .row_data(0)
                .unwrap()
                .glow_wide
                .size()
                .width,
            0
        );
        assert!(worker.submit(&rows, 0, 0, 48., 700));
        assert_eq!(finish(&mut worker, &rows), Some((0, 0)));
    }
    #[test]
    fn seek_and_reduced_motion_discard_late_results() {
        let rows = rows();
        let mut worker = WordPaint::default();
        for (ms, enabled) in [(10000, true), (1500, false)] {
            assert!(worker.submit(&rows, 0, 0, 48., 700));
            assert_eq!(finish_at(&mut worker, &rows, ms, enabled), None);
            assert_eq!(
                rows.row_data(0)
                    .unwrap()
                    .parts
                    .row_data(0)
                    .unwrap()
                    .glyphs
                    .row_count(),
                0
            );
        }
    }
}
