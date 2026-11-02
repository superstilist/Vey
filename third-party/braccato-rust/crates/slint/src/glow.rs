//! Cached glyph shadows for native renderers. Rasterization happens only on a cache miss.
use crate::{LyricGlyph, SwipeRamp, plan_letter_mask_sweep};
use resvg::{tiny_skia, usvg};
use slint::{Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct GlowKey(String, String, u16, u16, u16);
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct FontKey(String, String, u16);

struct WordMetrics {
    advance: f32,
    ascent: f32,
    height: f32,
    letters: Rc<[(SharedString, f32, f32)]>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GlowCacheStats {
    pub metric_builds: usize,
    pub shadow_builds: usize,
    pub metric_entries: usize,
    pub shadow_entries: usize,
    pub resident_bytes: usize,
}

#[derive(Clone, Default)]
pub struct WordGlow {
    pub wide: Image,
    pub tight: Image,
    /// Transparent inset in the masks' fixed 64px font coordinate system.
    pub padding: f32,
    letters: Rc<[(SharedString, f32, f32)]>,
}

impl WordGlow {
    fn bytes(&self) -> usize {
        let images = if self.wide == self.tight {
            &[&self.wide][..]
        } else {
            &[&self.wide, &self.tight][..]
        };
        images
            .iter()
            .map(|image| {
                let size = image.size();
                size.width as usize * size.height as usize * 4
            })
            .sum()
    }
    /// Native equivalent of upstream planLetterWave's default linear swipe.
    pub fn letters(&self, duration_ms: f32) -> ModelRc<LyricGlyph> {
        self.letters_with_settings(duration_ms, 900.0, 0.1, 1.6, -0.2, -0.1, 1.4, 1.5)
    }

    /// Use the theme's configured crest duration while retaining the upstream lead and sweep ratios.
    pub fn letters_with_duration(
        &self,
        duration_ms: f32,
        wave_duration_ms: f32,
    ) -> ModelRc<LyricGlyph> {
        self.letters_with_settings(
            duration_ms,
            wave_duration_ms,
            0.1,
            1.6,
            -0.2,
            -0.1,
            1.4,
            1.5,
        )
    }

    /// Plan each glyph from the same forward linear swipe windows used by the upstream engine.
    #[allow(clippy::too_many_arguments)]
    pub fn letters_with_settings(
        &self,
        duration_ms: f32,
        wave_duration_ms: f32,
        swipe_lead_ratio: f32,
        swipe_duration_ratio: f32,
        start_from: f32,
        end_from: f32,
        start_to: f32,
        end_to: f32,
    ) -> ModelRc<LyricGlyph> {
        if duration_ms <= 0.0 {
            return ModelRc::default();
        }
        let wave_duration_ms = wave_duration_ms.max(1.0);
        let swipe_duration_ms = (duration_ms * swipe_duration_ratio).max(1.0);
        let swipe_lead_ms = duration_ms * swipe_lead_ratio;
        let sweeps = plan_letter_mask_sweep(
            SwipeRamp {
                start_from,
                end_from,
                start_to,
                end_to,
            },
            self.letters.len(),
            swipe_duration_ms,
        );
        ModelRc::new(VecModel::from(
            self.letters
                .iter()
                .zip(sweeps)
                .map(|((text, start, width), sweep)| {
                    let touched = sweep.delay_ms - swipe_lead_ms;
                    let lit = touched + sweep.duration_ms;
                    let crest_ms = wave_duration_ms * 0.4;
                    let (delay_ms, wave_ms) = if lit - crest_ms > touched {
                        (touched, (lit - touched) / 0.4)
                    } else {
                        (lit - crest_ms, wave_duration_ms)
                    };
                    LyricGlyph {
                        text: text.clone(),
                        start: *start,
                        width: *width,
                        start_em: 0.,
                        width_em: 0.,
                        isolated_advance_em: 0.,
                        delay_ms,
                        wave_ms,
                        sweep_delay_ms: sweep.delay_ms,
                        sweep_duration_ms: sweep.duration_ms,
                        sweep_from: sweep.from,
                        sweep_to: sweep.to,
                        sweep_feather: sweep.feather,
                    }
                })
                .collect::<Vec<_>>(),
        ))
    }
}
/// Supply the same font bytes registered by the host's Slint UI.
/// The bounded cache stores neutral alpha masks; color and timing stay on the GPU/UI side.
pub struct GlowCache {
    options: usvg::Options<'static>,
    entries: HashMap<GlowKey, WordGlow>,
    order: VecDeque<GlowKey>,
    capacity: usize,
    resident_bytes: usize,
    metrics: HashMap<FontKey, Rc<WordMetrics>>,
    metric_order: VecDeque<FontKey>,
    metric_glyphs: usize,
    metric_builds: usize,
    shadow_builds: usize,
}

impl GlowCache {
    /// Register a host-owned font in the glyph/shadow rasterizer. Register the same
    /// bytes with the host's Slint font collection for matching text and glow.
    /// Existing image handles remain valid; future lookups use the new font data.
    pub fn register_font_shared_data(
        &mut self,
        data: std::sync::Arc<dyn AsRef<[u8]> + Send + Sync>,
    ) -> bool {
        let before = self.options.fontdb.faces().count();
        self.options
            .fontdb_mut()
            .load_font_source(usvg::fontdb::Source::Binary(data));
        if self.options.fontdb.faces().count() == before {
            return false;
        }
        self.entries.clear();
        self.order.clear();
        self.resident_bytes = 0;
        self.metrics.clear();
        self.metric_order.clear();
        self.metric_glyphs = 0;
        true
    }

    pub fn new(fonts: impl IntoIterator<Item = Vec<u8>>, capacity: usize) -> Self {
        let mut options = usvg::Options::default();
        for font in fonts {
            options.fontdb_mut().load_font_data(font);
        }
        Self {
            options,
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity: capacity.max(1),
            resident_bytes: 0,
            metrics: HashMap::new(),
            metric_order: VecDeque::new(),
            metric_glyphs: 0,
            metric_builds: 0,
            shadow_builds: 0,
        }
    }

    pub fn stats(&self) -> GlowCacheStats {
        GlowCacheStats {
            metric_builds: self.metric_builds,
            shadow_builds: self.shadow_builds,
            metric_entries: self.metrics.len(),
            shadow_entries: self.entries.len(),
            resident_bytes: self.resident_bytes,
        }
    }

    fn metrics(&mut self, text: &str, family: &str, weight: u16) -> Option<Rc<WordMetrics>> {
        let key = FontKey(text.to_owned(), family.to_owned(), weight);
        if let Some(metrics) = self.metrics.get(&key) {
            return Some(metrics.clone());
        }
        let metrics = Rc::new(self.read_metrics(text, family, weight)?);
        self.metric_builds += 1;
        // Geometry is cheap but must not compete with raster textures: a long song's
        // future words otherwise evict the active shadows on every cue update.
        while self.metrics.len() >= 2048 || self.metric_glyphs + metrics.letters.len() > 65536 {
            let Some(old) = self.metric_order.pop_front() else {
                break;
            };
            if let Some(evicted) = self.metrics.remove(&old) {
                self.metric_glyphs -= evicted.letters.len();
            }
        }
        self.metric_glyphs += metrics.letters.len();
        self.metric_order.push_back(key.clone());
        self.metrics.insert(key, metrics.clone());
        Some(metrics)
    }

    pub fn word(&mut self, text: &str, family: &str, weight: u16) -> WordGlow {
        self.word_with_radii(text, family, weight, 17.1, 3.2, 64.0)
    }

    /// Rasterize upstream drop-shadow radii at the requested rendered font size.
    pub fn word_with_radii(
        &mut self,
        text: &str,
        family: &str,
        weight: u16,
        wide_radius_px: f32,
        tight_radius_px: f32,
        font_size: f32,
    ) -> WordGlow {
        if text.trim().is_empty() || text.chars().count() > 256 {
            return WordGlow::default();
        }
        let scale = 64.0 / font_size.max(1.0);
        let wide_sigma = (wide_radius_px.max(0.0) * scale * 100.0)
            .round()
            .clamp(0.0, 4096.0) as u16;
        let tight_sigma = (tight_radius_px.max(0.0) * scale * 100.0)
            .round()
            .clamp(0.0, 4096.0) as u16;
        if wide_sigma == 0 && tight_sigma == 0 {
            return self
                .metrics(text, family, weight)
                .map_or_else(WordGlow::default, |metrics| WordGlow {
                    padding: 32.0,
                    letters: metrics.letters.clone(),
                    ..Default::default()
                });
        }
        let key = GlowKey(
            text.to_owned(),
            family.to_owned(),
            weight,
            wide_sigma,
            tight_sigma,
        );
        if let Some(image) = self.entries.get(&key) {
            return image.clone();
        }
        let Some(metrics) = self.metrics(text, family, weight) else {
            return WordGlow::default();
        };
        self.shadow_builds += 1;
        let image = self.render(
            text,
            family,
            weight,
            wide_sigma as f32 / 100.0,
            tight_sigma as f32 / 100.0,
            &metrics,
        );
        let bytes = image.bytes();
        while self.entries.len() >= self.capacity || self.resident_bytes + bytes > 32 * 1024 * 1024
        {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&old) {
                self.resident_bytes -= evicted.bytes();
            }
        }
        self.resident_bytes += bytes;
        self.order.push_back(key.clone());
        self.entries.insert(key, image.clone());
        image
    }

    /// Make the soft line masks used by the upstream Sustain theme's distance blur.
    pub fn blurred_text(
        &mut self,
        text: &str,
        family: &str,
        weight: u16,
        radius_px: f32,
        font_size: f32,
    ) -> Image {
        self.word_with_radii(text, family, weight, radius_px, radius_px, font_size)
            .wide
    }

    /// Keep the inset together with a blurred run so hosts can align it without stretching.
    pub fn blurred_run(
        &mut self,
        text: &str,
        family: &str,
        weight: u16,
        radius_px: f32,
        font_size: f32,
    ) -> WordGlow {
        self.word_with_radii(text, family, weight, radius_px, radius_px, font_size)
    }

    fn read_metrics(&self, text: &str, family: &str, weight: u16) -> Option<WordMetrics> {
        let query = usvg::fontdb::Query {
            families: &[usvg::fontdb::Family::Name(family)],
            weight: usvg::fontdb::Weight(weight),
            ..Default::default()
        };
        let id = self.options.fontdb.query(&query)?;
        let metrics = self
            .options
            .fontdb
            .with_face_data(id, |bytes, index| {
                let mut face = ttf_parser::Face::parse(bytes, index).ok()?;
                // Match the native text brush's variable-font weight when
                // measuring mask bounds and letter sweep geometry.
                face.set_variation(ttf_parser::Tag::from_bytes(b"wght"),weight as f32);
                let scale = 64.0 / face.units_per_em() as f32;
                let width: f32 = text
                    .chars()
                    .filter_map(|c| face.glyph_index(c))
                    .filter_map(|g| face.glyph_hor_advance(g))
                    .map(|w| w as f32 * scale)
                    .sum();
                let mut advance = 0.0;
                let mut letters = Vec::new();
                // Joining scripts stay in Slint's whole-run shaper; never split their contextual forms.
                if !text
                    .chars()
                    .any(|c| matches!(c as u32, 0x0590..=0x0fff | 0x200c..=0x200d))
                    && width > 0.0
                {
                    for grapheme in text.graphemes(true) {
                        let w: f32 = grapheme
                            .chars()
                            .filter_map(|character| face.glyph_index(character))
                            .filter_map(|glyph| face.glyph_hor_advance(glyph))
                            .map(|advance| advance as f32 * scale)
                            .sum();
                        if w > 0.0 {
                            letters.push((grapheme.to_owned(), advance / width, w / width));
                        } else if let Some((last, _, _)) = letters.last_mut() {
                            last.push_str(grapheme);
                        }
                        advance += w;
                    }
                }
                Some((
                    width,
                    face.ascender() as f32 * scale,
                    (face.ascender() as f32 - face.descender() as f32 + face.line_gap() as f32)
                        * scale,
                    letters,
                ))
            })
            .flatten();
        let (advance, ascent, height, letters) = metrics?;
        Some(WordMetrics {
            advance,
            ascent,
            height,
            letters: letters
                .into_iter()
                .map(|(text, start, width)| (text.into(), start, width))
                .collect::<Vec<_>>()
                .into(),
        })
    }

    fn render(
        &self,
        text: &str,
        family: &str,
        weight: u16,
        wide_sigma: f32,
        tight_sigma: f32,
        metrics: &WordMetrics,
    ) -> WordGlow {
        // A fixed inset cuts off broad shadows, especially when small backing vocals
        // need a larger sigma at our 64px raster size. Keep three sigma plus an AA guard.
        let padding = (wide_sigma.max(tight_sigma) * 3.0 + 3.0).ceil().max(32.0);
        let width = (metrics.advance + padding * 2.0).ceil().clamp(1.0, 8192.0) as u32;
        let height = (metrics.height + padding * 2.0).ceil().clamp(1.0, 512.0) as u32;
        let escaped = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&apos;")
        };
        let render = |sigma: f32| -> Option<Image> {
            // A CSS drop-shadow with zero radius rests under the identical foreground glyph.
            // A separately rasterized sharp copy cannot do that across native text backends:
            // tiny shaping/metrics differences expose a second, displaced outline.
            if sigma <= 0.0 {
                return None;
            }
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">
                <defs><filter id="glow" filterUnits="userSpaceOnUse" x="0" y="0" width="{width}" height="{height}">
                <feGaussianBlur stdDeviation="{sigma}"/></filter></defs>
                <text x="{padding}" y="{}" font-family="{}" font-size="64" font-weight="{weight}"
                fill="white" xml:space="preserve" filter="url(#glow)">{}</text></svg>"#,
                padding + metrics.ascent,
                escaped(family),
                escaped(text)
            );
            let tree = usvg::Tree::from_data(svg.as_bytes(), &self.options).ok()?;
            let mut pixels = tiny_skia::Pixmap::new(width, height)?;
            resvg::render(&tree, tiny_skia::Transform::default(), &mut pixels.as_mut());
            let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
            for (target, source) in buffer.make_mut_slice().iter_mut().zip(pixels.pixels()) {
                *target = Rgba8Pixel::new(255, 255, 255, source.alpha());
            }
            Some(Image::from_rgba8(buffer))
        };
        let wide = render(wide_sigma).unwrap_or_default();
        let tight = if wide_sigma == tight_sigma {
            wide.clone()
        } else {
            render(tight_sigma).unwrap_or_default()
        };
        WordGlow {
            wide,
            tight,
            padding,
            letters: metrics.letters.clone(),
        }
    }
}
