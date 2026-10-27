//! Braccato's bundled typography with a consistent native shaping/paint fallback.
use std::sync::OnceLock;
use ttf_parser::Face;

fn faces() -> &'static [Face<'static>; 3] {
    static FACES: OnceLock<[Face<'static>; 3]> = OnceLock::new();
    FACES.get_or_init(|| {
        let mut faces = [
            Face::parse(include_bytes!("../assets/fonts/Satoshi-700.otf"), 0).unwrap(),
            Face::parse(include_bytes!("../assets/fonts/NotoSans.ttf"), 0).unwrap(),
            Face::parse(include_bytes!("../assets/fonts/NotoSansJP.ttf"), 0).unwrap(),
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
        "Satoshi" if weight < 600 => include_bytes!("../assets/fonts/Satoshi-500.otf"),
        "Satoshi" => include_bytes!("../assets/fonts/Satoshi-700.otf"),
        "Roboto" => include_bytes!("../assets/fonts/Roboto.ttf"),
        "Noto Sans" => include_bytes!("../assets/fonts/NotoSans.ttf"),
        "Noto Sans JP" => include_bytes!("../assets/fonts/NotoSansJP.ttf"),
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
                    include_bytes!("../assets/fonts/Satoshi-700.otf").to_vec(),
                ),
                (
                    "Satoshi 500",
                    include_bytes!("../assets/fonts/Satoshi-500.otf").to_vec(),
                ),
                (
                    "Roboto",
                    include_bytes!("../assets/fonts/Roboto.ttf").to_vec(),
                ),
                (
                    "Noto Sans",
                    include_bytes!("../assets/fonts/NotoSans.ttf").to_vec(),
                ),
                (
                    "Noto Sans JP",
                    include_bytes!("../assets/fonts/NotoSansJP.ttf").to_vec(),
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
}
thread_local! {static METRICS:std::cell::RefCell<Metrics>=Default::default();}
fn with_metrics<T>(f: impl FnOnce(&mut Metrics) -> T) -> T {
    METRICS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let next = revision();
        if cache.revision != next || cache.families.len() > 4096 || cache.widths.len() > 4096 {
            *cache = Metrics {
                revision: next,
                ..Default::default()
            };
        }
        f(&mut cache)
    })
}
pub fn family_for(text: &str, preferred: &str) -> String {
    let key = (text.to_owned(), preferred.to_owned());
    if let Some(family) = with_metrics(|cache| cache.families.get(&key).cloned()) {
        return family;
    }
    let family = family_uncached(text, preferred);
    with_metrics(|cache| cache.families.insert(key, family.clone()));
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
    with_metrics(|cache| cache.widths.insert(key, width));
    width
}
fn measure_uncached(text: &str, size: f32, preferred: &str, weight: u16) -> f32 {
    let family = family_for(text, preferred);
    let Some(bytes) = data(&family, weight) else {
        return measure(text, size);
    };
    let Ok(mut face) = Face::parse(&bytes, 0) else {
        return measure(text, size);
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
            include_bytes!("../assets/fonts/Satoshi-700.otf").to_vec(),
            include_bytes!("../assets/fonts/Satoshi-500.otf").to_vec(),
            include_bytes!("../assets/fonts/NotoSans.ttf").to_vec(),
            include_bytes!("../assets/fonts/NotoSansJP.ttf").to_vec(),
            include_bytes!("../assets/fonts/Roboto.ttf").to_vec(),
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
    fn native_paint_and_text_use_fonts_covering_the_actual_script() {
        assert_eq!(family("There's a snake"), "Satoshi");
        assert_eq!(family("Ще й забулися слова"), "Noto Sans");
        assert_eq!(family("你好"), "Noto Sans JP");
        assert!(measure("word\u{a0}", 48.) > measure("word", 48.));
    }
}
