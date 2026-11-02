//! Permanent file artwork and metadata. All callers run file work on workers.
use lofty::{
    config::WriteOptions,
    picture::{MimeType, Picture, PictureType},
    prelude::*,
    probe::Probe,
    tag::{ItemKey, Tag, TagType},
};
use std::{io::Cursor, path::Path};

/// Comment prefix used to persist the YouTube Music identity beside the audio.
pub const YTM_ID_PREFIX: &str = "ytmusic:";

/// The 11-character YouTube/YouTube Music video id, if the text contains one.
/// Accepts the stored `ytmusic:<id>` comment, a bare id, or a public video URL.
pub fn ytmusic_id(text: &str) -> Option<String> {
    fn valid(id: &str) -> bool {
        id.len() == 11
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }
    let text = text.trim();
    let candidate = match text.strip_prefix(YTM_ID_PREFIX) {
        Some(id) => id.trim(),
        None => text
            .split_once("v=")
            .map(|(_, id)| id)
            .or_else(|| text.rsplit('/').next())
            .unwrap_or(text),
    };
    let candidate = candidate
        .split(['&', '#', '?', ' '])
        .next()
        .unwrap_or(candidate)
        .trim();
    valid(candidate).then(|| candidate.to_string())
}

/// Parse the ` [video-id]` suffix that downloaded files carry.
pub fn id_from_file_stem(stem: &str) -> Option<String> {
    let start = stem.rfind('[')?;
    let id = stem.get(start + 1..)?.strip_suffix(']')?;
    ytmusic_id(id).filter(|_| !stem[..start].trim().is_empty())
}

pub async fn fetch_cover(url: &str) -> Result<Vec<u8>, String> {
    static CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> = std::sync::OnceLock::new();
    let client = CLIENT.get_or_init(|| reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())).as_ref().map_err(Clone::clone)?;
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.without_url().to_string())?;
    if response
        .content_length()
        .is_some_and(|size| size > crate::artwork::MAX_BYTES as u64)
    {
        return Err("Обкладинка завелика".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| e.without_url().to_string())?
    {
        if bytes.len() + chunk.len() > crate::artwork::MAX_BYTES {
            return Err("Обкладинка завелика".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub fn jpeg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = crate::artwork::decode(bytes)?;
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .to_rgb8()
        .write_to(&mut output, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;
    Ok(output.into_inner())
}

/// Preserve other pictures and tags; replace only the front cover.
pub fn write_tags(
    path: &Path,
    title: &str,
    artist: &str,
    album: &str,
    ytm_id: Option<&str>,
    cover: Option<&[u8]>,
) -> Result<(), String> {
    let mut file = Probe::open(path)
        .and_then(|p| p.read())
        .map_err(|e| e.to_string())?;
    let tag_type = match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp3" => TagType::Id3v2,
        "m4a" | "m4b" | "mp4" => TagType::Mp4Ilst,
        "flac" | "ogg" | "oga" | "opus" => TagType::VorbisComments,
        _ => return Err("Цей формат не підтримує запис обкладинки".into()),
    };
    if file.tag(tag_type).is_none() {
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = file.tag_mut(tag_type).ok_or("Не вдалося відкрити теги")?;
    if !title.is_empty() {
        tag.set_title(title.to_owned());
    }
    if !artist.is_empty() {
        tag.set_artist(artist.to_owned());
    }
    if !album.is_empty() {
        tag.set_album(album.to_owned());
    }
    if let Some(id) = ytm_id.and_then(ytmusic_id) {
        tag.insert_text(ItemKey::Comment, format!("{YTM_ID_PREFIX}{id}"));
    }
    if let Some(bytes) = cover {
        let cover = jpeg(bytes)?;
        tag.remove_picture_type(PictureType::CoverFront);
        tag.push_picture(Picture::new_unchecked(
            PictureType::CoverFront,
            Some(MimeType::Jpeg),
            None,
            cover,
        ));
    }
    file.save_to_path(path, WriteOptions::default())
        .map_err(|e| e.to_string())
}

/// Edit a copy first. The original front cover remains in an adjacent backup.
pub fn replace_cover(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let stage = path.with_extension(format!(
        "cover-edit-{}.{}.{}",
        std::process::id(),
        crate::services::now_ms(),
        ext
    ));
    let result = (|| {
        std::fs::copy(path, &stage).map_err(|e| e.to_string())?;
        write_tags(&stage, "", "", "", None, Some(bytes))?;
        let original = Probe::open(path)
            .and_then(|p| p.read())
            .map_err(|e| e.to_string())?;
        for (index, picture) in original
            .tags()
            .iter()
            .flat_map(|tag| tag.pictures())
            .filter(|p| p.pic_type() == PictureType::CoverFront)
            .enumerate()
        {
            let ext = match image::guess_format(picture.data()) {
                Ok(image::ImageFormat::Jpeg) => "jpg",
                Ok(image::ImageFormat::Png) => "png",
                Ok(image::ImageFormat::WebP) => "webp",
                _ => "bin",
            };
            let backup = path.with_extension(format!(
                "cover-backup-{}-{index}.{ext}",
                crate::services::now_ms()
            ));
            std::fs::write(backup, picture.data()).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&stage, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(stage);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stored_and_bare_video_ids() {
        assert_eq!(
            ytmusic_id("ytmusic:dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(ytmusic_id("dQw4w9WgXcQ").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(
            ytmusic_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=RD").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            ytmusic_id("https://youtu.be/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn rejects_non_video_comments() {
        assert_eq!(ytmusic_id("some listener comment"), None);
        assert_eq!(ytmusic_id("ytmusic:short"), None);
        assert_eq!(ytmusic_id(""), None);
    }

    #[test]
    fn reads_download_filename_suffix() {
        assert_eq!(
            id_from_file_stem("Artist - Title [dQw4w9WgXcQ]").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            id_from_file_stem("Artist - Title [Live]"),
            None,
            "ordinary brackets are not ids"
        );
        assert_eq!(id_from_file_stem("[dQw4w9WgXcQ]"), None);
    }
}
