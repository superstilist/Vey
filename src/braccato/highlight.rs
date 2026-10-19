//! Lossless native syntax tokens for all lyric formats (@braccato/highlight).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    Text,
    BackgroundText,
    Timestamp,
    WordTime,
    Agent,
    Meta,
    Tag,
    Attribute,
    Value,
    Punctuation,
    Comment,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenType,
    pub text: String,
}
fn push(out: &mut Vec<Token>, kind: TokenType, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut().filter(|t| t.kind == kind) {
        last.text.push_str(text);
    } else {
        out.push(Token {
            kind,
            text: text.into(),
        });
    }
}
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut out = vec![];
    let mut i = 0;
    let mut background = false;
    while i < src.len() {
        let rest = &src[i..];
        if rest.starts_with("<!--") {
            let n = rest.find("-->").map(|n| n + 3).unwrap_or(rest.len());
            push(&mut out, TokenType::Comment, &rest[..n]);
            i += n;
            continue;
        }
        if rest.starts_with('<')
            && rest
                .as_bytes()
                .get(1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'/' || *c == b'?' || *c == b'!')
        {
            let end = rest.find('>').map(|n| n + 1).unwrap_or(rest.len());
            let tag = &rest[..end];
            background = (background || tag.contains("x-bg")) && !tag.starts_with("</");
            xml_tag(tag, &mut out);
            i += end;
            continue;
        }
        if rest.starts_with('[') || rest.starts_with('<') || rest.starts_with('(') {
            let open = rest.as_bytes()[0] as char;
            let close = match open {
                '[' => ']',
                '<' => '>',
                _ => ')',
            };
            if let Some(end) = rest.find(close) {
                let body = &rest[1..end];
                let timing = body
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, ':' | '.' | ','));
                if timing && body.chars().any(|c| c.is_ascii_digit()) {
                    push(&mut out, TokenType::Punctuation, &rest[..1]);
                    push(
                        &mut out,
                        if open == '[' {
                            TokenType::Timestamp
                        } else {
                            TokenType::WordTime
                        },
                        body,
                    );
                    push(&mut out, TokenType::Punctuation, &rest[end..end + 1]);
                    i += end + 1;
                    continue;
                }
                if open == '[' {
                    if let Some((name, value)) = body.split_once(':') {
                        if name.chars().all(|c| c.is_ascii_alphabetic() || c == '#') {
                            push(&mut out, TokenType::Punctuation, "[");
                            push(&mut out, TokenType::Meta, name);
                            push(&mut out, TokenType::Punctuation, ":");
                            push(&mut out, TokenType::Value, value);
                            push(&mut out, TokenType::Punctuation, "]");
                            i += end + 1;
                            continue;
                        }
                    }
                }
            }
        }
        // Voice prefixes remain distinct from words and timestamps.
        if rest.starts_with('v') || rest.starts_with("bg:") {
            if let Some(end) = rest.find(':').filter(|n| *n < 12) {
                let name = &rest[..end];
                if name == "bg"
                    || name
                        .strip_prefix('v')
                        .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                {
                    push(&mut out, TokenType::Agent, name);
                    push(&mut out, TokenType::Punctuation, ":");
                    i += end + 1;
                    continue;
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        push(
            &mut out,
            if background {
                TokenType::BackgroundText
            } else {
                TokenType::Text
            },
            &rest[..ch.len_utf8()],
        );
        i += ch.len_utf8();
    }
    out
}
fn xml_tag(tag: &str, out: &mut Vec<Token>) {
    let mut i = 0;
    let mut first = true;
    while i < tag.len() {
        let rest = &tag[i..];
        let ch = rest.chars().next().unwrap();
        if matches!(ch, '<' | '>' | '/' | '?' | '!' | '=') {
            push(out, TokenType::Punctuation, &rest[..1]);
            i += 1;
            continue;
        }
        if matches!(ch, '\'' | '"') {
            let end = rest[1..].find(ch).map(|n| n + 2).unwrap_or(rest.len());
            push(out, TokenType::Value, &rest[..end]);
            i += end;
            continue;
        }
        if ch.is_whitespace() {
            push(out, TokenType::Text, &rest[..ch.len_utf8()]);
            i += ch.len_utf8();
            continue;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '/' | '?' | '\'' | '"'))
            .unwrap_or(rest.len())
            .max(ch.len_utf8());
        push(
            out,
            if first {
                TokenType::Tag
            } else {
                TokenType::Attribute
            },
            &rest[..end],
        );
        first = false;
        i += end;
    }
}
pub fn lines(tokens: &[Token]) -> Vec<String> {
    tokens
        .iter()
        .map(|t| t.text.as_str())
        .collect::<String>()
        .split('\n')
        .map(str::to_string)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lossless_unicode_and_line_endings() {
        for src in [
            "[ti:Song]\r\n[00:10.00]v1:<00:10.00>Привіт 👨‍👩‍👧‍👦",
            "1\n00:00:01,000 --> 00:00:02,000\nHello",
            "<!--hi--><tt><p begin='1s'>世界</p></tt>",
        ] {
            let tokens = tokenize(src);
            assert_eq!(
                tokens.iter().map(|t| t.text.as_str()).collect::<String>(),
                src
            );
            assert!(tokens.windows(2).all(|w| w[0].kind != w[1].kind));
        }
    }
    #[test]
    fn recognizes_timing_and_xml() {
        assert!(tokenize("[00:01.00]<00:01.00>Hello")
            .iter()
            .any(|t| t.kind == TokenType::WordTime));
        assert!(tokenize("<p begin='1s'>Hi</p>")
            .iter()
            .any(|t| t.kind == TokenType::Attribute));
    }
}
