//! Decode artwork on workers, with bounded memory and UI-sized output.
use image::{ImageReader, RgbaImage};
use std::io::Cursor;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

pub const MAX_BYTES: usize = 15_000_000;
const MAX_EDGE: u32 = 768;

/// Prefer the catalog cover and retain a public thumbnail fallback for videos.
pub fn remote_urls(track: &crate::services::Track) -> Vec<String> {
    let mut urls = Vec::new();
    if !track.thumbnail_url.is_empty() {
        urls.push(track.thumbnail_url.clone());
    }
    let valid_video = |id: &&str| {
        id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    };
    let video = track.video_id.as_deref().filter(valid_video)
        .or_else(|| track.path.strip_prefix("yt://").filter(valid_video));
    if let Some(id) = video {
        for fallback in [
            format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
            format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"),
        ] {
            if !urls.contains(&fallback) {
                urls.push(fallback);
            }
        }
    }
    urls
}

#[cfg(not(target_arch = "wasm32"))]
pub fn cache_path(key: &str) -> std::path::PathBuf {
    use sha1::{Digest, Sha1};
    crate::storage::data_dir()
        .join("artwork")
        .join(format!("{:x}.png", Sha1::digest(key.as_bytes())))
}

/// Check user-edited and downloaded disk cache entries across candidate keys.
#[cfg(not(target_arch = "wasm32"))]
pub fn cached_for_keys(keys: &[String]) -> Option<RgbaImage> {
    for key in keys {
        let primary = cache_path(key);
        if let Some(img) = cached(&primary) {
            return Some(img);
        }
        let downloaded = primary.with_extension("cache.png");
        if let Some(img) = cached(&downloaded) {
            return Some(img);
        }
    }
    None
}

/// Persist a fetched/embedded cover into the disposable cache without
/// overwriting a user-edited cover.
#[cfg(not(target_arch = "wasm32"))]
pub fn store_cached_image(key: &str, image: &RgbaImage) {
    let path = cache_path(key);
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_ok() {
        let downloaded = path.with_extension("cache.png");
        let tmp = path.with_extension(format!("{}.pending.jpg", std::process::id()));
        let saved = image::DynamicImage::ImageRgba8(image.clone())
            .into_rgb8()
            .save_with_format(&tmp, image::ImageFormat::Jpeg)
            .or_else(|_| image.save_with_format(&tmp, image::ImageFormat::Png));
        if saved.is_ok() {
            let _ = std::fs::remove_file(&downloaded);
            let _ = std::fs::rename(&tmp, &downloaded);
            let _ = std::fs::remove_file(tmp);
        }
    }
}

