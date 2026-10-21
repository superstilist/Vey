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
    for line in &mut normalized.lyrics.lines {
        let part_timed = crate::lyrics::line_has_part_level_timing(line);
        let background_timed = crate::lyrics::line_has_background_part_timing(line);
        for part in &mut line.parts {
            // Untimed backing phrases must not be divided into estimated words
            // just because the lead vocal has real word cues.
            let timed = if part.is_background {
                background_timed
            } else {
                part_timed
            };
            if !timed || !part.start_time_explicit || part.text.split_whitespace().count() > 1 {
                part.start_ms = line.start_ms;
                part.duration_ms = 0;
            }
        }
        let roman = crate::lyrics::LyricLine {
            text: line.romanization.clone(),
            parts: line.timed_romanization.clone(),
            ..Default::default()
        };
        if !crate::lyrics::line_has_part_level_timing(&roman) {
            line.timed_romanization.clear();
        }
    }
    let lyrics = to_lyrics(&normalized);
    if normalized.sync_type == LyricsSyncType::Plain {
        lyrics
    } else {
        instrumental::insert_instrumental_breaks(lyrics, duration_ms, 5000)
    }
}

/// Resolve the display mode without upgrading line cues to word-level sync.
pub fn effective_sync_type(alt: &LyricsAlternative) -> LyricsSyncType {
    if crate::lyrics::has_syllable_level_timing(&alt.lyrics) {
        return LyricsSyncType::RichSync;
    }
    if crate::lyrics::has_part_level_timing(&alt.lyrics) {
        return match alt.format.as_str() {
            "ttml" | "qrc" | "json" => LyricsSyncType::RichSync,
            _ => LyricsSyncType::Word,
        };
    }

    let has_line_timing = crate::lyrics::has_any_timing(&alt.lyrics);
    if has_line_timing || alt.sync_type == LyricsSyncType::Synced {
        LyricsSyncType::Synced
    } else {
        LyricsSyncType::Plain
    }
}

/// Hide optional display layers without changing cached/exported source lyrics.
pub fn visible_layers(
    alt: &LyricsAlternative,
    romanization: bool,
    translation: bool,
) -> LyricsAlternative {
    let mut display = alt.clone();
    for line in &mut display.lyrics.lines {
        if !romanization {
            line.romanization.clear();
            line.timed_romanization.clear();
        }
        if !translation {
            line.translation.clear();
            line.translations.clear();
        }
    }
    display
}

pub fn web_document(alt: &LyricsAlternative, duration_ms: i64) -> String {
    serde_json::json!({
        "lyrics": display_lyrics(alt, duration_ms),
        "options": {
            "songwriters": alt.lyrics.songwriters,
            "language": alt.lyrics.language,
            "syncType": match effective_sync_type(alt) {
                LyricsSyncType::Plain => "none",
                LyricsSyncType::Synced => "synced",
                _ => "richsync",
            }
        }
    })
    .to_string()
}

pub fn rows(alt: &LyricsAlternative, duration_ms: i64) -> Vec<LyricRow> {
    let time = |n: i64| n.clamp(0, i32::MAX as i64) as i32;
    let word_timed_lines = alt
        .lyrics
        .lines
        .iter()
        .filter(|line| {
            crate::lyrics::line_has_part_level_timing(line)
                || crate::lyrics::line_has_background_part_timing(line)
        })
        .map(|line| (line.start_ms, line.text.clone()))
        .collect::<std::collections::HashSet<_>>();
    display_lyrics(alt, duration_ms)
        .into_iter()
        .map(|line| {
            let word_timed =
                word_timed_lines.contains(&(line.start_time_ms.max(0) as u64, line.words.clone()));
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
                words: ModelRc::from(Rc::new(VecModel::from(if word_timed {
                    parts
                        .into_iter()
                        .map(|part| WordRow {
                            text: part.words.into(),
                            start_ms: time(part.start_time_ms),
                            end_ms: time(part.start_time_ms.saturating_add(part.duration_ms)),
                            background: part.is_background,
                            explicit: part.explicit,
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }))),
                ..Default::default()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;
    #[test]
    fn line_sync_stays_line_level_while_real_word_times_are_kept() {
        let (line, format, sync_type) = crate::lyrics::parse_payload(
            "<tt><body><p begin='1s' end='3s'><span begin='1s' end='3s'>First </span><span begin='1s' end='3s'>line</span></p></body></tt>",
        )
        .unwrap();
        assert_eq!(sync_type, LyricsSyncType::Synced);
        assert_eq!(line.lines[0].text, "First line");
        let line_alt = LyricsAlternative {
            provider: "line".into(),
            format: format.into(),
            lyrics: line.clone(),
            sync_type,
        };
        assert_eq!(effective_sync_type(&line_alt), LyricsSyncType::Synced);
        let rendered = display_lyrics(&line_alt, 25000);
        assert_eq!(
            rendered[0]
                .parts
                .as_ref()
                .unwrap()
                .iter()
                .map(|part| (part.start_time_ms, part.duration_ms))
                .collect::<Vec<_>>(),
            [(1000, 0), (1000, 0)]
        );
        assert_eq!(line_alt.lyrics, line);
        let mixed = LyricsAlternative {
            provider: "mixed".into(),
            format: "lrc".into(),
            sync_type: LyricsSyncType::Word,
            lyrics: crate::lyrics::parse_lrc(
                "[00:01.00]<00:01.00>First <00:02.00>line\n[00:03.00]Second line",
            ),
        };
        let mixed_rows = display_lyrics(&mixed, 10000);
        let synced_line = mixed_rows
            .iter()
            .find(|row| row.words == "Second line")
            .unwrap();
        let model_rows = rows(&mixed, 10000);
        assert_eq!(
            model_rows
                .iter()
                .find(|row| row.text == "First line")
                .unwrap()
                .words
                .row_count(),
            2
        );
        assert_eq!(
            model_rows
                .iter()
                .find(|row| row.text == "Second line")
                .unwrap()
                .words
                .row_count(),
            0
        );
        let original_line = mixed
            .lyrics
            .lines
            .iter()
            .find(|line| line.text == "Second line")
            .unwrap();
        assert_eq!(
            synced_line
                .parts
                .as_ref()
                .unwrap()
                .iter()
                .map(|part| (part.start_time_ms, part.duration_ms))
                .collect::<Vec<_>>(),
            original_line
                .parts
                .iter()
                .map(|part| (part.start_ms as i64, part.duration_ms as i64))
                .collect::<Vec<_>>()
        );
        let stale_word_label = LyricsAlternative {
            sync_type: LyricsSyncType::RichSync,
            ..line_alt.clone()
        };
        assert_eq!(
            effective_sync_type(&stale_word_label),
            LyricsSyncType::Synced
        );
        let unsynced = LyricsAlternative {
            provider: "plain".into(),
            format: "json".into(),
            sync_type: LyricsSyncType::RichSync,
            lyrics: crate::lyrics::parse_plain("plain text"),
        };
        assert_eq!(effective_sync_type(&unsynced), LyricsSyncType::Plain);
        let (word_line, word_format, word_sync) = crate::lyrics::parse_payload(
            "<tt><body><p begin='1s' end='3s'><span begin='1s' end='2s'>First </span><span begin='2s' end='3s'>line</span></p></body></tt>",
        )
        .unwrap();
        let word_alt = LyricsAlternative {
            provider: "word".into(),
            format: word_format.into(),
            lyrics: word_line,
            sync_type: word_sync,
        };
        assert_eq!(effective_sync_type(&word_alt), LyricsSyncType::RichSync);
    }

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
