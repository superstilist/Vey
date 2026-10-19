//! Local ↔ YouTube Music sync: read file tags, overwrite them from a YT match,
//! and cache YT audio into the library.
//!
//! Every overwrite writes a JSON backup first (`tag-backups/<sha1>-<ms>.json`
//! in the app data dir), so the original tags can be restored per file. Text
//! fields are only overwritten when the incoming value is non-empty —
//! blanking a title with an empty string would corrupt, not sync.

use lofty::config::WriteOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag, TagType};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Plain tag values shared between the UI, backups, and the file writer.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TagSnapshot {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<i32>,
    pub track_no: Option<u32>,
}

/// One backup row: the newest file per path wins on restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagBackupMeta {
    pub id: String,
    pub path: String,
    pub saved_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagBackupFile {
    path: String,
    saved_at: u64,
    tags: TagSnapshot,
}

/// Extensions the writer supports, mapped to the tag container lofty can
/// persist for them. Anything else (video containers, ancient codecs) reads
/// fine elsewhere but is refused here instead of half-written.
fn tag_type_for_ext(ext: &str) -> Option<TagType> {
    match ext {
        "mp3" => Some(TagType::Id3v2),
        "m4a" | "m4b" | "mp4" | "mov" => Some(TagType::Mp4Ilst),
        "ogg" | "oga" | "opus" => Some(TagType::VorbisComments),
        "flac" => Some(TagType::VorbisComments),
        "wav" => Some(TagType::RiffInfo),
        "aif" | "aiff" => Some(TagType::AiffText),
        _ => None,
    }
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn backups_dir(app: &Path) -> Result<PathBuf, String> {
    let dir = app.join("tag-backups");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create backup dir: {e}"))?;
    Ok(dir)
}

fn backup_id_for(path: &str, saved_at: u64) -> String {
    let digest = format!("{:x}", Sha1::digest(path.as_bytes()));
    format!("{digest}-{saved_at}")
}

/// Parse `<sha1-hex>-<nanos>.json` backup filenames.
fn parse_backup_id(file_name: &str) -> Option<(String, u64)> {
    let stem = file_name.strip_suffix(".json")?;
    let (hash, millis) = stem.split_once('-')?;
    if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let saved_at = millis.parse::<u64>().ok()?;
    Some((hash.to_string(), saved_at))
}

fn snapshot_from_tag(tag: &Tag) -> TagSnapshot {
    TagSnapshot {
        title: tag.title().as_deref().unwrap_or("").to_string(),
        artist: tag.artist().as_deref().unwrap_or("").to_string(),
        album: tag.album().as_deref().unwrap_or("").to_string(),
        album_artist: tag
            .get_string(&ItemKey::AlbumArtist)
            .unwrap_or("")
            .to_string(),
        genre: tag.genre().as_deref().unwrap_or("").to_string(),
        year: tag.year().map(|y| y as i32),
        track_no: tag.track(),
    }
}

fn read_snapshot(path: &Path) -> Result<TagSnapshot, String> {
    let tagged = Probe::open(path)
        .map_err(|e| format!("cannot open file: {e}"))?
        .read()
        .map_err(|e| format!("cannot read tags: {e}"))?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    Ok(tag.map(snapshot_from_tag).unwrap_or_default())
}

/// Overwrite `tag` from `next`, skipping empty text and missing numbers so a
/// partial YT match never blanks fields it has nothing for.
fn apply_snapshot(tag: &mut Tag, next: &TagSnapshot) {
    if !next.title.trim().is_empty() {
        tag.set_title(next.title.clone());
    }
    if !next.artist.trim().is_empty() {
        tag.set_artist(next.artist.clone());
    }
    if !next.album.trim().is_empty() {
        tag.set_album(next.album.clone());
    }
    if !next.album_artist.trim().is_empty() {
        tag.insert_text(ItemKey::AlbumArtist, next.album_artist.clone());
    }
    if !next.genre.trim().is_empty() {
        tag.set_genre(next.genre.clone());
    }
    if let Some(year) = next.year {
        if year > 0 {
            tag.set_year(year as u32);
        }
    }
    if let Some(track_no) = next.track_no {
        if track_no > 0 {
            tag.set_track(track_no);
        }
    }
}

fn write_backup_file(dir: &Path, path: &str, tags: &TagSnapshot) -> Result<TagBackupMeta, String> {
    let saved_at = now_nanos();
    let id = backup_id_for(path, saved_at);
    let file = TagBackupFile {
        path: path.to_string(),
        saved_at,
        tags: tags.clone(),
    };
    let raw = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{id}.json")), raw).map_err(|e| e.to_string())?;
    Ok(TagBackupMeta {
        id,
        path: path.to_string(),
        saved_at,
    })
}

fn read_backup_file(dir: &Path, file_name: &str) -> Result<TagBackupFile, String> {
    parse_backup_id(file_name).ok_or_else(|| "unknown backup id".to_string())?;
    let raw = std::fs::read_to_string(dir.join(file_name)).map_err(|e| e.to_string())?;
    serde_json::from_str::<TagBackupFile>(&raw).map_err(|e| e.to_string())
}

/// Newest backup covering `path`, if any.
fn latest_backup_for(dir: &Path, path: &str) -> Result<Option<(String, TagBackupFile)>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut best: Option<(String, TagBackupFile)> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if parse_backup_id(&name).is_none() {
            continue;
        }
        let Ok(file) = read_backup_file(dir, &name) else {
            continue;
        };
        if file.path != path {
            continue;
        }
        let newer = best
            .as_ref()
            .is_none_or(|(_, current)| file.saved_at > current.saved_at);
        if newer {
            best = Some((name.trim_end_matches(".json").to_string(), file));
        }
    }
    Ok(best)
}

