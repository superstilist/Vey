//! Braccato's bundled typography with a consistent native shaping/paint fallback.
use std::sync::OnceLock;
use ttf_parser::Face;

fn faces() -> &'static [Face<'static>; 3] {
    static FACES: OnceLock<[Face<'static>; 3]> = OnceLock::new();
    FACES.get_or_init(|| {
        let mut faces = [
            Face::parse(vey_ui::FONT_SATOSHI_700, 0).unwrap(),
            Face::parse(vey_ui::FONT_NOTO_SANS, 0).unwrap(),
            Face::parse(vey_ui::FONT_NOTO_SANS_JP, 0).unwrap(),
        ];
        for face in &mut faces {
            face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), 700.);
        }
        faces
    })
}
pub fn family(text: &str) -> &'static str {
    let index = faces()
        .iter()
        .position(|face| {
            text.chars()
                .all(|c| c.is_whitespace() || face.glyph_index(c).is_some())
        })
        .unwrap_or(2);
    ["Satoshi", "Noto Sans", "Noto Sans JP"][index]
}
pub fn measure(text: &str, size: f32) -> f32 {
    measure_with(text, size, family(text), 700)
}
fn nominal_measure(text: &str, size: f32) -> f32 {
    let family = family(text);
    let face = &faces()[["Satoshi", "Noto Sans", "Noto Sans JP"]
        .iter()
        .position(|f| *f == family)
        .unwrap()];
    text.chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .map_or(size, |a| a as f32 * size / face.units_per_em() as f32)
        })
        .sum()
}
#[derive(Clone)]
struct CustomFont {
    family: String,
    bytes: std::sync::Arc<Vec<u8>>,
}
fn custom() -> &'static std::sync::Mutex<Vec<CustomFont>> {
    static FONTS: OnceLock<std::sync::Mutex<Vec<CustomFont>>> = OnceLock::new();
    FONTS.get_or_init(Default::default)
}
static REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn revision() -> u64 {
    REVISION.load(std::sync::atomic::Ordering::Acquire)
}
pub fn register(bytes: Vec<u8>) -> Result<String, String> {
    if bytes.len() > 20 * 1024 * 1024 {
        return Err("Font exceeds 20 MiB".into());
    }
    let family = {
        let face = Face::parse(&bytes, 0).map_err(|_| "Invalid TTF/OTF font")?;
        [16, 1]
            .into_iter()
            .find_map(|id| {
                face.names()
                    .into_iter()
                    .filter(|n| n.name_id == id)
                    .find_map(|n| n.to_string())
            })
            .filter(|s| !s.trim().is_empty())
            .ok_or("Font family is missing")?
    };
    let mut fonts = custom().lock().unwrap();
    if fonts.iter().any(|f| f.bytes.as_slice() == bytes.as_slice()) {
        return Ok(family);
    }
    if fonts.len() >= 8 {
        return Err("Up to eight imported fonts can be loaded".into());
    }
    let bytes = std::sync::Arc::new(bytes);
    let blob = slint::fontique_011::fontique::Blob::new(bytes.clone());
    if slint::fontique_011::shared_collection()
        .register_fonts(blob, None)
        .is_empty()
    {
        return Err("Cannot register font".into());
    }
    fonts.push(CustomFont {
        family: family.clone(),
        bytes,
    });
    REVISION.fetch_add(1, std::sync::atomic::Ordering::Release);
    Ok(family)
}
pub fn import(path: &std::path::Path) -> Result<String, String> {
    use sha1::{Digest, Sha1};
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 20 * 1024 * 1024 {
        return Err("Font exceeds 20 MiB".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let hash = format!("{:x}", Sha1::digest(&bytes));
    let family = register(bytes.clone())?;
    let dir = crate::storage::data_dir().join("lyric-fonts");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{hash}.font")), bytes).map_err(|e| e.to_string())?;
    Ok(family)
}
pub fn restore() {
    if let Ok(files) = std::fs::read_dir(crate::storage::data_dir().join("lyric-fonts")) {
        for file in files.flatten().take(8) {
            if file.path().extension().is_some_and(|e| e == "font") {
                if let Ok(meta) = file.metadata() {
                    if meta.len() > 20 * 1024 * 1024 {
                        continue;
                    }
                }
                if let Ok(bytes) = std::fs::read(file.path()) {
                    let _ = register(bytes);
                }
            }
        }
    }
}
fn data(family: &str, weight: u16) -> Option<std::sync::Arc<Vec<u8>>> {
    if let Some(font) = custom()
        .lock()
        .unwrap()
        .iter()
        .filter(|f| f.family.eq_ignore_ascii_case(family))
        .min_by_key(|f| {
            Face::parse(&f.bytes, 0)
                .map_or(u16::MAX, |face| face.weight().to_number().abs_diff(weight))
        })
    {
        return Some(font.bytes.clone());
    }
    let bytes: &[u8] = match family {
        "Satoshi" if weight < 600 => vey_ui::FONT_SATOSHI_500,
        "Satoshi" => vey_ui::FONT_SATOSHI_700,
        "Roboto" => vey_ui::FONT_ROBOTO,
        "Noto Sans" => vey_ui::FONT_NOTO_SANS,
        "Noto Sans JP" => vey_ui::FONT_NOTO_SANS_JP,
        _ => return None,
    };
    // Bundled data is retained once, never copied for each measured text run.
    static BUNDLED: OnceLock<std::collections::HashMap<String, std::sync::Arc<Vec<u8>>>> =
        OnceLock::new();
    let key = if family == "Satoshi" && weight < 600 {
        "Satoshi 500"
    } else {
        family
    };
    BUNDLED
        .get_or_init(|| {
            [
                (
                    "Satoshi",
                    vey_ui::FONT_SATOSHI_700.to_vec(),
                ),
                (
                    "Satoshi 500",
                    vey_ui::FONT_SATOSHI_500.to_vec(),
                ),
                (
                    "Roboto",
                    vey_ui::FONT_ROBOTO.to_vec(),
                ),
                (
                    "Noto Sans",
                    vey_ui::FONT_NOTO_SANS.to_vec(),
                ),
                (
                    "Noto Sans JP",
                    vey_ui::FONT_NOTO_SANS_JP.to_vec(),
                ),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), std::sync::Arc::new(v)))
            .collect()
        })
        .get(key)
        .cloned()
        .or_else(|| Some(std::sync::Arc::new(bytes.to_vec())))
}
#[derive(Default)]
struct Metrics {
    revision: u64,
    families: std::collections::HashMap<(String, String), String>,
    widths: std::collections::HashMap<(String, String, u32, u16), f32>,
    shapers: std::collections::HashMap<(String, u16), ShapeFont>,
    runs: std::collections::HashMap<(String, String, u16), Option<ShapedRun>>,
    letters: std::collections::HashMap<(String, String, u16), Option<Vec<LetterGeometry>>>,
}
thread_local! {static METRICS:std::cell::RefCell<Metrics>=Default::default();}
fn with_metrics<T>(f: impl FnOnce(&mut Metrics) -> T) -> T {
    METRICS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let next = revision();
        if cache.revision != next
            || cache.families.len() > 4096
            || cache.widths.len() > 4096
            || cache.runs.len() > 4096
            || cache.letters.len() > 4096
            || cache.shapers.len() > 64
        {
            *cache = Metrics {
                revision: next,
                ..Default::default()
            };
        }
        f(&mut cache)
    })
}
fn publish_metrics(expected_revision: u64, f: impl FnOnce(&mut Metrics)) {
    with_metrics(|cache| {
        // Font registration can happen while a result is being computed. Do not
        // publish that result into a cache refreshed for a different font set.
        if cache.revision == expected_revision {
            f(cache);
        }
    });
}
pub fn family_for(text: &str, preferred: &str) -> String {
    let expected_revision = revision();
    let key = (text.to_owned(), preferred.to_owned());
    if let Some(family) = with_metrics(|cache| cache.families.get(&key).cloned()) {
        return family;
    }
    let family = family_uncached(text, preferred);
    publish_metrics(expected_revision, |cache| {
        cache.families.insert(key, family.clone());
    });
    family
}
fn family_uncached(text: &str, preferred: &str) -> String {
    if data(preferred, 700).is_some_and(|data| {
        Face::parse(&data, 0).is_ok_and(|f| {
            text.chars()
                .all(|c| c.is_whitespace() || f.glyph_index(c).is_some())
        })
    }) {
        preferred.into()
    } else {
        family(text).into()
    }
}
pub fn measure_with(text: &str, size: f32, preferred: &str, weight: u16) -> f32 {
    let expected_revision = revision();
    let key = (
        text.to_owned(),
        preferred.to_owned(),
        size.to_bits(),
        weight,
    );
    if let Some(width) = with_metrics(|cache| cache.widths.get(&key).copied()) {
        return width;
    }
    let width = measure_uncached(text, size, preferred, weight);
    publish_metrics(expected_revision, |cache| {
        cache.widths.insert(key, width);
    });
    width
}
fn measure_uncached(text: &str, size: f32, preferred: &str, weight: u16) -> f32 {
    let family = family_for(text, preferred);
    if let Some(run) = shaped_run(text, &family, weight) {
        return run.advance as f32 * size / run.units_per_em;
    }
    let Some(bytes) = data(&family, weight) else {
        return nominal_measure(text, size);
    };
    let Ok(mut face) = Face::parse(&bytes, 0) else {
        return nominal_measure(text, size);
    };
    face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), weight as f32);
    text.chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .map_or(size, |a| a as f32 * size / face.units_per_em() as f32)
        })
        .sum()
}

