//! Shared Rust models and parsing, plus the native fallback display adapter.
pub mod credits;
pub mod engine;
pub mod highlight;
pub mod instrumental;
pub mod parsers;
pub mod providers;
pub mod render;
pub mod rics;
pub mod settings;
pub mod text;
pub mod ttml;
pub mod types;
use crate::{
    lyrics::{LyricsAlternative, LyricsSyncType},
    LyricRow, WordRow,
};
use slint::{ModelRc, VecModel};
use std::{collections::HashMap, rc::Rc};

pub fn to_lyrics(alt: &LyricsAlternative) -> Vec<types::Lyric> {
    alt.lyrics
        .lines
        .iter()
        .map(|line| types::Lyric {
            start_time_ms: line.start_ms.min(i64::MAX as u64) as i64,
            duration_ms: line.duration_ms.min(i64::MAX as u64) as i64,
            words: line.text.clone(),
            key: (!line.key.is_empty()).then(|| line.key.clone()),
            agent: (!line.agent.is_empty()).then(|| line.agent.clone()),
            romanization: (!line.romanization.is_empty()).then(|| line.romanization.clone()),
            timed_romanization: (!line.timed_romanization.is_empty()).then(|| {
                line.timed_romanization
                    .iter()
                    .map(|p| types::LyricPart {
                        start_time_ms: p.start_ms as i64,
                        duration_ms: p.duration_ms as i64,
                        words: p.text.clone(),
                        is_background: p.is_background,
                        explicit: p.explicit,
                    })
                    .collect()
            }),
            translations: if line.translations.is_empty() {
                (!line.translation.is_empty())
                    .then(|| HashMap::from([("translation".into(), line.translation.clone())]))
            } else {
                Some(
                    line.translations
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                )
            },
            parts: Some(
                line.parts
                    .iter()
                    .map(|part| types::LyricPart {
                        start_time_ms: part.start_ms.min(i64::MAX as u64) as i64,
                        duration_ms: part.duration_ms.min(i64::MAX as u64) as i64,
                        words: part.text.clone(),
                        is_background: part.is_background,
                        explicit: part.explicit,
                    })
                    .collect(),
            ),
            is_instrumental: line.text.trim().is_empty()
                && !line
                    .parts
                    .iter()
                    .any(|p| p.is_background && !p.text.trim().is_empty()),
            ..Default::default()
        })
        .collect()
}
pub fn display_lyrics(alt: &LyricsAlternative, duration_ms: i64) -> Vec<types::Lyric> {
    let mut normalized = alt.clone();
    normalized.sync_type = effective_sync_type(&normalized);
    if normalized.sync_type != LyricsSyncType::Plain {
        crate::lyrics::complete_timing(&mut normalized.lyrics, duration_ms.max(0) as u64);
    }
    let lyrics = to_lyrics(&normalized);
    if normalized.sync_type == LyricsSyncType::Plain {
        lyrics
    } else {
        instrumental::insert_instrumental_breaks(lyrics, duration_ms, 5000)
    }
}

/// Recover word timing from cached lyrics even when an older parser stored a
/// plain or line-synced label for parts that still contain word cue timestamps.
pub fn effective_sync_type(alt: &LyricsAlternative) -> LyricsSyncType {
    if alt.lyrics.lines.iter().any(|line| {
        line.parts
            .iter()
            .any(|part| part.duration_ms > 0 || part.start_ms != line.start_ms)
    }) {
        LyricsSyncType::RichSync
    } else if alt.sync_type != LyricsSyncType::Plain {
        alt.sync_type
    } else if alt
        .lyrics
        .lines
        .iter()
        .any(|line| line.start_ms > 0 || line.duration_ms > 0)
    {
        LyricsSyncType::Synced
    } else {
        LyricsSyncType::Plain
    }
}

pub fn web_document(alt: &LyricsAlternative, duration_ms: i64) -> String {
    serde_json::json!({
        "lyrics": display_lyrics(alt, duration_ms),
        "options": {"songwriters": alt.lyrics.songwriters, "language": alt.lyrics.language}
    })
    .to_string()
}

pub fn rows(alt: &LyricsAlternative, duration_ms: i64) -> Vec<LyricRow> {
    let time = |n: i64| n.clamp(0, i32::MAX as i64) as i32;
    display_lyrics(alt, duration_ms)
        .into_iter()
        .map(|line| {
            let parts = line.parts.unwrap_or_default();
            let rtl = text::is_rtl(&line.words);
            LyricRow {
                start_ms: time(line.start_time_ms),
                end_ms: time(line.start_time_ms.saturating_add(line.duration_ms)),
                text: line.words.into(),
                instrumental: line.is_instrumental,
                romanization: line.romanization.unwrap_or_default().into(),
                roman_words: ModelRc::from(Rc::new(VecModel::from(
                    line.timed_romanization
                        .unwrap_or_default()
                        .into_iter()
                        .map(|part| WordRow {
                            text: part.words.into(),
                            start_ms: time(part.start_time_ms),
                            end_ms: time(part.start_time_ms.saturating_add(part.duration_ms)),
                            background: false,
                            explicit: part.explicit,
                        })
                        .collect::<Vec<_>>(),
                ))),
                agent: line.agent.unwrap_or_default().into(),
                rtl,
                translation: line
                    .translations
                    .and_then(|t| {
                        t.get("uk")
                            .or_else(|| t.get("translation"))
                            .or_else(|| t.get("en"))
                            .or_else(|| t.values().next())
                            .cloned()
                    })
                    .unwrap_or_default()
                    .into(),
                background_text: parts
                    .iter()
                    .filter(|p| p.is_background)
                    .map(|p| p.words.as_str())
                    .collect::<String>()
                    .into(),
                words: ModelRc::from(Rc::new(VecModel::from(
                    parts
                        .into_iter()
                        .map(|part| WordRow {
                            text: part.words.into(),
                            start_ms: time(part.start_time_ms),
                            end_ms: time(part.start_time_ms.saturating_add(part.duration_ms)),
                            background: part.is_background,
                            explicit: part.explicit,
                        })
                        .collect::<Vec<_>>(),
                ))),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uses_braccato_gap_models_without_losing_background_or_translation() {
        let (mut lyrics, format, sync_type) = crate::lyrics::parse_payload(
            "[00:10.00]<00:10.00>Hello <00:11.00>world<00:12.00> [bg:(hey)]\n[00:22.00]Next",
        )
        .unwrap();
        lyrics.lines[0].translation = "Переклад".into();
        let a = LyricsAlternative {
            provider: "test".into(),
            lyrics,
            format: format.into(),
            sync_type,
        };
        let rows = display_lyrics(&a, 25000);
        assert!(rows[0].is_instrumental);
        assert_eq!(rows[1].words, "Hello world");
        assert_eq!(
            rows[1].translations.as_ref().unwrap()["translation"],
            "Переклад"
        );
        assert!(rows[1]
            .parts
            .as_ref()
            .unwrap()
            .iter()
            .any(|p| p.is_background));
        assert!(rows[2].is_instrumental);
    }
    #[test]
    fn plain_lyrics_have_no_artificial_instrumentals() {
        let a = LyricsAlternative {
            provider: "test".into(),
            lyrics: crate::lyrics::parse_plain("One\nTwo"),
            format: "txt".into(),
            sync_type: LyricsSyncType::Plain,
        };
        assert_eq!(display_lyrics(&a, 180000).len(), 2);
    }
}
