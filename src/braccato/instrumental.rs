//! Instrumental gap normalization from @braccato/parsers, with overlap-safe ends.
use super::types::Lyric;
fn blank(line: &Lyric) -> bool {
    line.words
        .chars()
        .all(|c| c.is_whitespace() || matches!(c, '♪' | '♫' | '♬' | '♩' | '🎵' | '🎶' | '\u{fe0f}'))
        && !line.parts.as_ref().is_some_and(|p| {
            p.iter()
                .any(|p| p.is_background && !p.words.trim().is_empty())
        })
}
fn gap(start: i64, end: i64) -> Lyric {
    Lyric {
        start_time_ms: start,
        duration_ms: end.saturating_sub(start),
        is_instrumental: true,
        parts: Some(vec![]),
        ..Default::default()
    }
}
pub fn insert_instrumental_breaks(
    mut lyrics: Vec<Lyric>,
    duration: i64,
    threshold: i64,
) -> Vec<Lyric> {
    if lyrics.is_empty() {
        return lyrics;
    }
    lyrics.sort_by_key(|l| l.start_time_ms);
    let mut merged: Vec<Lyric> = Vec::with_capacity(lyrics.len());
    for mut line in lyrics {
        line.duration_ms = line.duration_ms.max(0);
        line.start_time_ms = line.start_time_ms.max(0);
        if blank(&line) {
            line.words.clear();
            line.is_instrumental = true;
            line.parts = Some(vec![]);
            if let Some(prev) = merged.last_mut().filter(|p| p.is_instrumental) {
                let end = (line.start_time_ms + line.duration_ms)
                    .max(prev.start_time_ms + prev.duration_ms);
                prev.duration_ms = end - prev.start_time_ms;
                continue;
            }
        }
        merged.push(line);
    }
    let mut result = Vec::with_capacity(merged.len() + 2);
    let mut covered = 0;
    for line in merged {
        if line.start_time_ms - covered > threshold {
            result.push(gap(covered, line.start_time_ms));
        }
        covered = covered.max(line.start_time_ms.saturating_add(line.duration_ms));
        if !line.is_instrumental || line.duration_ms > threshold {
            result.push(line);
        }
    }
    if duration - covered > threshold {
        result.push(gap(covered, duration));
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlaps_do_not_create_false_gaps() {
        let lines = vec![
            Lyric {
                start_time_ms: 0,
                duration_ms: 20000,
                words: "held".into(),
                ..Default::default()
            },
            Lyric {
                start_time_ms: 1000,
                duration_ms: 1000,
                words: "echo".into(),
                ..Default::default()
            },
            Lyric {
                start_time_ms: 15000,
                duration_ms: 1000,
                words: "next".into(),
                ..Default::default()
            },
        ];
        assert!(!insert_instrumental_breaks(lines, 20000, 5000)
            .iter()
            .any(|l| l.is_instrumental));
    }
}
