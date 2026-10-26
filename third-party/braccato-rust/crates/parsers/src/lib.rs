//! Parsers and format detection for common synchronized lyric formats.

use braccato_types::{Lyric, LyricPart};
use regex::Regex;
use std::sync::OnceLock;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LyricFormat {
    Ttml,
    Lrc,
    Srt,
    Qrc,
    Plain,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("malformed TTML: {0}")]
    Ttml(String),
    #[error("unsupported format")]
    Unsupported,
}

pub fn detect_format(input: &str) -> LyricFormat {
    let trimmed = input.trim_start();
    if trimmed.starts_with('<') && trimmed.to_ascii_lowercase().contains("<tt") {
        LyricFormat::Ttml
    } else if Regex::new(r"(?m)^\[\d+:\d+(?:\.\d+)?\]")
        .unwrap()
        .is_match(input)
    {
        LyricFormat::Lrc
    } else if Regex::new(r"(?m)^\d+\s*\r?\n\d{2}:\d{2}:\d{2}[,.]\d+\s*-->")
        .unwrap()
        .is_match(input)
    {
        LyricFormat::Srt
    } else if input.contains("LyricContent=")
        || input.contains("<QrcInfos>")
        || Regex::new(r"(?m)^\[\d+,\d+\]").unwrap().is_match(input)
    {
        LyricFormat::Qrc
    } else {
        LyricFormat::Plain
    }
}

pub fn parse(input: &str, duration_ms: f64) -> Result<Vec<Lyric>, ParseError> {
    match detect_format(input) {
        LyricFormat::Ttml => parse_ttml(input, duration_ms),
        LyricFormat::Lrc => Ok(parse_lrc(input, duration_ms)),
        LyricFormat::Srt => Ok(parse_srt(input)),
        LyricFormat::Qrc => Ok(parse_qrc(input, duration_ms)),
        LyricFormat::Plain => Ok(input
            .lines()
            .map(|line| Lyric {
                words: line.to_owned(),
                ..Lyric::default()
            })
            .collect()),
    }
}

fn parse_clock(value: &str) -> f64 {
    let chunks: Vec<_> = value.trim().split(':').collect();
    match chunks.as_slice() {
        [sec] => sec.parse::<f64>().unwrap_or(0.0) * 1000.0,
        [min, sec] => {
            min.parse::<f64>().unwrap_or(0.0) * 60_000.0
                + sec.parse::<f64>().unwrap_or(0.0) * 1000.0
        }
        [hour, min, sec] => {
            hour.parse::<f64>().unwrap_or(0.0) * 3_600_000.0
                + min.parse::<f64>().unwrap_or(0.0) * 60_000.0
                + sec.parse::<f64>().unwrap_or(0.0) * 1000.0
        }
        _ => 0.0,
    }
}