/// A shaped text cluster that can be painted independently without changing its ink.
/// Ligatures and combining marks remain in one cluster. `start`/`width` are fractions
/// of the whole run; the `_em` fields retain unrounded font-space geometry. Byte
/// ranges refer to the original UTF-8 text.
#[derive(Clone, Debug, PartialEq)]
pub struct LetterGeometry {
    pub text: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub start: f32,
    pub width: f32,
    pub start_em: f32,
    pub width_em: f32,
    pub isolated_advance_em: f32,
}

struct ShapeFont {
    bytes: std::sync::Arc<Vec<u8>>,
    data: harfrust::ShaperData,
    instance: harfrust::ShaperInstance,
}

impl ShapeFont {
    fn new(family: &str, weight: u16) -> Option<Self> {
        let bytes = data(family, weight)?;
        let font = harfrust::FontRef::from_index(&bytes, 0).ok()?;
        let data = harfrust::ShaperData::new(&font);
        let instance = harfrust::ShaperInstance::from_variations(
            &font,
            [harfrust::Variation {
                tag: harfrust::Tag::new(b"wght"),
                value: weight as f32,
            }],
        );
        Some(Self {
            bytes,
            data,
            instance,
        })
    }

    fn shape(&self, text: &str) -> Option<ShapedRun> {
        let font = harfrust::FontRef::from_index(&self.bytes, 0).ok()?;
        let shaper = self
            .data
            .shaper(&font)
            .instance(Some(&self.instance))
            .build();
        let mut buffer = harfrust::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let direction = buffer.direction();
        let output = shaper.shape(buffer, harfrust::ShapeOptions::default());
        let glyphs: Vec<_> = output
            .glyph_infos()
            .iter()
            .zip(output.glyph_positions())
            .map(|(info, position)| ShapeGlyph {
                id: info.glyph_id,
                cluster: info.cluster as usize,
                x_advance: position.x_advance as i64,
                y_advance: position.y_advance as i64,
                x_offset: position.x_offset as i64,
                y_offset: position.y_offset as i64,
            })
            .collect();
        // A font selected for the run may still lack an optional or fallback glyph.
        // Let the normal text renderer handle that run instead of animating .notdef.
        if glyphs.iter().any(|glyph| glyph.id == 0) {
            return None;
        }
        Some(ShapedRun {
            advance: glyphs.iter().map(|glyph| glyph.x_advance).sum(),
            units_per_em: shaper.units_per_em() as f32,
            direction,
            glyphs,
        })
    }
}