pub fn decode(bytes: &[u8]) -> Result<RgbaImage, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Artwork exceeds size limit".into());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| e.to_string())?;
    Ok(if img.width() > MAX_EDGE || img.height() > MAX_EDGE {
        img.thumbnail(MAX_EDGE, MAX_EDGE)
    } else {
        img
    }
    .to_rgba8())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn cached(path: &Path) -> Option<RgbaImage> {
    if std::fs::metadata(path).ok()?.len() > MAX_BYTES as u64 {
        return None;
    }
    decode(&std::fs::read(path).ok()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn store_image(key: &str, image: &RgbaImage) {
    use sha1::{Digest, Sha1};
    let dir = crate::storage::data_dir().join("artwork");
    if std::fs::create_dir_all(&dir).is_ok() {
        let path = dir.join(format!("{:x}.png", Sha1::digest(key.as_bytes())));
        let _ = image.save_with_format(path, image::ImageFormat::Png);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn local_candidate<T>(path: &str, mut accept: impl FnMut(&[u8]) -> Option<T>) -> Option<T> {
    use lofty::{file::TaggedFileExt, probe::Probe};
    if let Ok(file) = Probe::open(path).and_then(|p| p.read()) {
        for front in [true, false] {
            for tag in file.tags() {
                for picture in tag.pictures() {
                    if (picture.pic_type() == lofty::picture::PictureType::CoverFront) != front {
                        continue;
                    }
                    if picture.data().len() <= MAX_BYTES {
                        if let Some(value) = accept(picture.data()) {
                            return Some(value);
                        }
                    }
                }
            }
        }
    }
    let parent = Path::new(path).parent()?;
    for name in ["cover", "folder", "front", "album"] {
        for ext in ["jpg", "jpeg", "png", "webp"] {
            let file = parent.join(format!("{name}.{ext}"));
            if std::fs::metadata(&file).is_ok_and(|m| m.len() <= MAX_BYTES as u64) {
                if let Ok(bytes) = std::fs::read(file) {
                    if let Some(value) = accept(&bytes) {
                        return Some(value);
                    }
                }
            }
        }
    }
    None
}

/// Original bytes for cover export; malformed candidates are skipped.
#[cfg(not(target_arch = "wasm32"))]
pub fn local(path: &str) -> Option<Vec<u8>> {
    local_candidate(path, |bytes| decode(bytes).ok().map(|_| bytes.to_vec()))
}

/// Decode each candidate once for display, continuing past malformed covers.
#[cfg(not(target_arch = "wasm32"))]
pub fn local_decoded(path: &str) -> Option<RgbaImage> {
    local_candidate(path, |bytes| decode(bytes).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_webp_and_bounds_large_artwork() {
        let img = image::RgbaImage::from_pixel(1600, 800, image::Rgba([15, 80, 140, 255]));
        let mut bytes = Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::WebP).unwrap();
        let decoded = decode(bytes.get_ref()).unwrap();
        assert_eq!(decoded.dimensions(), (768, 384));
        assert_eq!(decoded.get_pixel(0, 0).0, [15, 80, 140, 255]);
    }
    #[test]
    fn rejects_malformed_and_oversized_input() {
        assert!(decode(b"broken image").is_err());
        assert!(decode(&vec![0; MAX_BYTES + 1]).is_err());
    }
    #[test]
    fn corrupt_cache_is_a_miss() {
        let p = std::env::temp_dir().join(format!("pvf-art-{}.png", std::process::id()));
        std::fs::write(&p, b"truncated").unwrap();
        assert!(cached(&p).is_none());
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn folder_cover_works_without_embedded_tags() {
        let dir = std::env::temp_dir().join(format!(
            "pvf-folder-art-{}-{}",
            std::process::id(),
            crate::services::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbImage::from_pixel(4, 3, image::Rgb([255, 80, 0]))
            .save(dir.join("cover.png"))
            .unwrap();
        let bytes = local(dir.join("untagged.wav").to_str().unwrap()).unwrap();
        assert_eq!(decode(&bytes).unwrap().dimensions(), (4, 3));
        std::fs::remove_file(dir.join("cover.png")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn decoded_local_cover_skips_corrupt_candidates_and_preserves_priority() {
        let dir = std::env::temp_dir().join(format!(
            "pvf-art-fallback-{}-{}",
            std::process::id(),
            crate::services::now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cover.jpg"), b"corrupt").unwrap();
        image::RgbImage::from_pixel(4, 3, image::Rgb([255, 80, 0]))
            .save(dir.join("cover.png"))
            .unwrap();
        image::RgbImage::from_pixel(3, 2, image::Rgb([10, 20, 30]))
            .save(dir.join("folder.png"))
            .unwrap();
        let audio_path = dir.join("untagged.wav");
        let path = audio_path.to_str().unwrap();
        let decoded = local_decoded(path).unwrap();
        assert_eq!(decoded.dimensions(), (4, 3));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 80, 0, 255]);
        assert_eq!(decoded, decode(&local(path).unwrap()).unwrap());
        std::fs::remove_file(dir.join("cover.jpg")).unwrap();
        std::fs::remove_file(dir.join("cover.png")).unwrap();
        std::fs::remove_file(dir.join("folder.png")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn cached_for_keys_reuses_disposable_cover_across_identities() {
        let key = format!("yt://arttest{:04}", std::process::id() % 10000);
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([20, 120, 220, 255]));
        store_cached_image(&key, &img);
        let loaded = cached_for_keys(&["missing-local-path".to_string(), key.clone()]).unwrap();
        assert_eq!(loaded.dimensions(), (8, 8));
        let _ = std::fs::remove_file(cache_path(&key).with_extension("cache.png"));
    }
}
