//! Rust implementation of Braccato's Apple/AMLL TTML contract.
//! MIT: better-lyrics/braccato, packages/parsers/src/ttml.ts (see third-party/).
use crate::lyrics::{LyricLine, LyricPart, ParsedLyrics};
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};
static PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:</?|\s)([A-Za-z][\w.-]*):[\w.-]+").unwrap());
static DECLARED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"xmlns:([A-Za-z][\w.-]*)\s*=").unwrap());
static ROOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<(?:[\w.-]+:)?tt\b[^>]*>").unwrap());
static ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&([A-Za-z][A-Za-z0-9]+);").unwrap());
pub fn prepare_xml(raw: &str) -> String {
    let mut xml = ENTITY
        .replace_all(raw, |c: &regex::Captures<'_>| {
            if matches!(&c[1], "amp" | "lt" | "gt" | "quot" | "apos") {
                return c[0].to_string();
            }
            let decoded = html_escape::decode_html_entities(&c[0]);
            if decoded == c[0] {
                return c[0].to_string();
            }
            decoded
                .chars()
                .map(|ch| format!("&#{};", ch as u32))
                .collect()
        })
        .into_owned();
    if let Some(root) = ROOT.find(&xml) {
        let declared = DECLARED
            .captures_iter(root.as_str())
            .map(|c| c[1].to_string())
            .chain(["xml".into(), "xmlns".into()])
            .collect::<BTreeSet<_>>();
        let missing = PREFIX
            .captures_iter(&xml)
            .map(|c| c[1].to_string())
            .filter(|p| !declared.contains(p))
            .collect::<BTreeSet<_>>();
        let additions = missing
            .iter()
            .map(|p| format!(" xmlns:{p}=\"urn:braccato:unbound:{p}\""))
            .collect::<String>();
        xml.insert_str(root.end() - 1, &additions);
    }
    xml
}
pub fn time(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    let convert = |v: &str, m: f64| {
        v.parse::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n >= 0.)
            .map(|n| (n * m).round() as u64)
    };
    for (unit, m) in [("ms", 1.), ("h", 3600000.), ("m", 60000.), ("s", 1000.)] {
        if let Some(v) = raw.strip_suffix(unit) {
            return convert(v, m);
        }
    }
    let fields = raw.split(':').collect::<Vec<_>>();
    match fields.as_slice() {
        [s] => convert(s, 1000.),
        [m, s] => Some(convert(m, 60000.)?.saturating_add(convert(s, 1000.)?)),
        [h, m, s] => Some(
            convert(h, 3600000.)?
                .saturating_add(convert(m, 60000.)?)
                .saturating_add(convert(s, 1000.)?),
        ),
        _ => None,
    }
}
fn attr<'a>(node: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == name)
        .map(|a| a.value())
}
fn text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<String>()
}
fn role<'a>(node: roxmltree::Node<'a, '_>) -> &'a str {
    attr(node, "role").unwrap_or("")
}
fn parts(
    paragraph: roxmltree::Node<'_, '_>,
    start: u64,
    end: u64,
) -> (Vec<LyricPart>, String, String) {
    let mut parts = vec![];
    let mut translation = String::new();
    let mut roman = String::new();
    for n in paragraph.descendants() {
        if n.is_element() && n.tag_name().name() == "br" {
            parts.push(LyricPart {
                start_ms: start,
                duration_ms: 0,
                text: "\n".into(),
                ..Default::default()
            });
            continue;
        }
        if !n.is_text() {
            continue;
        }
        let value = n.text().unwrap_or("");
        if value.trim().is_empty() && value.contains(['\n', '\r']) {
            continue;
        }
        let ancestors = n
            .ancestors()
            .take_while(|p| *p != paragraph)
            .filter(|p| p.is_element())
            .collect::<Vec<_>>();
        if ancestors.iter().any(|p| role(*p) == "x-translation") {
            translation.push_str(value);
            continue;
        }
        if ancestors
            .iter()
            .any(|p| matches!(role(*p), "x-roman" | "x-romanization"))
        {
            roman.push_str(value);
            continue;
        }
        let begin = ancestors
            .iter()
            .find_map(|p| attr(*p, "begin").and_then(time))
            .unwrap_or(start);
        let finish = ancestors
            .iter()
            .find_map(|p| {
                attr(*p, "end").and_then(time).or_else(|| {
                    attr(*p, "dur")
                        .and_then(time)
                        .map(|d| begin.saturating_add(d))
                })
            })
            .unwrap_or(end);
        parts.push(LyricPart {
            start_ms: begin,
            duration_ms: finish.saturating_sub(begin),
            text: value.into(),
            is_background: ancestors.iter().any(|p| role(*p) == "x-bg"),
            explicit: ancestors.iter().any(|p| {
                attr(*p, "explicit") == Some("true") || attr(*p, "obscene") == Some("true")
            }),
        });
    }
    (parts, translation, roman)
}
pub fn parse(raw: &str) -> ParsedLyrics {
    let xml = prepare_xml(raw);
    let Ok(doc) = roxmltree::Document::parse(&xml) else {
        return ParsedLyrics::default();
    };
    if doc.root_element().tag_name().name() != "tt" {
        return ParsedLyrics::default();
    }
    let mut result = ParsedLyrics {
        language: attr(doc.root_element(), "lang").map(str::to_string),
        ..Default::default()
    };
    let mut agents = BTreeMap::new();
    let mut slot = 0;
    for n in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "agent")
    {
        if let Some(id) = attr(n, "id") {
            let name = if attr(n, "type") == Some("person") {
                slot += 1;
                format!("v{slot}")
            } else {
                "v1000".into()
            };
            agents.insert(id.to_string(), name);
        }
    }
    for n in doc.descendants().filter(|n| n.is_element()) {
        let name = n.tag_name().name();
        let writer = if name == "songwriter" {
            Some(text(n))
        } else if name == "meta" && matches!(attr(n, "key"), Some("songwriter" | "songwriters")) {
            attr(n, "value").map(str::to_string)
        } else {
            None
        };
        if let Some(writer) = writer
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            if !result.songwriters.contains(&writer) {
                result.songwriters.push(writer);
            }
        }
        if name != "p"
            || !n
                .ancestors()
                .any(|a| a.is_element() && a.tag_name().name() == "body")
        {
            continue;
        }
        let Some(start) = n.ancestors().find_map(|a| attr(a, "begin").and_then(time)) else {
            continue;
        };
        let end = attr(n, "end")
            .and_then(time)
            .or_else(|| {
                attr(n, "dur")
                    .and_then(time)
                    .map(|d| start.saturating_add(d))
            })
            .unwrap_or(start);
        let (parts, translation, romanization) = parts(n, start, end);
        let words = parts
            .iter()
            .filter(|p| !p.is_background)
            .map(|p| p.text.as_str())
            .collect::<String>()
            .trim()
            .to_string();
        if words.is_empty() && !parts.iter().any(|p| !p.text.trim().is_empty()) {
            continue;
        }
        let agent = attr(n, "agent")
            .map(|a| agents.get(a).cloned().unwrap_or(a.into()))
            .unwrap_or_default();
        let word_end = parts
            .iter()
            .map(|p| p.start_ms.saturating_add(p.duration_ms))
            .max()
            .unwrap_or(start);
        result.lines.push(LyricLine {
            start_ms: start,
            duration_ms: end.max(word_end).saturating_sub(start),
            text: words,
            parts,
            translation,
            romanization,
            key: attr(n, "key")
                .or_else(|| attr(n, "id"))
                .unwrap_or("")
                .into(),
            agent,
            ..Default::default()
        });
    }
    // Both dialects put `for` and `lang` on different levels. Resolve them
    // from the text node's nearest ancestors and fan out over repeated keys.
    for n in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "text")
    {
        let translation = n.ancestors().any(|p| {
            p.is_element() && matches!(p.tag_name().name(), "translation" | "translations")
        });
        let roman = n.ancestors().any(|p| {
            p.is_element() && matches!(p.tag_name().name(), "transliteration" | "transliterations")
        });
        if !translation && !roman {
            continue;
        }
        let Some(key) = n.ancestors().find_map(|p| attr(p, "for")) else {
            continue;
        };
        let language = n.ancestors().find_map(|p| attr(p, "lang")).unwrap_or("und");
        let words = text(n);
        for line in result.lines.iter_mut().filter(|l| l.key == key) {
            if translation {
                line.translations.insert(language.into(), words.clone());
                if line.translation.is_empty() {
                    line.translation = words.clone();
                }
            } else {
                line.romanization = words.clone();
                line.timed_romanization =
                    parts(n, line.start_ms, line.start_ms + line.duration_ms).0;
            }
        }
    }
    result.lines.sort_by_key(|l| l.start_ms);
    for i in 0..result.lines.len() {
        if result.lines[i].duration_ms == 0 {
            if let Some(next) = result.lines.get(i + 1) {
                result.lines[i].duration_ms =
                    next.start_ms.saturating_sub(result.lines[i].start_ms);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apple_metadata_and_repeated_keys() {
        let l = parse(
            r#"<tt xml:lang="ja"><head><metadata><iTunesMetadata><songwriters><songwriter>A, B</songwriter></songwriters></iTunesMetadata><translations><translation xml:lang="en"><text for="l1">Hello</text></translation></translations><transliterations><transliteration><text for="l1"><span begin="1s" end="2s">konnichiwa</span></text></transliteration></transliterations></metadata></head><body><div><p begin="1s" end="2s" itunes:key="l1">こんにちは</p><p begin="3s" end="4s" itunes:key="l1">こんにちは</p></div></body></tt>"#,
        );
        assert_eq!(l.lines.len(), 2);
        assert_eq!(l.language.as_deref(), Some("ja"));
        assert_eq!(l.songwriters, ["A, B"]);
        for line in l.lines {
            assert_eq!(line.translations["en"], "Hello");
            assert_eq!(line.romanization, "konnichiwa");
            assert_eq!(line.timed_romanization.len(), 1);
        }
    }
    #[test]
    fn agents_entities_background_and_explicit() {
        let l = parse(
            r#"<tt><head><metadata><ttm:agent xml:id="singer" type="person"/></metadata></head><body><p begin="1m" end="61s" ttm:agent="singer"><span begin="60s" end="61s" amll:obscene="true">It&rsquo;s caf&#233;</span><span ttm:role="x-bg">(hey)</span></p></body></tt>"#,
        );
        assert_eq!(l.lines[0].agent, "v1");
        assert_eq!(l.lines[0].text, "It’s café");
        assert!(l.lines[0].parts[0].explicit);
        assert!(l.lines[0].parts[1].is_background);
    }
    #[test]
    fn invalid_is_rejected_and_units_are_finite() {
        assert!(parse("<tt><body><p begin='1s'>broken").lines.is_empty());
        assert_eq!(time("1.5h"), Some(5400000));
        assert_eq!(time("NaNs"), None);
    }
}