pub fn parse_lrc(input: &str, duration_ms: f64) -> Vec<Lyric> {
    static LINE: OnceLock<Regex> = OnceLock::new();
    static WORD: OnceLock<Regex> = OnceLock::new();
    static OFFSET: OnceLock<Regex> = OnceLock::new();
    static BG: OnceLock<Regex> = OnceLock::new();
    let line_re = LINE.get_or_init(|| Regex::new(r"\[(\d+:\d+\.\d+)\]").unwrap());
    let word_re = WORD.get_or_init(|| Regex::new(r"<(\d+:\d+\.\d+)>").unwrap());
    let offset_re =
        OFFSET.get_or_init(|| Regex::new(r"(?i)^\[offset:([+-]?(?:\d+\.?\d*|\.\d+))\]$").unwrap());
    let bg_re = BG.get_or_init(|| Regex::new(r"(?s)\[bg:(.*)\]\s*$").unwrap());
    let mut offset_ms = 0.0;
    let mut entries: Vec<(Lyric, Option<f64>)> = Vec::new();
    for raw in input.lines() {
        let raw = raw.trim();
        if let Some(captures) = offset_re.captures(raw) {
            offset_ms = captures[1].parse::<f64>().unwrap_or(0.0) * 1000.0;
            continue;
        }
        let starts = line_re
            .captures_iter(raw)
            .filter_map(|capture| capture.get(1).map(|m| parse_clock(m.as_str())))
            .collect::<Vec<_>>();
        if starts.is_empty() {
            continue;
        }
        let start = starts.iter().copied().fold(f64::INFINITY, f64::min);
        let stated_end = starts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let body = line_re.replace_all(raw, "").trim().to_owned();
        let background = bg_re.captures(&body);
        let lead = background.as_ref().map_or(body.as_str(), |capture| {
            &body[..capture.get(0).unwrap().start()]
        });
        if is_credit_line(lead) {
            continue;
        }
        let bg = background.as_ref().map(|capture| capture[1].to_owned());
        let mut parts = Vec::new();
        let mut words = String::new();
        let mut closed_at = None;
        parse_lrc_segment(lead, false, word_re, &mut parts, &mut words, &mut closed_at);
        if let Some(bg) = bg {
            parse_lrc_segment(&bg, true, word_re, &mut parts, &mut words, &mut closed_at);
        }
        if parts
            .last()
            .is_some_and(|part| part.words.trim().is_empty())
        {
            let closing = parts.pop().unwrap();
            closed_at =
                Some(closed_at.map_or(closing.start_time_ms, |at| at.max(closing.start_time_ms)));
        }
        let words = words.trim().to_owned();
        entries.push((
            Lyric {
                start_time_ms: start,
                words: if words
                    .chars()
                    .all(|c| c.is_whitespace() || "♪♫♬♩🎵🎶".contains(c))
                {
                    String::new()
                } else {
                    words
                },
                duration_ms: (stated_end - start).max(0.0),
                parts: (!parts.is_empty()).then_some(parts),
                ..Lyric::default()
            },
            closed_at.or((starts.len() > 1).then_some(stated_end)),
        ));
    }
    entries.sort_by(|a, b| a.0.start_time_ms.total_cmp(&b.0.start_time_ms));
    for i in 0..entries.len() {
        let next_start = entries.get(i + 1).map(|item| item.0.start_time_ms);
        let stated_end = entries[i].1;
        let lyric = &mut entries[i].0;
        if let Some(end) = stated_end {
            lyric.duration_ms = (end - lyric.start_time_ms).max(0.0);
        } else if let Some(next) = next_start {
            if lyric.duration_ms == 0.0 {
                lyric.duration_ms = (next - lyric.start_time_ms).max(0.0);
            }
        } else if lyric.duration_ms == 0.0 && duration_ms > lyric.start_time_ms {
            lyric.duration_ms = duration_ms - lyric.start_time_ms;
        }
        if let Some(parts) = lyric.parts.as_mut() {
            for p in 0..parts.len() {
                let end = parts
                    .get(p + 1)
                    .map(|part| part.start_time_ms)
                    .or(stated_end)
                    .or(next_start)
                    .unwrap_or(lyric.start_time_ms + lyric.duration_ms);
                if parts[p].duration_ms == 0.0 {
                    parts[p].duration_ms = (end - parts[p].start_time_ms).max(0.0);
                }
            }
        }
    }
    let sung = entries
        .into_iter()
        .map(|(mut lyric, _)| {
            lyric.start_time_ms -= offset_ms;
            if let Some(parts) = lyric.parts.as_mut() {
                for part in parts {
                    part.start_time_ms -= offset_ms;
                }
            }
            lyric
        })
        .filter(|lyric| !lyric.words.is_empty())
        .collect::<Vec<_>>();
    insert_instrumental_breaks(sung, duration_ms, 5000.0)
}

pub fn insert_instrumental_breaks(
    lyrics: Vec<Lyric>,
    song_duration_ms: f64,
    gap_threshold_ms: f64,
) -> Vec<Lyric> {
    if lyrics.is_empty() {
        return lyrics;
    }
    let instrumental = |start_time_ms, duration_ms| Lyric {
        start_time_ms,
        duration_ms,
        words: String::new(),
        parts: Some(Vec::new()),
        is_instrumental: Some(true),
        ..Lyric::default()
    };
    let mut result = Vec::new();
    if lyrics[0].start_time_ms > gap_threshold_ms {
        result.push(instrumental(0.0, lyrics[0].start_time_ms));
    }
    for (index, lyric) in lyrics.iter().enumerate() {
        result.push(lyric.clone());
        if let Some(next) = lyrics.get(index + 1) {
            let end = lyric.start_time_ms + lyric.duration_ms;
            let gap = next.start_time_ms - end;
            if gap > gap_threshold_ms {
                result.push(instrumental(end, gap));
            }
        }
    }
    let last = lyrics.last().unwrap();
    let end = last.start_time_ms + last.duration_ms;
    let outro = song_duration_ms - end;
    if outro > gap_threshold_ms {
        result.push(instrumental(end, outro));
    }
    result
}