#[derive(Clone)]
struct ShapeGlyph {
    id: u32,
    cluster: usize,
    x_advance: i64,
    y_advance: i64,
    x_offset: i64,
    y_offset: i64,
}

#[derive(Clone)]
struct ShapedRun {
    advance: i64,
    units_per_em: f32,
    direction: harfrust::Direction,
    glyphs: Vec<ShapeGlyph>,
}

fn shaped_run(text: &str, family: &str, weight: u16) -> Option<ShapedRun> {
    let expected_revision = revision();
    let key = (text.to_owned(), family.to_owned(), weight);
    if let Some(run) = with_metrics(|cache| cache.runs.get(&key).cloned()) {
        return run;
    }
    let run = with_metrics(|cache| {
        let key = (family.to_owned(), weight);
        if !cache.shapers.contains_key(&key) {
            cache
                .shapers
                .insert(key.clone(), ShapeFont::new(family, weight)?);
        }
        cache.shapers.get(&key)?.shape(text)
    });
    publish_metrics(expected_revision, |cache| {
        cache.runs.insert(key, run.clone());
    });
    run
}

/// Advance before pixel rounding, including kerning, ligatures and variation.
pub fn shaped_advance_em(text: &str, preferred: &str, weight: u16) -> Option<f32> {
    let family = family_for(text, preferred);
    let run = shaped_run(text, &family, weight)?;
    Some(run.advance as f32 / run.units_per_em)
}