fn check_writable(path: &Path) -> Result<(PathBuf, TagType), String> {
    if !path.is_file() {
        return Err("file not found".to_string());
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let tag_type = tag_type_for_ext(&ext)
        .ok_or_else(|| format!("tag writing is not supported for .{ext} files"))?;
    Ok((path.to_path_buf(), tag_type))
}

/// Read the current tags of a local file (empty snapshot when untagged).

pub async fn local_read_tags(path: String) -> Result<TagSnapshot, String> {
    let path = Path::new(path.trim());
    if !path.is_file() {
        return Err("file not found".to_string());
    }
    read_snapshot(path)
}

/// Overwrite a local file's tags, writing a JSON backup of the originals
/// first when `backup` is set. Returns the backup row (or a stub with an
/// empty id when backups were skipped).

pub async fn local_write_tags(
    app: PathBuf,
    path: String,
    tags: TagSnapshot,
    backup: Option<bool>,
) -> Result<TagBackupMeta, String> {
    let path = path.trim().to_string();
    let (fs_path, tag_type) = check_writable(Path::new(&path))?;
    let original = read_snapshot(&fs_path).unwrap_or_default();
    let meta = if backup.unwrap_or(true) {
        let dir = backups_dir(&app)?;
        write_backup_file(&dir, &path, &original)?
    } else {
        TagBackupMeta {
            id: String::new(),
            path: path.clone(),
            saved_at: now_nanos(),
        }
    };
    let mut tagged = Probe::open(&fs_path)
        .map_err(|e| format!("cannot open file: {e}"))?
        .read()
        .map_err(|e| format!("cannot read tags: {e}"))?;
    if let Some(tag) = tagged.primary_tag_mut() {
        apply_snapshot(tag, &tags);
    } else {
        let mut tag = Tag::new(tag_type);
        apply_snapshot(&mut tag, &tags);
        tagged.insert_tag(tag);
    }
    tagged
        .save_to_path(&fs_path, WriteOptions::default())
        .map_err(|e| format!("cannot save tags: {e}"))?;
    Ok(meta)
}

/// Restore the newest backup for a file and return the restored snapshot.

pub async fn local_restore_tags(app: PathBuf, path: String) -> Result<TagSnapshot, String> {
    let path = path.trim().to_string();
    let (fs_path, tag_type) = check_writable(Path::new(&path))?;
    let dir = backups_dir(&app)?;
    let (_, file) =
        latest_backup_for(&dir, &path)?.ok_or_else(|| "no backup for this file".to_string())?;
    let mut tagged = Probe::open(&fs_path)
        .map_err(|e| format!("cannot open file: {e}"))?
        .read()
        .map_err(|e| format!("cannot read tags: {e}"))?;
    if let Some(tag) = tagged.primary_tag_mut() {
        apply_snapshot(tag, &file.tags);
    } else {
        let mut tag = Tag::new(tag_type);
        apply_snapshot(&mut tag, &file.tags);
        tagged.insert_tag(tag);
    }
    tagged
        .save_to_path(&fs_path, WriteOptions::default())
        .map_err(|e| format!("cannot save tags: {e}"))?;
    Ok(file.tags)
}

/// Newest-first backup rows across all synced files.

pub async fn local_list_backups(app: PathBuf) -> Result<Vec<TagBackupMeta>, String> {
    let dir = backups_dir(&app)?;
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    let mut seen: std::collections::HashMap<String, TagBackupMeta> =
        std::collections::HashMap::new();
    for entry in entries.flatten().take(2000) {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some((_, _)) = parse_backup_id(&name) else {
            continue;
        };
        let id = name.trim_end_matches(".json").to_string();
        let Ok(file) = read_backup_file(&dir, &name) else {
            continue;
        };
        let newer = seen
            .get(&file.path)
            .is_none_or(|current| file.saved_at > current.saved_at);
        if newer {
            seen.insert(
                file.path.clone(),
                TagBackupMeta {
                    id,
                    path: file.path,
                    saved_at: file.saved_at,
                },
            );
        }
    }
    let mut metas: Vec<TagBackupMeta> = seen.into_values().collect();
    metas.sort_by_key(|m| std::cmp::Reverse(m.saved_at));
    Ok(metas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_type_covers_common_library_containers() {
        assert_eq!(tag_type_for_ext("mp3"), Some(TagType::Id3v2));
        assert_eq!(tag_type_for_ext("m4a"), Some(TagType::Mp4Ilst));
        assert_eq!(tag_type_for_ext("ogg"), Some(TagType::VorbisComments));
        assert_eq!(tag_type_for_ext("opus"), Some(TagType::VorbisComments));
        assert_eq!(tag_type_for_ext("flac"), Some(TagType::VorbisComments));
        assert_eq!(tag_type_for_ext("wav"), Some(TagType::RiffInfo));
        assert_eq!(tag_type_for_ext("mkv"), None);
        assert_eq!(tag_type_for_ext(""), None);
    }

    #[test]
    fn backup_ids_round_trip_through_filenames() {
        let (hash, saved_at) =
            parse_backup_id(&format!("{}-123.json", "a".repeat(40))).expect("id");
        assert_eq!(hash, "a".repeat(40));
        assert_eq!(saved_at, 123);
        assert!(parse_backup_id("junk.json").is_none());
        assert!(parse_backup_id("abc-123.json").is_none());
        assert!(parse_backup_id(&format!("{}-x.json", "b".repeat(40))).is_none());
    }

    #[test]
    fn apply_skips_empty_fields_but_overwrites_the_rest() {
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("Old".to_string());
        tag.set_artist("Old Artist".to_string());
        tag.set_album("Old Album".to_string());
        apply_snapshot(
            &mut tag,
            &TagSnapshot {
                title: "New".to_string(),
                artist: "  ".to_string(),
                album: "New Album".to_string(),
                album_artist: "New AA".to_string(),
                genre: String::new(),
                year: Some(2024),
                track_no: None,
            },
        );
        assert_eq!(tag.title().as_deref(), Some("New"));
        assert_eq!(tag.artist().as_deref(), Some("Old Artist"));
        assert_eq!(tag.album().as_deref(), Some("New Album"));
        assert_eq!(tag.get_string(&ItemKey::AlbumArtist), Some("New AA"));
        assert_eq!(tag.year(), Some(2024));
    }

    #[test]
    fn backup_files_round_trip_and_latest_wins() {
        let dir = std::env::temp_dir().join(format!(
            "playervf-sync-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let snap = TagSnapshot {
            title: "T".to_string(),
            ..TagSnapshot::default()
        };
        let first = write_backup_file(&dir, "C:\\music\\a.mp3", &snap).expect("write");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = write_backup_file(&dir, "C:\\music\\a.mp3", &snap).expect("write");
        write_backup_file(&dir, "C:\\music\\b.mp3", &snap).expect("write");
        assert_ne!(first.id, second.id);
        let (_, latest) = latest_backup_for(&dir, "C:\\music\\a.mp3")
            .expect("latest")
            .expect("found");
        assert!(latest.saved_at >= first.saved_at);
        assert_eq!(latest.tags.title, "T");
        assert!(latest_backup_for(&dir, "C:\\music\\missing.mp3")
            .expect("none")
            .is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