fn parse_lrc_segment(
    source: &str,
    background: bool,
    word_re: &Regex,
    parts: &mut Vec<LyricPart>,
    full_text: &mut String,
    closed_at: &mut Option<f64>,
) {
    let mut cursor = 0;
    let mut previous: Option<usize> = None;
    for capture in word_re.captures_iter(source) {
        let whole = capture.get(0).unwrap();
        let fragment = &source[cursor..whole.start()];
        full_text.push_str(fragment);
        if let Some(index) = previous
            && let Some(part) = parts.get_mut(index)
        {
            part.words.push_str(fragment);
        }
        let time = parse_clock(&capture[1]);
        if let Some(index) = previous {
            let part: &mut LyricPart = &mut parts[index];
            part.duration_ms = (time - part.start_time_ms).max(0.0);
            if part.words.trim().is_empty() {
                *closed_at = Some(time);
            }
        }
        parts.push(LyricPart {
            start_time_ms: time,
            words: String::new(),
            duration_ms: 0.0,
            is_background: Some(background),
            ..LyricPart::default()
        });
        previous = Some(parts.len() - 1);
        cursor = whole.end();
    }
    let tail = &source[cursor..];
    full_text.push_str(tail);
    if let Some(index) = previous {
        if let Some(part) = parts.get_mut(index) {
            part.words.push_str(tail);
        }
        if parts[index].words.trim().is_empty() {
            let closing = parts.pop().unwrap();
            *closed_at =
                Some(closed_at.map_or(closing.start_time_ms, |at| at.max(closing.start_time_ms)));
        }
    }
}

/// Extract songwriter names from LRC author/lyricist tags and recognized inline credit lines.
pub fn lrc_songwriters(input: &str) -> Vec<String> {
    let mut names = Vec::new();
    let timestamp = Regex::new(r"\[\d+:\d+\.\d+\]").unwrap();
    let background = Regex::new(r"(?s)\[bg:.*\]\s*$").unwrap();
    for raw in input.lines() {
        let line = raw.trim();
        if let Some((tag, value)) = line
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .and_then(|s| s.split_once(':'))
            && matches!(tag.to_ascii_lowercase().as_str(), "au" | "lr")
        {
            names.extend(split_credit_names(value));
        }
        let text = timestamp.replace_all(line, "");
        let text = background.replace(&text, "");
        if let Some((role, value)) = text.trim().split_once([':', '：'])
            && is_songwriter_role(role)
        {
            names.extend(split_credit_names(value));
        }
    }
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty() && seen.insert(name.clone()))
        .collect()
}

