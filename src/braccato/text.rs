//! Unicode text handling from Braccato's text/language modules, implemented in Rust.
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;
pub fn graphemes(text: &str) -> impl Iterator<Item = &str> {
    text.graphemes(true)
}
pub fn is_rtl(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c.script(),
            Script::Arabic
                | Script::Hebrew
                | Script::Syriac
                | Script::Thaana
                | Script::Nko
                | Script::Adlam
                | Script::Mandaic
                | Script::Samaritan
                | Script::Yezidi
                | Script::Old_Hungarian
        )
    })
}
pub fn is_joining(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c.script(),
            Script::Arabic
                | Script::Syriac
                | Script::Nko
                | Script::Mongolian
                | Script::Adlam
                | Script::Mandaic
        )
    })
}
pub fn contains_non_latin(text: &str) -> bool {
    text.chars().any(|c| {
        !matches!(
            c.script(),
            Script::Latin | Script::Common | Script::Inherited
        )
    })
}
pub fn normalize_language(raw: &str) -> String {
    let raw = raw.trim().replace('_', "-");
    if matches!(raw.as_str(), "" | "auto" | "und")
        || raw
            .split('-')
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return String::new();
    }
    raw.split('-')
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.to_ascii_lowercase()
            } else if s.len() == 2 {
                s.to_ascii_uppercase()
            } else if s.len() == 4 {
                format!(
                    "{}{}",
                    s[..1].to_ascii_uppercase(),
                    s[1..].to_ascii_lowercase()
                )
            } else {
                s.into()
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}
pub fn detect_language(text: &str) -> Option<&'static str> {
    let scripts = text.chars().map(|c| c.script()).collect::<Vec<_>>();
    [
        (Script::Hiragana, "ja"),
        (Script::Katakana, "ja"),
        (Script::Hangul, "ko"),
        (Script::Han, "zh"),
        (Script::Cyrillic, "ru"),
        (Script::Devanagari, "hi"),
        (Script::Arabic, "ar"),
        (Script::Thai, "th"),
        (Script::Greek, "el"),
        (Script::Hebrew, "he"),
        (Script::Bengali, "bn"),
        (Script::Tamil, "ta"),
        (Script::Telugu, "te"),
        (Script::Malayalam, "ml"),
        (Script::Kannada, "kn"),
        (Script::Gujarati, "gu"),
        (Script::Gurmukhi, "pa"),
        (Script::Sinhala, "si"),
        (Script::Myanmar, "my"),
        (Script::Georgian, "ka"),
        (Script::Khmer, "km"),
        (Script::Lao, "lo"),
    ]
    .into_iter()
    .find_map(|(script, lang)| scripts.contains(&script).then_some(lang))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joining_and_graphemes() {
        assert!(is_rtl("مرحبا"));
        assert!(is_joining("العربية"));
        assert_eq!(graphemes("e\u{301}👨‍👩‍👧‍👦").count(), 2);
        assert!(!is_rtl("Україна"));
    }
    #[test]
    fn languages() {
        assert_eq!(normalize_language("zh_hant_TW"), "zh-Hant-TW");
        assert_eq!(detect_language("日本かな"), Some("ja"));
        assert_eq!(normalize_language("und"), "");
    }
}