/// Returns geometry for the animated-letter path, or `None` to retain whole-run
/// text shaping. Checking the isolated glyph IDs and offsets prevents contextual
/// substitutions or mark positioning from changing when the animation starts.
pub fn letter_geometry(text: &str, preferred: &str, weight: u16) -> Option<Vec<LetterGeometry>> {
    let expected_revision = revision();
    let family = family_for(text, preferred);
    let key = (text.to_owned(), family.clone(), weight);
    if let Some(letters) = with_metrics(|cache| cache.letters.get(&key).cloned()) {
        return letters;
    }
    let letters = letter_geometry_uncached(text, &family, weight);
    publish_metrics(expected_revision, |cache| {
        cache.letters.insert(key, letters.clone());
    });
    letters
}

fn letter_geometry_uncached(text: &str, family: &str, weight: u16) -> Option<Vec<LetterGeometry>> {
    use unicode_script::{Script, UnicodeScript};
    // These scripts do not require joining or syllabic reordering. Others keep
    // the full shaped run, even if a particular font could split one safely.
    if text.chars().any(|c| {
        c.is_control()
            || matches!(c, '\u{200c}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            || !matches!(
                c.script(),
                Script::Latin
                    | Script::Cyrillic
                    | Script::Greek
                    | Script::Armenian
                    | Script::Georgian
                    | Script::Han
                    | Script::Hiragana
                    | Script::Katakana
                    | Script::Hangul
                    | Script::Common
                    | Script::Inherited
            )
    }) {
        return None;
    }
    let run = shaped_run(text, family, weight)?;
    if run.advance <= 0
        || run.direction != harfrust::Direction::LeftToRight
        || run.glyphs.first()?.cluster != 0
        || run
            .glyphs
            .windows(2)
            .any(|pair| pair[0].cluster > pair[1].cluster)
        || run.glyphs.iter().any(|glyph| glyph.y_advance != 0)
    {
        return None;
    }
    let mut letters = Vec::new();
    let mut glyph_start = 0;
    let mut pen = 0_i64;
    while glyph_start < run.glyphs.len() {
        let byte_start = run.glyphs[glyph_start].cluster;
        let glyph_end = run.glyphs[glyph_start..]
            .iter()
            .position(|glyph| glyph.cluster != byte_start)
            .map_or(run.glyphs.len(), |offset| glyph_start + offset);
        let byte_end = run
            .glyphs
            .get(glyph_end)
            .map_or(text.len(), |glyph| glyph.cluster);
        let cluster = text.get(byte_start..byte_end)?;
        let isolated = shaped_run(cluster, family, weight)?;
        let full = &run.glyphs[glyph_start..glyph_end];
        if !same_cluster_ink(full, &isolated.glyphs) {
            return None;
        }
        let advance: i64 = full.iter().map(|glyph| glyph.x_advance).sum();
        if advance <= 0 {
            return None;
        }
        letters.push(LetterGeometry {
            text: cluster.to_owned(),
            byte_start,
            byte_end,
            start: pen as f32 / run.advance as f32,
            width: advance as f32 / run.advance as f32,
            start_em: pen as f32 / run.units_per_em,
            width_em: advance as f32 / run.units_per_em,
            isolated_advance_em: isolated.advance as f32 / isolated.units_per_em,
        });
        pen += advance;
        glyph_start = glyph_end;
    }
    Some(letters)
}

