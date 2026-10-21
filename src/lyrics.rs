//! Rust lyric primitives used by the Slint renderer.
//!
//! The parser deliberately keeps timing and background-vocal data separate
//! from presentation so provider adapters can feed the same view model.

use regex::Regex;
use std::sync::LazyLock;
static QRC_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*\[\d+,\d+\]").unwrap());
static QRC_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\((\d+),(\d+)(?:,\d+)?\)").unwrap());
static QRC_CONTENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)LyricContent\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap());
static LRC_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\d{1,4}:\d{2}(?:[.,:]\d{1,3})?\]").unwrap());
static LRC_WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<\d{1,4}:\d{2}(?:[.,:]\d{1,3})?>").unwrap());
static SRT_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^\s*\d{1,2}:\d{2}(?::\d{2})?[,.]\d{1,6}\s*-->\s*\d{1,2}:\d{2}(?::\d{2})?[,.]\d{1,6}",
    )
    .unwrap()
});
static SUBTITLE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"</?[A-Za-z][^>]*>").unwrap());
static TTML_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(?:[A-Za-z_][\w.-]*:)?tt(?:\s|/?>)").unwrap());
static TTML_TIMING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:begin|end|dur)\s*=").unwrap());
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LyricPart {
    #[serde(default)]
    pub explicit: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub start_time_explicit: bool,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub text: String,
    pub is_background: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LyricLine {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub romanization: String,
    #[serde(default)]
    pub timed_romanization: Vec<LyricPart>,
    #[serde(default)]
    pub translations: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub translation: String,
    pub start_ms: u64,
    pub duration_ms: u64,
    pub text: String,
    pub parts: Vec<LyricPart>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParsedLyrics {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub songwriters: Vec<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub offset_ms: i64,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LyricsSyncType {
    RichSync,
    Word,
    Synced,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LyricsAlternative {
    pub provider: String,
    pub sync_type: LyricsSyncType,
    pub format: String,
    pub lyrics: ParsedLyrics,
}

pub fn line_has_word_level_timing(line: &LyricLine) -> bool {
    let parts = line
        .parts
        .iter()
        .filter(|part| !part.is_background && !part.text.trim().is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2
        || !parts.iter().all(|part| part.start_time_explicit)
        || !parts.iter().any(|part| part.start_ms != parts[0].start_ms)
    {
        return false;
    }

    let has_cjk = line.text.chars().any(|ch| {
        matches!(
            ch as u32,
            0x2E80..=0x2FFF
                | 0x3000..=0x30FF
                | 0x3100..=0x312F
                | 0x3130..=0x318F
                | 0x31A0..=0x31BF
                | 0x31F0..=0x31FF
                | 0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xAC00..=0xD7AF
                | 0xF900..=0xFAFF
                | 0xFF00..=0xFFEF
                | 0x20000..=0x323AF
        )
    });
    if has_cjk {
        // QRC may time Chinese characters or syllables without spaces.
        // Validate the concatenated cue text, ignoring only whitespace.
        let compact = |text: &str| {
            text.chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
        };
        let reconstructed = parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<String>();
        compact(&reconstructed) == compact(&line.text)
    } else {
        let line_word_count = line.text.split_whitespace().count();
        line_word_count > 1
            && parts
                .iter()
                .all(|part| part.text.split_whitespace().count() == 1)
            && parts.len() == line_word_count
            && parts
                .iter()
                .map(|part| part.text.trim())
                .collect::<Vec<_>>()
                .join(" ")
                == line.text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

pub fn has_word_level_timing(lyrics: &ParsedLyrics) -> bool {
    lyrics.lines.iter().any(line_has_word_level_timing)
}

/// Explicit cues inside a word, as used by Braccato/Unison TTML and QRC.
/// Keep the source's whitespace: adjacent spans can be syllables, whereas a
/// whitespace-only node between spans can be an intentional word boundary.
pub fn line_has_syllable_level_timing(line: &LyricLine) -> bool {
    let lead = line
        .parts
        .iter()
        .filter(|part| !part.is_background)
        .collect::<Vec<_>>();
    let cues = lead
        .iter()
        .copied()
        .filter(|part| !part.text.trim().is_empty())
        .collect::<Vec<_>>();
    if cues.len() < 2
        || cues.len() <= line.text.split_whitespace().count()
        || cues
            .iter()
            .any(|part| !part.start_time_explicit || part.text.split_whitespace().count() != 1)
        || cues
            .windows(2)
            .any(|pair| pair[1].start_ms < pair[0].start_ms)
        || !cues.iter().any(|part| part.start_ms != cues[0].start_ms)
    {
        return false;
    }
    let normalize = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let reconstructed = lead
        .iter()
        .map(|part| part.text.as_str())
        .collect::<String>();
    normalize(&reconstructed) == normalize(&line.text)
}

pub fn line_has_part_level_timing(line: &LyricLine) -> bool {
    line_has_word_level_timing(line) || line_has_syllable_level_timing(line)
}

fn background_timing_line(line: &LyricLine) -> LyricLine {
    let parts = line
        .parts
        .iter()
        .filter(|part| part.is_background)
        .cloned()
        .map(|mut part| {
            part.is_background = false;
            part
        })
        .collect::<Vec<_>>();
    LyricLine {
        text: parts.iter().map(|part| part.text.as_str()).collect(),
        parts,
        ..Default::default()
    }
}

pub fn line_has_background_part_timing(line: &LyricLine) -> bool {
    line_has_part_level_timing(&background_timing_line(line))
}

pub fn has_syllable_level_timing(lyrics: &ParsedLyrics) -> bool {
    lyrics.lines.iter().any(|line| {
        line_has_syllable_level_timing(line)
            || line_has_syllable_level_timing(&background_timing_line(line))
    })
}

pub fn has_part_level_timing(lyrics: &ParsedLyrics) -> bool {
    lyrics
        .lines
        .iter()
        .any(|line| line_has_part_level_timing(line) || line_has_background_part_timing(line))
}

pub fn has_any_timing(lyrics: &ParsedLyrics) -> bool {
    lyrics.lines.iter().any(|line| {
        line.start_ms > 0
            || line.duration_ms > 0
            || line
                .parts
                .iter()
                .any(|part| part.start_ms > 0 || part.duration_ms > 0)
    })
}

fn parse_lrc_time(value: &str) -> Option<u64> {
    let (minutes, rest) = value.trim().split_once(':')?;
    let minutes = minutes.parse::<u64>().ok()?;
    let (seconds, fraction) = rest.split_once(['.', ',', ':']).unwrap_or((rest, "0"));
    let seconds = seconds.parse::<u64>().ok()?;
    if seconds >= 60 {
        return None;
    }
    let fraction = fraction.chars().take(3).collect::<String>();
    let millis = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u64>().ok()? * 100,
        2 => fraction.parse::<u64>().ok()? * 10,
        _ => fraction.parse::<u64>().ok()?,
    };
    Some(
        minutes
            .saturating_mul(60_000)
            .saturating_add(seconds * 1_000)
            .saturating_add(millis),
    )
}

fn parse_media_time(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Some(ms) = value.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| n.max(0.0) as u64);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| (n.max(0.0) * 1_000.0) as u64);
    }
    let fields = value.split(':').collect::<Vec<_>>();
    match fields.as_slice() {
        [hours, minutes, seconds] => {
            let h = hours.parse::<u64>().ok()?;
            let m = minutes.parse::<u64>().ok()?;
            let s = seconds.parse::<f64>().ok()?;
            if m >= 60 || !s.is_finite() || !(0.0..60.0).contains(&s) {
                return None;
            }
            Some(
                h.saturating_mul(3_600_000)
                    .saturating_add(m.saturating_mul(60_000))
                    .saturating_add((s * 1_000.0).max(0.0) as u64),
            )
        }
        _ => parse_lrc_time(value),
    }
}

fn adjust_time(time: u64, offset: i64) -> u64 {
    if offset < 0 {
        time.saturating_sub(offset.unsigned_abs())
    } else {
        time.saturating_add(offset as u64)
    }
}

fn parse_time_tag(line: &str, open: char, close: char, from: usize) -> Option<(u64, usize)> {
    let relative_open = line[from..].find(open)?;
    let start = from + relative_open;
    let relative_close = line[start + open.len_utf8()..].find(close)?;
    let end = start + open.len_utf8() + relative_close;
    let time = parse_lrc_time(&line[start + open.len_utf8()..end])?;
    Some((time, end + close.len_utf8()))
}

fn split_background(raw: &str) -> (&str, Option<&str>) {
    let Some((lead, suffix)) = raw.rsplit_once("[bg:") else {
        return (raw, None);
    };
    let Some(end) = suffix.rfind(']') else {
        return (raw, None);
    };
    (lead.trim_end(), Some(suffix[..end].trim()))
}

fn parse_word_parts(
    raw: &str,
    line_start_ms: u64,
    is_background: bool,
) -> (String, Vec<LyricPart>) {
    let mut parts = Vec::new();
    let mut plain = String::new();
    let mut cursor = 0;
    let mut active_start = line_start_ms;
    let mut active_start_explicit = false;
    let mut segment_start = 0;

    while let Some((start_ms, after_tag)) = parse_time_tag(raw, '<', '>', cursor) {
        let tag_start = raw[cursor..after_tag]
            .find('<')
            .map(|n| cursor + n)
            .unwrap_or(cursor);
        let text = raw[segment_start..tag_start].to_string();
        if !text.is_empty() && !(parts.is_empty() && text.trim().is_empty()) {
            plain.push_str(&text);
            parts.push(LyricPart {
                start_ms: active_start,
                start_time_explicit: active_start_explicit,
                duration_ms: start_ms.saturating_sub(active_start),
                text,
                is_background,
                ..Default::default()
            });
        }
        active_start = start_ms;
        active_start_explicit = true;
        cursor = after_tag;
        segment_start = after_tag;
    }

    let remainder = &raw[segment_start..];
    if !remainder.is_empty() {
        plain.push_str(remainder);
        parts.push(LyricPart {
            start_ms: active_start,
            start_time_explicit: active_start_explicit,
            duration_ms: 0,
            text: remainder.to_string(),
            is_background,
            ..Default::default()
        });
    }
    let _ = cursor;

    if parts.is_empty() && !raw.is_empty() {
        parts.push(LyricPart {
            start_ms: line_start_ms,
            duration_ms: 0,
            text: raw.to_string(),
            is_background,
            ..Default::default()
        });
        plain.push_str(raw);
    }
    (plain.trim().to_string(), parts)
}

/// Parse LRC, enhanced LRC (`<mm:ss.xx>` word markers), and PlayerVF's
/// trailing `[bg:...]` background-vocal group.
pub fn parse_lrc(input: &str) -> ParsedLyrics {
    let mut result = ParsedLyrics {
        title: None,
        artist: None,
        album: None,
        offset_ms: 0,
        lines: Vec::new(),
        ..Default::default()
    };

    for raw_line in input.trim_start_matches('\u{feff}').lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                let tag = &line[1..end];
                let value = line[end + 1..].trim();
                let (name, meta_value) = tag.split_once(':').unwrap_or((tag, value));
                match name.to_ascii_lowercase().as_str() {
                    "ti" => {
                        result.title = Some(meta_value.to_string());
                        continue;
                    }
                    "ar" => {
                        result.artist = Some(meta_value.to_string());
                        continue;
                    }
                    "al" => {
                        result.album = Some(meta_value.to_string());
                        continue;
                    }
                    "au" | "lr" => {
                        result
                            .songwriters
                            .extend(crate::braccato::credits::split_names(meta_value));
                        continue;
                    }
                    "by" | "length" | "re" | "tool" | "ve" => {
                        continue;
                    }
                    "offset" => {
                        result.offset_ms = if meta_value.contains('.') {
                            meta_value
                                .parse::<f64>()
                                .map(|value| (value * 1_000.0) as i64)
                                .unwrap_or(0)
                        } else {
                            meta_value.parse().unwrap_or(0)
                        };
                        continue;
                    }
                    _ => {}
                }
            }
        }

        let mut positions = Vec::new();
        let mut cursor = 0;
        while line[cursor..].starts_with('[') {
            let Some((time, after)) = parse_time_tag(line, '[', ']', cursor) else {
                break;
            };
            positions.push(time);
            cursor = after;
        }
        if positions.is_empty() {
            continue;
        }
        let payload = line[cursor..].trim();
        let (lead_raw, bg_raw) = split_background(payload);
        // Most lines contain colons only inside timestamps. Avoid allocating a
        // stripped copy and running the regex for every sung line.
        if lead_raw
            .split('>')
            .any(|chunk| chunk.split('<').next().unwrap_or("").contains([':', '：']))
        {
            let clean_credit = LRC_WORD.replace_all(lead_raw, "");
            if let Some(names) = crate::braccato::credits::credit_line(&clean_credit) {
                result.songwriters.extend(names);
                continue;
            }
        }
        let original_start = positions[0];
        positions.sort_unstable();
        positions.dedup();
        for start_ms in positions {
            let (agent, lead_raw) = lead_raw
                .split_once(':')
                .filter(|(v, _)| {
                    v.trim() == "bg"
                        || v.trim()
                            .strip_prefix('v')
                            .is_some_and(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()))
                })
                .map(|(v, t)| (v.trim().to_string(), t.trim_start()))
                .unwrap_or((String::new(), lead_raw));
            let (text, mut parts) = parse_word_parts(lead_raw, start_ms, agent == "bg");
            // Repeated line timestamps repeat the word timing relative to the first line.
            let delta = start_ms.saturating_sub(original_start);
            for part in &mut parts {
                if lead_raw.contains('<') {
                    part.start_ms = part.start_ms.saturating_add(delta);
                }
            }
            if let Some(bg_raw) = bg_raw {
                let (_, mut bg_parts) = parse_word_parts(bg_raw, start_ms, true);
                if bg_raw.contains('<') {
                    for part in &mut bg_parts {
                        part.start_ms = part.start_ms.saturating_add(delta);
                    }
                }
                parts.append(&mut bg_parts);
            }
            result.lines.push(LyricLine {
                translation: String::new(),
                agent,
                start_ms,
                duration_ms: [Some(lead_raw), bg_raw]
                    .into_iter()
                    .flatten()
                    .filter_map(|raw| {
                        raw.rfind('<')
                            .and_then(|i| parse_time_tag(raw, '<', '>', i))
                            .filter(|(_, after)| raw[*after..].trim().is_empty())
                            .map(|(end, _)| end)
                    })
                    .max()
                    .map(|end| end.saturating_add(delta).saturating_sub(start_ms))
                    .unwrap_or(0),
                text,
                parts,
                ..Default::default()
            });
        }
    }

    for line in &mut result.lines {
        line.start_ms = adjust_time(line.start_ms, -result.offset_ms);
        for part in &mut line.parts {
            part.start_ms = adjust_time(part.start_ms, -result.offset_ms);
        }
    }
    result.lines.sort_by_key(|line| line.start_ms);
    let mut merged: Vec<LyricLine> = Vec::new();
    for line in std::mem::take(&mut result.lines) {
        if let Some(previous) = merged
            .last_mut()
            .filter(|p| p.start_ms == line.start_ms && p.agent.is_empty() && line.agent.is_empty())
        {
            if previous.text != line.text {
                if !previous.translation.is_empty() {
                    previous.translation.push('\n');
                }
                previous.translation.push_str(&line.text);
            }
        } else {
            merged.push(line);
        }
    }
    result.lines = merged;
    result.songwriters = crate::braccato::credits::unique_names(result.songwriters);
    for index in 0..result.lines.len() {
        if let Some(next) = result
            .lines
            .get(index + 1)
            .filter(|_| result.lines[index].duration_ms == 0)
        {
            result.lines[index].duration_ms =
                next.start_ms.saturating_sub(result.lines[index].start_ms);
        }
    }
    result
}

