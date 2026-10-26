//! Lightweight lexical highlighting shared by native lyric editors.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Timestamp,
    Tag,
    Timing,
    Punctuation,
    Text,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
}

/// Tokenizes lyric text without changing it; concatenated token contents equal the input.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < input.len() {
        let tail = &input[cursor..];
        let candidate = if tail.starts_with('[') {
            tail.find(']').map(|end| (end + 1, TokenKind::Timestamp))
        } else if tail.starts_with('<') {
            tail.find('>').map(|end| (end + 1, TokenKind::Timing))
        } else if tail.starts_with('(') {
            tail.find(')').map(|end| (end + 1, TokenKind::Timing))
        } else {
            None
        };
        if let Some((end, kind)) = candidate {
            tokens.push(Token {
                kind,
                text: tail[..end].to_owned(),
            });
            cursor += end;
            continue;
        }
        let next = tail
            .char_indices()
            .skip(1)
            .find(|(_, c)| matches!(c, '[' | '<' | '('))
            .map(|(i, _)| i)
            .unwrap_or(tail.len());
        tokens.push(Token {
            kind: if tail.starts_with(':') || tail.starts_with(',') {
                TokenKind::Punctuation
            } else {
                TokenKind::Text
            },
            text: tail[..next].to_owned(),
        });
        cursor += next;
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_reconstruct_source() {
        let s = "[00:01.2]<00:01.2>Hello";
        let t = tokenize(s);
        assert_eq!(t.iter().map(|x| x.text.as_str()).collect::<String>(), s);
        assert_eq!(t[0].kind, TokenKind::Timestamp);
        assert_eq!(t[1].kind, TokenKind::Timing);
    }
}