fn same_cluster_ink(full: &[ShapeGlyph], isolated: &[ShapeGlyph]) -> bool {
    if full.len() != isolated.len() {
        return false;
    }
    let mut full_pen = 0_i64;
    let mut isolated_pen = 0_i64;
    for (full, isolated) in full.iter().zip(isolated) {
        if full.id != isolated.id
            || full_pen + full.x_offset != isolated_pen + isolated.x_offset
            || full.y_offset != isolated.y_offset
            || full.y_advance != 0
            || isolated.y_advance != 0
        {
            return false;
        }
        full_pen += full.x_advance;
        isolated_pen += isolated.x_advance;
    }
    // The final advance can contain kerning against the next cluster. The glyph
    // ink above must match; the cluster's full-run advance positions that neighbor.
    true
}
pub fn refresh_cache(cache: &mut braccato_slint::GlowCache, previous: &mut u64) -> bool {
    let next = revision();
    if next == *previous {
        return false;
    }
    for f in custom().lock().unwrap().iter() {
        cache.register_font_shared_data(f.bytes.clone());
    }
    *previous = next;
    true
}
pub fn cache() -> braccato_slint::GlowCache {
    let mut cache = braccato_slint::GlowCache::new(
        [
            vey_ui::FONT_SATOSHI_700.to_vec(),
            vey_ui::FONT_SATOSHI_500.to_vec(),
            vey_ui::FONT_NOTO_SANS.to_vec(),
            vey_ui::FONT_NOTO_SANS_JP.to_vec(),
            vey_ui::FONT_ROBOTO.to_vec(),
        ],
        384,
    );
    for f in custom().lock().unwrap().iter() {
        cache.register_font_shared_data(f.bytes.clone());
    }
    cache
}
pub fn padding(size: f32) -> f32 {
    if size >= 40. {
        32.
    } else {
        20.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_metrics_preserve_font_weight_script_and_revision() {
        for text in ["word\u{a0}", "Ще й забулися", "幅の異なる文字"] {
            for weight in [500, 700] {
                let expected = measure_uncached(text, 32., "Satoshi", weight);
                assert_eq!(measure_with(text, 32., "Satoshi", weight), expected);
                assert_eq!(measure_with(text, 32., "Satoshi", weight), expected);
            }
        }
        with_metrics(|cache| cache.revision = u64::MAX);
        assert_eq!(
            measure_with("revision", 48., "Roboto", 700),
            measure_uncached("revision", 48., "Roboto", 700)
        );
        assert_eq!(with_metrics(|cache| cache.revision), revision());
    }
    #[test]
    fn font_results_from_an_older_revision_cannot_enter_a_refreshed_cache() {
        let family_key = ("stale font generation".to_owned(), "Satoshi".to_owned());
        let width_key = (
            family_key.0.clone(),
            family_key.1.clone(),
            48_f32.to_bits(),
            700,
        );
        let run_key = (family_key.0.clone(), family_key.1.clone(), 700);
        let stale_revision = revision().wrapping_sub(1);
        publish_metrics(stale_revision, |cache| {
            cache.families.insert(family_key.clone(), "stale".into());
            cache.widths.insert(width_key.clone(), f32::INFINITY);
            cache.runs.insert(run_key.clone(), None);
            cache.letters.insert(run_key.clone(), None);
        });
        with_metrics(|cache| {
            assert!(!cache.families.contains_key(&family_key));
            assert!(!cache.widths.contains_key(&width_key));
            assert!(!cache.runs.contains_key(&run_key));
            assert!(!cache.letters.contains_key(&run_key));
        });
        let current_revision = revision();
        publish_metrics(current_revision, |cache| {
            cache.widths.insert(width_key.clone(), 42.);
        });
        assert_eq!(
            with_metrics(|cache| cache.widths.get(&width_key).copied()),
            Some(42.)
        );
    }
    #[test]
    fn native_paint_and_text_use_fonts_covering_the_actual_script() {
        assert_eq!(family("There's a snake"), "Satoshi");
        assert_eq!(family("Ще й забулися слова"), "Noto Sans");
        assert_eq!(family("你好"), "Noto Sans JP");
        assert!(measure("word\u{a0}", 48.) > measure("word", 48.));
    }
    #[test]
    fn shaped_metrics_and_cluster_positions_preserve_real_pair_kerning() {
        let size = 64.;
        for text in ["AV", "To", "WA", "Ta"] {
            let letters = letter_geometry(text, "Satoshi", 700).unwrap();
            assert_eq!(letters.len(), 2);
            let full = measure_with(text, size, "Satoshi", 700);
            let separate: f32 = text
                .chars()
                .map(|c| measure_with(&c.to_string(), size, "Satoshi", 700))
                .sum();
            assert!(
                full < separate - 0.1,
                "{text}: shaped {full}, separate {separate}"
            );
            assert_eq!(letters[1].start_em, letters[0].width_em);
            assert!((letters.iter().map(|g| g.width_em).sum::<f32>() * size - full).abs() < 0.001);
            assert!((shaped_advance_em(text, "Satoshi", 700).unwrap() * size - full).abs() < 0.001);
        }
    }
    #[test]
    fn ligatures_and_combining_marks_are_kept_as_shaped_clusters() {
        let text = "office";
        let letters = letter_geometry(text, "Roboto", 700).unwrap();
        assert!(letters.iter().any(|letter| letter.text == "ffi"));
        assert_eq!(
            letters
                .iter()
                .map(|letter| letter.text.as_str())
                .collect::<String>(),
            text
        );
        for letter in &letters {
            assert_eq!(&text[letter.byte_start..letter.byte_end], letter.text);
        }
        let combining = "e\u{301}";
        let letters = letter_geometry(combining, "Noto Sans", 700).unwrap();
        assert_eq!(letters.len(), 1);
        assert_eq!(letters[0].text, combining);
        assert!(
            (measure_with(combining, 48., "Noto Sans", 700)
                - measure_with("é", 48., "Noto Sans", 700))
            .abs()
                < 0.001
        );
        // There is no precomposed x-acute: this exercises an actual positioned
        // mark rather than only Unicode composition to a single glyph.
        let marked = "x\u{301}";
        let letters = letter_geometry(marked, "Noto Sans", 700).unwrap();
        let run = shaped_run(marked, "Noto Sans", 700).unwrap();
        assert_eq!(letters.len(), 1);
        assert_eq!(letters[0].text, marked);
        assert!(run.glyphs.len() >= 2);
        assert!(run.glyphs.iter().any(|glyph| glyph.x_advance == 0));
        assert!(
            measure_with("word\u{a0}", 48., "Noto Sans", 700)
                > measure_with("word", 48., "Noto Sans", 700)
        );
    }
    #[test]
    fn contextual_and_reordered_scripts_keep_whole_run_shaping() {
        for text in ["سلام", "שלום", "नमस्ते", "a\u{200d}b", "a\u{202e}b"] {
            assert!(letter_geometry(text, "Noto Sans", 700).is_none(), "{text}");
            let width = measure_with(text, 48., "Noto Sans", 700);
            assert!(width.is_finite() && width > 0.);
        }
        let text = "Ще";
        let letters = letter_geometry(text, "Satoshi", 700).unwrap();
        assert_eq!(
            letters
                .iter()
                .map(|letter| letter.text.as_str())
                .collect::<String>(),
            text
        );
    }
}
