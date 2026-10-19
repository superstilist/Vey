//! Public native equivalent of @braccato/parsers. This is the common entry point
//! for a complete document with timing normalization, metadata and gap insertion.
use super::types::Lyric;
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Format {
    Ttml,
    Lrc,
    Qrc,
    Srt,
    Vtt,
    Plain,
}
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub duration_ms: i64,
    pub instrumental_gap_ms: i64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            duration_ms: 0,
            instrumental_gap_ms: 5000,
        }
    }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub format: Format,
    pub lyrics: Vec<Lyric>,
    pub language: Option<String>,
    pub songwriters: Vec<String>,
}
pub fn parse(input: &str, options: Options) -> Option<Document> {
    let (mut parsed, format, sync_type) = crate::lyrics::parse_payload(input)?;
    if sync_type != crate::lyrics::LyricsSyncType::Plain {
        crate::lyrics::complete_timing(&mut parsed, options.duration_ms.max(0) as u64);
    }
    let format = match format {
        "ttml" => Format::Ttml,
        "qrc" => Format::Qrc,
        "srt" => Format::Srt,
        "vtt" => Format::Vtt,
        "lrc" => Format::Lrc,
        _ => Format::Plain,
    };
    let language = parsed.language.clone();
    let songwriters = parsed.songwriters.clone();
    let alt = crate::lyrics::LyricsAlternative {
        provider: String::new(),
        format: String::new(),
        lyrics: parsed,
        sync_type,
    };
    let mut lyrics = super::to_lyrics(&alt);
    if sync_type != crate::lyrics::LyricsSyncType::Plain {
        lyrics = super::instrumental::insert_instrumental_breaks(
            lyrics,
            options.duration_ms,
            options.instrumental_gap_ms,
        );
    }
    Some(Document {
        format,
        lyrics,
        language,
        songwriters,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fills_duration_and_blanks_without_dropping_singing_gaps() {
        let l = parse(
            "[00:01.00]A\n[00:04.00]\n[00:08.00]\n[00:20.00]B",
            Options {
                duration_ms: 30000,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(l.lyrics.iter().filter(|l| l.is_instrumental).count(), 1);
        let gap = l.lyrics.iter().find(|l| l.is_instrumental).unwrap();
        assert_eq!((gap.start_time_ms, gap.duration_ms), (4000, 16000));
    }
}