fn split_credit_names(value: &str) -> Vec<String> {
    value
        .split(['/', '、', '，', ','])
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn is_credit_line(value: &str) -> bool {
    let Some((role, names)) = value.trim().split_once([':', '：']) else {
        return false;
    };
    !names.trim().is_empty() && is_credit_role(role)
}

fn is_songwriter_role(role: &str) -> bool {
    let role = role.to_ascii_lowercase().replace(char::is_whitespace, "");
    matches!(
        role.as_str(),
        "au" | "lr"
            | "词"
            | "詞"
            | "作词"
            | "作詞"
            | "曲"
            | "作曲"
            | "词曲"
            | "詞曲"
            | "lyricsby"
            | "writtenby"
            | "composedby"
            | "lyricist"
            | "composer"
    ) || role
        .split(['/', '&', '、', ',', '，', '・', '·'])
        .any(|part| {
            matches!(
                part,
                "词" | "詞" | "作词" | "作詞" | "曲" | "作曲" | "词曲" | "詞曲"
            )
        })
}

fn is_credit_role(role: &str) -> bool {
    is_songwriter_role(role)
        || matches!(
            role.trim().to_ascii_lowercase().as_str(),
            "编曲"
                | "編曲"
                | "和声"
                | "和聲"
                | "混音"
                | "演唱"
                | "原唱"
                | "翻唱"
                | "produced by"
                | "arranged by"
                | "producer"
                | "arranger"
                | "vocals"
                | "guitar"
                | "bass"
                | "drums"
        )
}

fn parse_srt(input: &str) -> Vec<Lyric> {
    let re =
        Regex::new(r"(?m)(\d{2}:\d{2}:\d{2}[,.]\d+)\s*-->\s*(\d{2}:\d{2}:\d{2}[,.]\d+)").unwrap();
    input
        .split("\n\n")
        .filter_map(|block| {
            let cap = re.captures(block)?;
            let to_ms = |v: &str| {
                let norm = v.replace(',', ".");
                let mut n = parse_clock(&norm);
                if norm.split('.').nth(1).is_none_or(|f| f.len() < 3) {
                    n = (n / 10.0).round() * 10.0;
                }
                n
            };
            let start = to_ms(cap.get(1)?.as_str());
            let end = to_ms(cap.get(2)?.as_str());
            let text = block[cap.get(0)?.end()..]
                .lines()
                .map(|l| {
                    Regex::new(r"<[^>]+>")
                        .unwrap()
                        .replace_all(l, "")
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_owned();
            (!text.is_empty()).then_some(Lyric {
                start_time_ms: start,
                words: text,
                duration_ms: end - start,
                ..Lyric::default()
            })
        })
        .collect()
}

fn parse_qrc(input: &str, duration_ms: f64) -> Vec<Lyric> {
    static LINE: OnceLock<Regex> = OnceLock::new();
    static WORD: OnceLock<Regex> = OnceLock::new();
    let line_re = LINE.get_or_init(|| Regex::new(r"^\[(\d+),(\d+)\](.*)$").unwrap());
    let word_re = WORD.get_or_init(|| Regex::new(r"\((\d+),(\d+)\)").unwrap());
    let unwrapped = unwrap_qrc(input);
    let lyrics = unwrapped
        .lines()
        .filter_map(|line| {
            let c = line_re.captures(line.trim())?;
            let start = c[1].parse::<f64>().ok()?;
            let dur = c[2].parse::<f64>().ok()?;
            let body = &c[3];
            let mut cursor = 0;
            let mut parts = Vec::new();
            let mut text = String::new();
            for m in word_re.captures_iter(body) {
                let whole = m.get(0).unwrap();
                let words = &body[cursor..whole.start()];
                let at = m[1].parse::<f64>().unwrap_or(start);
                let d = m[2].parse::<f64>().unwrap_or(0.0);
                text.push_str(words);
                parts.push(LyricPart {
                    start_time_ms: at,
                    words: words.to_owned(),
                    duration_ms: d,
                    ..LyricPart::default()
                });
                cursor = whole.end();
            }
            let tail = &body[cursor..];
            text.push_str(tail);
            if let Some(last) = parts.last_mut() {
                last.words.push_str(tail);
                if last.duration_ms == 0.0 {
                    last.duration_ms = (duration_ms - last.start_time_ms).max(0.0);
                }
            }
            Some(Lyric {
                start_time_ms: start,
                words: text,
                duration_ms: if dur > 0.0 {
                    dur
                } else {
                    (duration_ms - start).max(0.0)
                },
                parts: (!parts.is_empty()).then_some(parts),
                ..Lyric::default()
            })
        })
        .collect::<Vec<_>>();
    insert_instrumental_breaks(lyrics, duration_ms, 5000.0)
}

fn unwrap_qrc(input: &str) -> String {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_str(input);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                for attribute in element.attributes().flatten() {
                    if attribute.key.local_name().as_ref() == b"LyricContent"
                        && let Ok(value) = attribute.decode_and_unescape_value(reader.decoder())
                    {
                        return value.into_owned();
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    input.to_owned()
}

fn parse_ttml(input: &str, duration_ms: f64) -> Result<Vec<Lyric>, ParseError> {
    use quick_xml::{Reader, escape::unescape, events::Event};
    use std::collections::HashMap;
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut lyrics = Vec::new();
    #[derive(Default)]
    struct Paragraph {
        start: f64,
        end: Option<f64>,
        text: String,
        parts: Vec<LyricPart>,
        agent: Option<String>,
        key: Option<String>,
    }
    #[derive(Clone, Default)]
    struct Span {
        start: Option<f64>,
        end: Option<f64>,
        background: bool,
        explicit: bool,
        part_index: Option<usize>,
    }
    let mut para: Option<Paragraph> = None;
    let mut spans: Vec<Span> = Vec::new();
    #[derive(Clone)]
    struct MetadataText {
        kind: String,
        key: String,
        language: String,
        element: String,
        depth: usize,
        text: String,
        parts: Vec<LyricPart>,
    }
    let mut metadata_mode: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    let mut metadata_text: Option<MetadataText> = None;
    let mut keyed_lines: HashMap<String, Vec<usize>> = HashMap::new();
    let mut translations: Vec<(String, String, String)> = Vec::new();
    let mut romanizations: Vec<(String, String, Vec<LyricPart>)> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.local_name();
                let tag = String::from_utf8_lossy(name.as_ref()).into_owned();
                let mut attributes = HashMap::new();
                for attribute in e.attributes().flatten() {
                    attributes.insert(
                        String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned(),
                        attribute
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(|error| ParseError::Ttml(error.to_string()))?
                            .into_owned(),
                    );
                }
                if name.as_ref() == b"p" {
                    let mut p = Paragraph::default();
                    let mut dur = None;
                    if let Some(value) = attributes.get("begin") {
                        p.start = parse_ttml_time(value);
                    }
                    if let Some(value) = attributes.get("end") {
                        p.end = Some(parse_ttml_time(value));
                    }
                    if let Some(value) = attributes.get("dur") {
                        dur = Some(parse_ttml_time(value));
                    }
                    p.agent = attributes.get("agent").cloned();
                    p.key = attributes
                        .get("key")
                        .or_else(|| attributes.get("id"))
                        .cloned();
                    if p.end.is_none() {
                        p.end = dur.map(|d| p.start + d);
                    }
                    para = Some(p);
                } else if name.as_ref() == b"span" {
                    let mut span = Span::default();
                    if let Some(value) = attributes.get("begin") {
                        span.start = Some(parse_ttml_time(value));
                    }
                    if let Some(value) = attributes.get("end") {
                        span.end = Some(parse_ttml_time(value));
                    }
                    span.background = attributes.get("role").is_some_and(|value| value == "x-bg");
                    span.explicit = attributes
                        .get("explicit")
                        .or_else(|| attributes.get("obscene"))
                        .is_some_and(|value| value == "true");
                    if spans.last().is_some_and(|s| s.background) {
                        span.background = true;
                    }
                    spans.push(span);
                }
                if matches!(tag.as_str(), "translations" | "translation") {
                    let parent = metadata_mode.last();
                    metadata_mode.push((
                        "translation".into(),
                        attributes
                            .get("lang")
                            .cloned()
                            .or_else(|| parent.and_then(|mode| mode.1.clone())),
                        attributes
                            .get("for")
                            .cloned()
                            .or_else(|| parent.and_then(|mode| mode.2.clone())),
                    ));
                }
                if matches!(tag.as_str(), "transliterations" | "transliteration") {
                    let parent = metadata_mode.last();
                    metadata_mode.push((
                        "romanization".into(),
                        attributes
                            .get("lang")
                            .cloned()
                            .or_else(|| parent.and_then(|mode| mode.1.clone())),
                        attributes
                            .get("for")
                            .cloned()
                            .or_else(|| parent.and_then(|mode| mode.2.clone())),
                    ));
                }
                if metadata_text.is_some() {
                    if let Some(active) = &mut metadata_text {
                        active.depth += 1;
                    }
                } else if matches!(tag.as_str(), "translation" | "transliteration" | "text") {
                    let context = metadata_mode.last();
                    if let Some(key) = attributes
                        .get("for")
                        .or_else(|| context.and_then(|mode| mode.2.as_ref()))
                    {
                        let kind = context.map(|mode| mode.0.clone()).unwrap_or_else(|| {
                            if tag == "transliteration" {
                                "romanization".into()
                            } else {
                                "translation".into()
                            }
                        });
                        metadata_text = Some(MetadataText {
                            kind,
                            key: key.clone(),
                            language: attributes
                                .get("lang")
                                .cloned()
                                .or_else(|| context.and_then(|mode| mode.1.clone()))
                                .unwrap_or_default(),
                            element: tag,
                            depth: 1,
                            text: String::new(),
                            parts: Vec::new(),
                        });
                    }
                }
            }
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"p" => {
                let mut start = 0.0;
                let mut end = duration_ms;
                let mut text = String::new();
                for a in e.attributes().flatten() {
                    let key = a.key.local_name();
                    let value = String::from_utf8_lossy(&a.value);
                    match key.as_ref() {
                        b"begin" => start = parse_ttml_time(&value),
                        b"end" => end = parse_ttml_time(&value),
                        b"text" => text = value.into_owned(),
                        _ => {}
                    }
                }
                if !text.trim().is_empty() {
                    lyrics.push(Lyric {
                        start_time_ms: start,
                        words: text.trim().to_owned(),
                        duration_ms: (end - start).max(0.0),
                        ..Lyric::default()
                    });
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(metadata) = &mut metadata_text {
                    let decoded = e.decode().unwrap_or_default();
                    let value = unescape(&decoded)
                        .map(|value| value.into_owned())
                        .unwrap_or_else(|_| decoded.into_owned());
                    metadata.text.push_str(&value);
                    if let Some(span) = spans
                        .iter()
                        .rev()
                        .find(|span| span.start.is_some() || span.end.is_some())
                    {
                        let start = span.start.unwrap_or_default();
                        let end = span.end.unwrap_or(start);
                        let part_index = metadata
                            .parts
                            .iter()
                            .position(|part| part.start_time_ms == start);
                        if let Some(index) = part_index {
                            metadata.parts[index].words.push_str(&value);
                        } else {
                            metadata.parts.push(LyricPart {
                                start_time_ms: start,
                                words: value.clone(),
                                duration_ms: (end - start).max(0.0),
                                is_background: Some(span.background),
                                explicit: Some(span.explicit),
                            });
                        }
                    }
                }
                if let Some(p) = &mut para {
                    let decoded = e.decode().unwrap_or_default();
                    let value = unescape(&decoded)
                        .map(|value| value.into_owned())
                        .unwrap_or_else(|_| decoded.into_owned());
                    p.text.push_str(&value);
                    if let Some(span_index) = spans
                        .iter()
                        .rposition(|s| s.start.is_some() || s.end.is_some())
                    {
                        let info = &mut spans[span_index];
                        let start = info.start.unwrap_or(p.start);
                        let end = info.end.unwrap_or(start);
                        let part_index = *info.part_index.get_or_insert_with(|| {
                            p.parts.push(LyricPart {
                                start_time_ms: start,
                                words: String::new(),
                                duration_ms: (end - start).max(0.0),
                                is_background: Some(info.background),
                                explicit: Some(info.explicit),
                            });
                            p.parts.len() - 1
                        });
                        p.parts[part_index].words.push_str(&value);
                    } else if let Some(part) = p.parts.last_mut() {
                        part.words.push_str(&value);
                    }
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(p) = &mut para {
                    let value = String::from_utf8_lossy(&e).into_owned();
                    p.text.push_str(&value);
                    if let Some(part) = p.parts.last_mut() {
                        part.words.push_str(&value);
                    }
                }
            }
            Ok(Event::GeneralRef(e)) => {
                let decoded = e.decode().unwrap_or_default();
                let entity = format!("&{decoded};");
                let value = unescape(&entity)
                    .map(|value| value.into_owned())
                    .or_else(|_| {
                        e.resolve_char_ref()
                            .map(|value| value.map_or(String::new(), |c| c.to_string()))
                    })
                    .unwrap_or_default();
                if let Some(metadata) = &mut metadata_text {
                    metadata.text.push_str(&value);
                    if let Some(span) = spans
                        .iter()
                        .rev()
                        .find(|span| span.start.is_some() || span.end.is_some())
                    {
                        let start = span.start.unwrap_or_default();
                        let end = span.end.unwrap_or(start);
                        if let Some(part) = metadata
                            .parts
                            .iter_mut()
                            .find(|part| part.start_time_ms == start)
                        {
                            part.words.push_str(&value);
                        } else {
                            metadata.parts.push(LyricPart {
                                start_time_ms: start,
                                words: value.clone(),
                                duration_ms: (end - start).max(0.0),
                                is_background: Some(span.background),
                                explicit: Some(span.explicit),
                            });
                        }
                    }
                }
                if let Some(p) = &mut para {
                    p.text.push_str(&value);
                    if let Some(span_index) = spans
                        .iter()
                        .rposition(|span| span.start.is_some() || span.end.is_some())
                    {
                        if let Some(part_index) = spans[span_index].part_index {
                            p.parts[part_index].words.push_str(&value);
                        }
                    } else if let Some(part) = p.parts.last_mut() {
                        part.words.push_str(&value);
                    }
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"span" => {
                spans.pop();
            }
            Ok(Event::End(e))
                if e.local_name().as_ref() == b"translation"
                    || e.local_name().as_ref() == b"transliteration"
                    || e.local_name().as_ref() == b"translations"
                    || e.local_name().as_ref() == b"transliterations" =>
            {
                if let Some(metadata) = metadata_text.as_mut() {
                    metadata.depth = metadata.depth.saturating_sub(1);
                }
                let closing = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                let should_pop = matches!(
                    closing.as_str(),
                    "translation" | "transliteration" | "translations" | "transliterations"
                );
                if should_pop
                    && let Some((kind, _, _)) = metadata_mode.pop()
                    && let Some(metadata) = metadata_text.take()
                    && !metadata.text.trim().is_empty()
                {
                    if kind == "romanization" {
                        romanizations.push((
                            metadata.key,
                            metadata.text.trim().to_owned(),
                            metadata.parts,
                        ));
                    } else {
                        translations.push((
                            metadata.key,
                            metadata.language,
                            metadata.text.trim().to_owned(),
                        ));
                    }
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"text" => {
                if metadata_text
                    .as_ref()
                    .is_some_and(|metadata| metadata.element == "text")
                {
                    if let Some(metadata) = metadata_text.take()
                        && !metadata.text.trim().is_empty()
                    {
                        if metadata.kind == "romanization" {
                            romanizations.push((
                                metadata.key,
                                metadata.text.trim().to_owned(),
                                metadata.parts,
                            ));
                        } else {
                            translations.push((
                                metadata.key,
                                metadata.language,
                                metadata.text.trim().to_owned(),
                            ));
                        }
                    }
                } else if let Some(metadata) = metadata_text.as_mut() {
                    metadata.depth = metadata.depth.saturating_sub(1);
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"p" => {
                if let Some(p) = para.take() {
                    let words = p.text.trim().to_owned();
                    if !words.is_empty() {
                        let stop = p.end.unwrap_or(duration_ms);
                        let line_index = lyrics.len();
                        if let Some(key) = &p.key {
                            keyed_lines.entry(key.clone()).or_default().push(line_index);
                        }
                        lyrics.push(Lyric {
                            start_time_ms: p.start,
                            words,
                            duration_ms: (stop - p.start).max(0.0),
                            parts: (!p.parts.is_empty()).then_some(p.parts),
                            agent: p.agent,
                            key: p.key,
                            ..Lyric::default()
                        });
                    }
                }
            }
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"span" => {
                let _ = e;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(ParseError::Ttml(err.to_string())),
            _ => {}
        }
    }
    for (key, language, text) in translations {
        if let Some(indices) = keyed_lines.get(&key) {
            for index in indices {
                if let Some(line) = lyrics.get_mut(*index)
                    && !language.is_empty()
                {
                    line.translations
                        .get_or_insert_with(std::collections::BTreeMap::new)
                        .insert(language.clone(), text.clone());
                    if line.translation.is_none() {
                        line.translation = Some(braccato_types::Translation {
                            text: text.clone(),
                            lang: language.clone(),
                        });
                    }
                }
            }
        }
    }
    for (key, text, parts) in romanizations {
        if let Some(indices) = keyed_lines.get(&key) {
            for index in indices {
                if let Some(line) = lyrics.get_mut(*index) {
                    line.romanization = Some(text.clone());
                    if !parts.is_empty() {
                        line.timed_romanization = Some(parts.clone());
                    }
                }
            }
        }
    }
    Ok(insert_instrumental_breaks(lyrics, duration_ms, 5000.0))
}

pub fn parse_ttml_time(value: &str) -> f64 {
    let s = value.trim();
    for (suffix, mult) in [
        ("ms", 1.0),
        ("h", 3_600_000.0),
        ("m", 60_000.0),
        ("s", 1_000.0),
    ] {
        if let Some(v) = s.strip_suffix(suffix) {
            return v.parse::<f64>().unwrap_or(0.0) * mult;
        }
    }
    parse_clock(s)
}

/// Read songwriter values from TTML `<songwriters>` metadata and composer `<meta>` entries.
pub fn ttml_songwriters(source: &str) -> Vec<String> {
    use quick_xml::{Reader, escape::unescape, events::Event};
    let mut reader = Reader::from_str(source);
    let mut names = Vec::new();
    let mut in_songwriter = false;
    let mut current = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                let tag = element.local_name();
                if tag.as_ref() == b"songwriter" {
                    in_songwriter = true;
                    current.clear();
                }
                if tag.as_ref() == b"meta" {
                    let mut key = String::new();
                    let mut value = String::new();
                    for attribute in element.attributes().flatten() {
                        let attribute_key = attribute.key.local_name();
                        let attribute_value = attribute
                            .decode_and_unescape_value(reader.decoder())
                            .unwrap_or_default();
                        match attribute_key.as_ref() {
                            b"key" => key = attribute_value.into_owned(),
                            b"value" => value = attribute_value.into_owned(),
                            _ => {}
                        }
                    }
                    if matches!(key.as_str(), "songwriter" | "songwriters") && !value.is_empty() {
                        names.push(value);
                    }
                }
            }
            Ok(Event::Text(text)) if in_songwriter => {
                let decoded = text.decode().unwrap_or_default();
                let value = unescape(&decoded)
                    .map(|value| value.into_owned())
                    .unwrap_or_else(|_| decoded.into_owned());
                current.push_str(&value);
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == b"songwriter" => {
                in_songwriter = false;
                if !current.trim().is_empty() {
                    names.push(current.trim().to_owned());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty() && seen.insert(name.clone()))
        .collect()
}

/// Extract songwriter credits from QRC timed lines.
pub fn qrc_songwriters(source: &str) -> Vec<String> {
    let line_re = Regex::new(r"^\[\d+,\d+\](.*)$").unwrap();
    let word_re = Regex::new(r"\(\d+,\d+\)").unwrap();
    let mut names = Vec::new();
    let unwrapped = unwrap_qrc(source);
    for line in unwrapped.lines() {
        if let Some(capture) = line_re.captures(line.trim()) {
            let text = word_re.replace_all(&capture[1], "");
            if let Some((role, value)) = text.trim().split_once([':', '：'])
                && is_songwriter_role(role)
            {
                names.extend(split_credit_names(value));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_formats() {
        assert_eq!(detect_format("[00:01.20]Hi"), LyricFormat::Lrc);
        assert_eq!(
            detect_format("1\n00:00:01,000 --> 00:00:02,000\nHi"),
            LyricFormat::Srt
        );
        assert_eq!(detect_format("<tt><body/></tt>"), LyricFormat::Ttml);
        assert_eq!(detect_format("[100,200]Hi(100,200)"), LyricFormat::Qrc);
    }
    #[test]
    fn parses_enhanced_lrc_words_and_timings() {
        let lyrics = parse_lrc("[00:01.00]<00:01.00>Hello <00:01.50>world", 3000.0);
        assert_eq!(lyrics[0].words, "Hello world");
        let p = lyrics[0].parts.as_ref().unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].duration_ms, 500.0);
    }
    #[test]
    fn lrc_offset_background_closings_and_instrumental_gaps_are_preserved() {
        let source = "[au:Writer One/Writer Two]\n[offset:1]\n[00:07.00]<00:07.00>Lead <00:09.00> [bg:<00:08.00>(echo)<00:09.50>]\n[00:18.00]Next";
        let lyrics = parse_lrc(source, 25_000.0);
        assert_eq!(lrc_songwriters(source), vec!["Writer One", "Writer Two"]);
        assert!(lyrics[0].is_instrumental());
        let lead = lyrics
            .iter()
            .find(|line| line.words.contains("Lead"))
            .unwrap();
        assert_eq!(lead.start_time_ms, 6_000.0);
        assert_eq!(lead.duration_ms, 2_500.0);
        assert_eq!(lead.parts.as_ref().unwrap()[1].is_background, Some(true));
        assert!(
            lyrics
                .iter()
                .any(|line| line.is_instrumental() && line.duration_ms > 5_000.0)
        );
    }
    #[test]
    fn credit_metadata_is_extracted_without_rendering_credit_lines() {
        let lrc = "[00:00.00]作词：Writer A/Writer B\n[au:Writer A]\n[00:02.00]The lyric";
        let parsed = parse_lrc(lrc, 5000.0);
        assert_eq!(
            parsed.iter().filter(|line| !line.is_instrumental()).count(),
            1
        );
        assert_eq!(lrc_songwriters(lrc), vec!["Writer A", "Writer B"]);

        let ttml = r#"<tt><head><metadata><songwriters><songwriter>Writer A</songwriter><songwriter>Writer B</songwriter></songwriters></metadata></head></tt>"#;
        assert_eq!(ttml_songwriters(ttml), vec!["Writer A", "Writer B"]);
        assert_eq!(
            qrc_songwriters("[100,300]作曲(100,300)：Writer A"),
            vec!["Writer A"]
        );
    }
    #[test]
    fn parses_srt_cues() {
        let lyrics = parse("1\n00:00:01,000 --> 00:00:03,250\nHello <i>world</i>", 0.0).unwrap();
        assert_eq!(lyrics[0].words, "Hello world");
        assert_eq!(lyrics[0].duration_ms, 2250.0);
    }
    #[test]
    fn parses_ttml_basic_paragraphs() {
        let lyrics = parse(
            "<tt><body><p begin=\"1s\" end=\"2s\">Hello</p></body></tt>",
            0.0,
        )
        .unwrap();
        assert_eq!(lyrics[0].start_time_ms, 1000.0);
        assert_eq!(lyrics[0].duration_ms, 1000.0);
    }

    #[test]
    fn ttml_keeps_word_timing_agent_and_background_vocals() {
        let source = r#"<tt xmlns:ttm="urn:test"><body><p begin="1s" end="3s" ttm:agent="v1" xml:id="line-a"><span begin="1s" end="1.5s">Lead</span><span ttm:role="x-bg"><span begin="1.5s" end="2s">Echo</span></span></p></body></tt>"#;
        let lyrics = parse(source, 0.0).unwrap();
        let line = &lyrics[0];
        assert_eq!(line.agent.as_deref(), Some("v1"));
        assert_eq!(line.key.as_deref(), Some("line-a"));
        let parts = line.parts.as_ref().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].words, "Lead");
        assert_eq!(parts[0].duration_ms, 500.0);
        assert_eq!(parts[1].words, "Echo");
        assert_eq!(parts[1].is_background, Some(true));
    }

    #[test]
    fn parses_qrc_xml_envelope() {
        let lines = parse(
            "<QrcInfos LyricContent=\"[100,300]hello(100,300)\" />",
            1000.0,
        )
        .unwrap();
        assert_eq!(lines[0].words, "hello");
        assert_eq!(lines[0].parts.as_ref().unwrap()[0].start_time_ms, 100.0);
    }

    #[test]
    fn parses_qrc_xml_entities_and_ttml_metadata_dialects() {
        let qrc = r#"<QrcInfos LyricContent="[100,300]hello &amp; goodbye(100,300)" />"#;
        assert_eq!(parse(qrc, 1000.0).unwrap()[0].words, "hello & goodbye");

        let ttml = r#"<tt><head><metadata><translations lang="fr"><translation for="chorus"><text>Bonjour &amp; merci</text></translation></translations><transliterations><transliteration for="chorus"><text><span begin="1s" end="2s">bon-jour</span></text></transliteration></transliterations></metadata></head><body><div><p begin="1s" end="3s" key="chorus">Hello</p><p begin="4s" end="5s" key="chorus">Again</p></div></body></tt>"#;
        let lines = parse(ttml, 0.0).unwrap();
        let matching = lines
            .iter()
            .filter(|line| line.key.as_deref() == Some("chorus"))
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 2);
        for line in matching {
            assert_eq!(line.translations.as_ref().unwrap()["fr"], "Bonjour & merci");
            assert_eq!(line.translation.as_ref().unwrap().lang, "fr");
            assert_eq!(line.romanization.as_deref(), Some("bon-jour"));
            assert_eq!(
                line.timed_romanization.as_ref().unwrap()[0].duration_ms,
                1000.0
            );
        }
    }
    #[test]
    fn plain_keeps_lines() {
        let l = parse("one\ntwo", 0.0).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[1].words, "two");
    }
}