/// Parse QQ QRC timing (`[start,duration]word(absolute-time,duration,...)word`).
pub fn parse_qrc(input: &str) -> ParsedLyrics {
    let mut result = ParsedLyrics {
        title: None,
        artist: None,
        album: None,
        offset_ms: 0,
        lines: Vec::new(),
        ..Default::default()
    };
    let timing_pattern = &*QRC_WORD;
    let mut singers = std::collections::BTreeMap::<String, String>::new();
    let mut current_singer = String::new();
    for source in input.trim_start_matches('\u{feff}').lines() {
        let line = source.trim();
        if let Some(value) = line.strip_prefix("[ti:").and_then(|s| s.strip_suffix(']')) {
            result.title = Some(value.to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("[ar:").and_then(|s| s.strip_suffix(']')) {
            result.artist = Some(value.to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("[al:").and_then(|s| s.strip_suffix(']')) {
            result.album = Some(value.to_string());
            continue;
        }
        if !line.starts_with('[') {
            continue;
        }
        let Some(header_end) = line.find(']') else {
            continue;
        };
        let Some((start, duration)) = line[1..header_end].split_once(',') else {
            continue;
        };
        let Ok(start_ms) = start.parse::<u64>() else {
            continue;
        };
        let Ok(duration_ms) = duration.parse::<u64>() else {
            continue;
        };
        let body = &line[header_end + 1..];
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut cursor = 0;
        for captures in timing_pattern.captures_iter(body) {
            let marker = captures.get(0).unwrap();
            let word = &body[cursor..marker.start()];
            text.push_str(word);
            if !word.is_empty() {
                parts.push(LyricPart {
                    // QQ and the original Braccato parser use absolute song times.
                    start_ms: captures[1].parse().unwrap_or(start_ms),
                    start_time_explicit: true,
                    duration_ms: captures[2].parse().unwrap_or(0),
                    text: word.to_string(),
                    is_background: false,
                    ..Default::default()
                });
            }
            cursor = marker.end();
        }
        let remainder = &body[cursor..];
        if !remainder.is_empty() {
            text.push_str(remainder);
            if let Some(last) = parts.last_mut() {
                last.text.push_str(remainder);
            } else {
                parts.push(LyricPart {
                    start_ms,
                    duration_ms: 0,
                    text: remainder.to_string(),
                    is_background: false,
                    ..Default::default()
                });
            }
        }
        if let Some(names) = crate::braccato::credits::credit_line(&text) {
            result.songwriters.extend(names);
            continue;
        }
        let mut line_start = start_ms;
        let mut line_duration = duration_ms;
        if let Some((name, _)) = text
            .split_once([':', '：'])
            .filter(|(name, _)| name.chars().count() <= 20)
        {
            let bytes = name.len() + text[name.len()..].chars().next().unwrap().len_utf8();
            let name = name.trim().to_string();
            let next = singers.len() + 1;
            current_singer = singers
                .entry(name.clone())
                .or_insert_with(|| {
                    if matches!(name.to_uppercase().as_str(), "合" | "合唱" | "ALL") {
                        "v1000".into()
                    } else {
                        format!("v{next}")
                    }
                })
                .clone();
            text = text[bytes..].trim_start().into();
            let mut remaining = bytes;
            for part in &mut parts {
                if remaining == 0 {
                    break;
                }
                let take = remaining.min(part.text.len());
                part.text.drain(..take);
                remaining -= take;
            }
            parts.retain(|p| !p.text.is_empty());
            if let (Some(first), Some(last)) = (parts.first(), parts.last()) {
                line_start = first.start_ms;
                line_duration = last
                    .start_ms
                    .saturating_add(last.duration_ms)
                    .saturating_sub(line_start);
            }
        }
        if !text.trim().is_empty() {
            result.lines.push(LyricLine {
                translation: String::new(),
                agent: current_singer.clone(),
                start_ms: line_start,
                duration_ms: line_duration,
                text: text.trim().to_string(),
                parts,
                ..Default::default()
            });
        }
    }
    result.songwriters = crate::braccato::credits::unique_names(result.songwriters);
    result.lines.sort_by_key(|line| line.start_ms);
    result
}

/// Parse timed-text markup with line and word spans into the shared lyric
/// model. Unknown styling is intentionally ignored by the native renderer.
pub fn parse_ttml(input: &str) -> ParsedLyrics {
    crate::braccato::ttml::parse(input)
}

/// SubRip cues keep their explicit end times, including multiline text.
pub fn parse_srt(input: &str) -> ParsedLyrics {
    let mut result = parse_plain("");
    let normalized = input.replace("\r\n", "\n");
    let mut cue: Option<LyricLine> = None;
    for source in normalized
        .trim_start_matches('\u{feff}')
        .lines()
        .chain(std::iter::once(""))
    {
        let line = source.trim();
        if line.is_empty() {
            if let Some(mut c) = cue.take() {
                if !c.text.is_empty() {
                    c.parts.push(LyricPart {
                        start_ms: c.start_ms,
                        duration_ms: c.duration_ms,
                        text: c.text.clone(),
                        is_background: false,
                        ..Default::default()
                    });
                    result.lines.push(c);
                }
            }
        } else if let Some((start, end)) = line.split_once("-->") {
            let start = parse_media_time(&start.trim().replace(',', "."));
            let end = end
                .split_whitespace()
                .next()
                .and_then(|v| parse_media_time(&v.replace(',', ".")));
            cue = start
                .zip(end)
                .filter(|(s, e)| e > s)
                .map(|(s, e)| LyricLine {
                    start_ms: s,
                    duration_ms: e - s,
                    text: String::new(),
                    parts: vec![],
                    translation: String::new(),
                    ..Default::default()
                });
        } else if let Some(c) = &mut cue {
            if !c.text.is_empty() {
                c.text.push('\n');
            }
            // Preserve literal text; Slint escapes it at rendering time.
            let literal = SUBTITLE_TAG.replace_all(line, "");
            c.text
                .push_str(&html_escape::decode_html_entities(&literal));
        }
    }
    result.lines.sort_by_key(|l| l.start_ms);
    result
}

/// One detection and classification path for providers, paste, and migration.
/// Mirrors the original app's Braccato pipeline without a JavaScript runtime.
pub fn parse_payload(raw: &str) -> Option<(ParsedLyrics, &'static str, LyricsSyncType)> {
    parse_payload_depth(raw, 0)
}
/// Recover a complete TTML document from chat prose, stray fence/language
/// markers, or an HTML-escaped transport. Never decode entities inside XML
/// which already has real tags, or search through a JSON/SSE envelope.
pub(crate) fn recover_ttml(raw: &str) -> Option<String> {
    if raw.starts_with(['{', '[', '"'])
        || raw
            .lines()
            .any(|line| line.starts_with("data:") || line.starts_with("event:"))
    {
        return None;
    }
    let mut source = std::borrow::Cow::Borrowed(raw);
    for _ in 0..3 {
        if let Some(root) = TTML_START.find(&source) {
            let name = source[root.start() + 1..]
                .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()?;
            let closing = format!("</{name}");
            let end = source.rfind(&closing)? + closing.len();
            let tail = &source[end..];
            let bracket = tail.find('>')?;
            if !tail[..bracket].trim().is_empty() {
                return None;
            }
            let document = &source[root.start()..end + bracket + 1];
            return (document != raw).then(|| document.to_string());
        }
        let decoded = html_escape::decode_html_entities(&source);
        if decoded == source {
            break;
        }
        source = std::borrow::Cow::Owned(decoded.into_owned());
    }
    None
}
pub(crate) fn code_fence_body(raw: &str) -> Option<&str> {
    let marker = raw.chars().next()?;
    let (opening, rest) = raw.split_once('\n')?;
    let width = opening.chars().take_while(|&c| c == marker).count();
    if opening[width..].contains(marker) {
        return None;
    }
    let (body, closing) = rest.rsplit_once('\n')?;
    let closing = closing.trim();
    if closing.len() < width || !closing.chars().all(|c| c == marker) {
        return None;
    }
    Some(body)
}
fn parse_payload_depth(
    raw: &str,
    depth: usize,
) -> Option<(ParsedLyrics, &'static str, LyricsSyncType)> {
    if depth > 8 {
        return None;
    }
    let raw = raw.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if raw.is_empty() {
        return None;
    }
    // Chat/provider responses often fence the document, including JSON which
    // can itself contain fenced lyrics. Unwrap before format detection and use
    // the same depth limit as JSON envelopes. Unwrap clean fences first so
    // escaped XML inside a JSON string is not mistaken for raw XML.
    if raw.starts_with("```") || raw.starts_with("~~~") {
        if let Some(body) = code_fence_body(raw) {
            return parse_payload_depth(body, depth + 1);
        }
        return recover_ttml(raw).and_then(|document| parse_payload_depth(&document, depth + 1));
    }
    if let Some(document) = recover_ttml(raw) {
        return parse_payload_depth(&document, depth + 1);
    }
    if raw.starts_with('"') {
        let inner = serde_json::from_str::<String>(raw).ok()?;
        return parse_payload_depth(&inner, depth + 1);
    }
    if raw.starts_with('{') {
        let value: serde_json::Value = serde_json::from_str(raw).ok()?;
        for field in [
            "ttml",
            "xml",
            "wordByWord",
            "syncedLyrics",
            "lrc",
            "qrc",
            "srt",
            "vtt",
            "lyrics",
            "synced",
            "plain",
            "plainLyrics",
            "data",
            "result",
            "raw",
            "content",
            "payload",
            "lines",
            "items",
            "text",
            "lyric",
            "subtitles",
            "captions",
            "transcript",
            "results",
            "response",
            "lyricText",
            "lyricContent",
        ] {
            if let Some(inner) = value.get(field) {
                let encoded;
                let inner = if let Some(text) = inner.as_str() {
                    text
                } else if inner.is_object() || inner.is_array() {
                    encoded = inner.to_string();
                    encoded.as_str()
                } else {
                    continue;
                };
                if let Some(mut result) = parse_payload_depth(inner, depth + 1) {
                    if let Some(language) = value.get("language").and_then(|v| v.as_str()) {
                        result.0.language = Some(language.into());
                    }
                    if let Some(writers) = value.get("songwriters").and_then(|v| v.as_array()) {
                        result.0.songwriters = writers
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect();
                    }
                    return Some(result);
                }
            }
        }
        return None;
    }
    if raw.starts_with('[') {
        if let Ok(lines) = serde_json::from_str::<Vec<String>>(raw) {
            let joined = lines.join("\n");
            return parse_payload_depth(&joined, depth + 1);
        }
    }
    if raw
        .strip_prefix('[')
        .is_some_and(|s| s.trim_start().starts_with('{'))
    {
        let source: serde_json::Value = serde_json::from_str(raw).ok()?;
        let lyrics: Vec<crate::braccato::types::Lyric> =
            serde_json::from_value(source.clone()).ok()?;
        let source_lines = source.as_array()?;
        let has_start = |value: &serde_json::Value| {
            value.get("startTimeMs").is_some() || value.get("start_time_ms").is_some()
        };
        let has_explicit_timing = source_lines.iter().any(|line| {
            has_start(line)
                || line
                    .get("parts")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|parts| parts.iter().any(has_start))
        });
        let mut parsed = ParsedLyrics::default();
        for (line_index, line) in lyrics.into_iter().enumerate() {
            if line.words.is_empty() && !line.is_instrumental {
                continue;
            }
            let source_line = &source_lines[line_index];
            let convert = |p: crate::braccato::types::LyricPart| LyricPart {
                start_time_explicit: false,
                start_ms: p.start_time_ms.max(0) as u64,
                duration_ms: p.duration_ms.max(0) as u64,
                text: p.words,
                is_background: p.is_background,
                explicit: p.explicit,
            };
            let mut translations = line
                .translations
                .unwrap_or_default()
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>();
            let legacy = line.translation;
            if let Some(t) = &legacy {
                translations.entry(t.lang.clone()).or_insert(t.text.clone());
            }
            let translation = legacy
                .map(|t| t.text)
                .or_else(|| translations.values().next().cloned())
                .unwrap_or_default();
            parsed.lines.push(LyricLine {
                start_ms: line.start_time_ms.max(0) as u64,
                duration_ms: line.duration_ms.max(0) as u64,
                text: line.words,
                parts: line
                    .parts
                    .unwrap_or_default()
                    .into_iter()
                    .enumerate()
                    .map(|(part_index, part)| {
                        let start_time_explicit = source_line
                            .get("parts")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|parts| parts.get(part_index))
                            .is_some_and(has_start);
                        let mut converted = convert(part);
                        converted.start_time_explicit = start_time_explicit;
                        converted
                    })
                    .collect(),
                key: line.key.unwrap_or_default(),
                agent: line.agent.unwrap_or_default(),
                translation,
                translations,
                romanization: line.romanization.unwrap_or_default(),
                timed_romanization: line
                    .timed_romanization
                    .unwrap_or_default()
                    .into_iter()
                    .enumerate()
                    .map(|(index, part)| {
                        let mut converted = convert(part);
                        converted.start_time_explicit = source_line
                            .get("timedRomanization")
                            .and_then(serde_json::Value::as_array)
                            .and_then(|parts| parts.get(index))
                            .is_some_and(has_start);
                        converted
                    })
                    .collect(),
            });
        }
        parsed.lines.sort_by_key(|l| l.start_ms);
        if parsed.lines.is_empty() {
            return None;
        }
        let sync = if has_part_level_timing(&parsed) {
            LyricsSyncType::RichSync
        } else if has_any_timing(&parsed) || has_explicit_timing {
            LyricsSyncType::Synced
        } else {
            LyricsSyncType::Plain
        };
        return Some((parsed, "json", sync));
    }
    let qrc = &*QRC_LINE;
    let lrc = &*LRC_LINE;
    let (lyrics, format) = if raw.starts_with('<') && QRC_CONTENT.is_match(raw) {
        let capture = QRC_CONTENT.captures(raw)?;
        let inner = capture.get(1).or_else(|| capture.get(2))?.as_str();
        let inner = html_escape::decode_html_entities(inner);
        (parse_qrc(&inner), "qrc")
    } else if raw.starts_with('<') {
        // Invalid/unknown markup must not become literal XML in the lyric view.
        let xml = crate::braccato::ttml::prepare_xml(raw);
        let doc = roxmltree::Document::parse(&xml).ok()?;
        if doc.root_element().tag_name().name() != "tt" {
            return None;
        }
        (parse_ttml(raw), "ttml")
    } else if raw.starts_with("WEBVTT") {
        (parse_srt(raw), "vtt")
    } else if SRT_CUE.is_match(raw) {
        (parse_srt(raw), "srt")
    } else if qrc.is_match(raw) {
        (parse_qrc(raw), "qrc")
    } else if lrc.is_match(raw) {
        (parse_lrc(raw), "lrc")
    } else {
        (parse_plain(raw), "txt")
    };
    if lyrics.lines.is_empty() {
        return None;
    }
    let words = has_part_level_timing(&lyrics);
    let syllables = has_syllable_level_timing(&lyrics);
    let has_line_timing = has_any_timing(&lyrics)
        || match format {
            "lrc" => LRC_LINE.is_match(raw),
            "qrc" => QRC_LINE.is_match(raw),
            "ttml" => TTML_TIMING.is_match(raw),
            "srt" | "vtt" => SRT_CUE.is_match(raw),
            _ => false,
        };
    let sync = if format == "txt" || !has_line_timing {
        LyricsSyncType::Plain
    } else if syllables || words && matches!(format, "ttml" | "qrc" | "json") {
        LyricsSyncType::RichSync
    } else if words {
        LyricsSyncType::Word
    } else {
        LyricsSyncType::Synced
    };
    Some((lyrics, format, sync))
}

pub fn parse_plain(input: &str) -> ParsedLyrics {
    let lines = input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|text| LyricLine {
            translation: String::new(),
            start_ms: 0,
            duration_ms: 0,
            parts: vec![LyricPart {
                start_ms: 0,
                duration_ms: 0,
                text: text.to_string(),
                is_background: false,
                ..Default::default()
            }],
            text: text.to_string(),
            ..Default::default()
        })
        .collect();
    ParsedLyrics {
        title: None,
        artist: None,
        album: None,
        offset_ms: 0,
        lines,
        ..Default::default()
    }
}

/// Fill open-ended line cues without creating part-level timing.
pub fn complete_line_timing(lyrics: &mut ParsedLyrics, duration_ms: u64) {
    for index in 0..lyrics.lines.len() {
        let next = lyrics
            .lines
            .get(index + 1)
            .map(|l| l.start_ms)
            .unwrap_or_else(|| adjust_time(duration_ms, -lyrics.offset_ms));
        let line = &mut lyrics.lines[index];
        if line.duration_ms == 0 {
            line.duration_ms = next.saturating_sub(line.start_ms);
        }
    }
}

/// Kept as the shared timing completion entry point; only line ends are inferred.
pub fn complete_timing(lyrics: &mut ParsedLyrics, duration_ms: u64) {
    complete_line_timing(lyrics, duration_ms);
}

/// Export the current model without dropping word timing, translations or backing vocals.
pub fn serialize_lrc(lyrics: &ParsedLyrics) -> String {
    fn time(ms: u64) -> String {
        format!(
            "{:02}:{:02}.{:03}",
            ms / 60_000,
            ms / 1_000 % 60,
            ms % 1_000
        )
    }
    fn words(parts: &[&LyricPart]) -> String {
        parts
            .iter()
            .map(|p| format!("<{}>{}", time(p.start_ms), p.text))
            .collect()
    }
    let mut lines = Vec::new();
    if let Some(title) = &lyrics.title {
        lines.push(format!("[ti:{title}]"));
    }
    if let Some(artist) = &lyrics.artist {
        lines.push(format!("[ar:{artist}]"));
    }
    for line in &lyrics.lines {
        let lead = line
            .parts
            .iter()
            .filter(|p| !p.is_background)
            .collect::<Vec<_>>();
        let bg = line
            .parts
            .iter()
            .filter(|p| p.is_background)
            .collect::<Vec<_>>();
        let text = if lead.len() > 1 {
            words(&lead)
        } else {
            line.text.clone()
        };
        let suffix = if bg.is_empty() {
            String::new()
        } else {
            format!(" [bg:{}]", words(&bg))
        };
        lines.push(format!("[{}]{text}{suffix}", time(line.start_ms)));
        if !line.translation.is_empty() {
            lines.push(format!("[{}]{}", time(line.start_ms), line.translation));
        }
    }
    lines.join("\n")
}

fn add_alternative(
    alternatives: &mut Vec<LyricsAlternative>,
    provider: &str,
    _field: &str,
    raw: &str,
) {
    if let Some((lyrics, format, sync_type)) = parse_payload(raw) {
        alternatives.push(LyricsAlternative {
            provider: provider.to_ascii_lowercase(),
            sync_type,
            format: format.into(),
            lyrics,
        });
    }
}

/// Parse provider `data:` records from the Better Lyrics SSE response. Every
/// usable provider result becomes a typed alternative for the Slint UI.
pub fn parse_provider_sse(input: &str) -> Vec<LyricsAlternative> {
    let mut alternatives = Vec::new();
    let mut records = Vec::new();
    let mut multiline = String::new();
    for line in input.lines().map(str::trim) {
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if serde_json::from_str::<serde_json::Value>(data).is_ok() {
                records.push(data.to_string());
            } else {
                if !multiline.is_empty() {
                    multiline.push('\n');
                }
                multiline.push_str(data);
            }
        } else if line.is_empty() && !multiline.is_empty() {
            records.push(std::mem::take(&mut multiline));
        }
    }
    if !multiline.is_empty() {
        records.push(multiline);
    }
    for record in records {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(&record) else {
            continue;
        };
        let provider = event
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let Some(results) = event.get("results").and_then(|v| v.as_object()) else {
            continue;
        };
        let start = alternatives.len();
        for field in [
            "ttml",
            "xml",
            "wordByWord",
            "syncedLyrics",
            "lrc",
            "qrc",
            "srt",
            "vtt",
            "lyrics",
            "synced",
            "plain",
            "plainLyrics",
        ] {
            let Some(value) = results.get(field) else {
                continue;
            };
            // Providers may send a lyric document directly, or wrap it in a
            // Braccato JSON model. Preserve objects and arrays so the shared
            // payload parser can handle both shapes instead of silently
            // dropping non-string results.
            let encoded;
            let raw = if let Some(raw) = value.as_str() {
                raw
            } else if value.is_object() || value.is_array() {
                encoded = value.to_string();
                encoded.as_str()
            } else {
                continue;
            };
            add_alternative(&mut alternatives, provider, field, raw);
        }
        let translation = results
            .get("translation")
            .or_else(|| results.get("translated"))
            .and_then(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| (value.is_object() || value.is_array()).then(|| value.to_string()))
            });
        if let Some(raw) = translation {
            let mut translated = parse_payload(&raw);
            // Some providers wrap each language in its own key, for example
            // `{ "uk": "[00:01.00]...", "en": "[...]" }`.
            if translated.is_none() {
                if let Ok(languages) = serde_json::from_str::<serde_json::Value>(&raw) {
                    translated = languages.as_object().and_then(|values| {
                        values
                            .values()
                            .filter_map(serde_json::Value::as_str)
                            .find_map(parse_payload)
                    });
                }
            }
            if let Some((translated, _, translated_sync)) = translated {
                for alt in &mut alternatives[start..] {
                    let same_line_count = translated.lines.len() == alt.lyrics.lines.len();
                    for (index, line) in alt.lyrics.lines.iter_mut().enumerate() {
                        let translation_line = if translated_sync == LyricsSyncType::Plain {
                            same_line_count
                                .then(|| translated.lines.get(index))
                                .flatten()
                        } else {
                            translated
                                .lines
                                .iter()
                                .find(|l| l.start_ms == line.start_ms)
                        };
                        if let Some(translation_line) = translation_line {
                            line.translation = translation_line.text.clone();
                        }
                    }
                }
            }
        }
    }
    alternatives
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_metadata_multiple_timestamps_and_offsets() {
        let parsed =
            parse_lrc("[ti:Song]\n[ar:Artist]\n[offset:-250]\n[00:01.23][00:02.00]Hello\n");
        assert_eq!(parsed.title.as_deref(), Some("Song"));
        assert_eq!(parsed.artist.as_deref(), Some("Artist"));
        assert_eq!(parsed.offset_ms, -250);
        assert_eq!(
            parsed
                .lines
                .iter()
                .map(|line| line.start_ms)
                .collect::<Vec<_>>(),
            vec![1480, 2250]
        );
        assert_eq!(parsed.lines[0].duration_ms, 770);
    }

    #[test]
    fn parses_enhanced_lrc_word_markers() {
        let parsed = parse_lrc("[00:01.00]<00:01.00>Hello <00:01.50>world");
        let line = &parsed.lines[0];
        assert_eq!(line.text, "Hello world");
        assert_eq!(
            line.parts
                .iter()
                .map(|part| part.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Hello ", "world"]
        );
        assert_eq!(line.parts[0].start_ms, 1_000);
        assert_eq!(line.parts[1].start_ms, 1_500);
    }

    #[test]
    fn retains_background_vocal_parts_after_the_lead() {
        let parsed = parse_lrc("[00:04.00]Lead line [bg:(backing words)]");
        let line = &parsed.lines[0];
        assert_eq!(line.text, "Lead line");
        assert_eq!(line.parts.len(), 2);
        assert!(!line.parts[0].is_background);
        assert!(line.parts[1].is_background);
        assert_eq!(line.parts[1].text, "(backing words)");
    }

    #[test]
    fn malformed_timestamps_and_plain_text_are_skipped() {
        let parsed = parse_lrc("plain text\n[00:99.00]bad\n[00:01.00]good");
        assert_eq!(parsed.lines.len(), 1);
        assert_eq!(parsed.lines[0].text, "good");
    }

    #[test]
    fn parses_qrc_syllable_timing() {
        let parsed = parse_qrc("[ti:Song]\n[0,2000]Hello(0,1000)World(1000,1000)");
        assert_eq!(parsed.title.as_deref(), Some("Song"));
        assert_eq!(parsed.lines[0].text, "HelloWorld");
        assert_eq!(parsed.lines[0].parts.len(), 2);
        assert_eq!(parsed.lines[0].parts[1].start_ms, 1_000);
    }

    #[test]
    fn parses_ttml_word_timing_and_background_role() {
        let xml = r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata"><body><p begin="00:00:10.000" end="00:00:14.000"><span begin="00:00:10.000" end="00:00:12.000">Hello</span><span begin="00:00:12.000" end="00:00:14.000" ttm:role="x-bg">(yeah)</span></p></body></tt>"#;
        let parsed = parse_ttml(xml);
        assert_eq!(parsed.lines.len(), 1);
        assert_eq!(parsed.lines[0].text, "Hello");
        assert_eq!(parsed.lines[0].parts.len(), 2);
        assert!(parsed.lines[0].parts[1].is_background);
    }

    #[test]
    fn parses_provider_sse_into_timed_alternatives() {
        let sse = "event: provider\ndata: {\"provider\":\"lrclib\",\"results\":{\"synced\":\"[00:01.00]First\\n[00:02.00]Second\",\"plain\":\"First\\nSecond\"}}\n";
        let alternatives = parse_provider_sse(sse);
        assert_eq!(alternatives.len(), 2);
        assert_eq!(alternatives[0].provider, "lrclib");
        assert_eq!(alternatives[0].sync_type, LyricsSyncType::Synced);
        assert_eq!(alternatives[0].lyrics.lines[0].text, "First");
    }
}
