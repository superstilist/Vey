//! One catalog for Slint and controller messages. Catalog lookup never translates track/lyric text; lyric enrichment is separate.
use crate::{I18n, MainWindow};
use slint::ComponentHandle;
pub const LANGUAGES: [&str; 8] = ["uk", "en", "cs", "pl", "de", "es", "fr", "ja"];
static CURRENT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn set_current(locale: &str) {
    CURRENT.store(
        LANGUAGES.iter().position(|l| *l == locale).unwrap_or(0),
        std::sync::atomic::Ordering::Relaxed,
    );
}
pub fn current() -> &'static str {
    LANGUAGES[CURRENT.load(std::sync::atomic::Ordering::Relaxed)]
}
type Entry = [&'static str; 8];
fn catalog() -> &'static std::collections::HashMap<&'static str, Entry> {
    static CATALOG: std::sync::OnceLock<std::collections::HashMap<&'static str, Entry>> =
        std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        include_str!("../locales/catalog.tsv")
            .lines()
            .filter_map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                let entry: Entry = fields.try_into().ok()?;
                Some((entry[0], entry))
            })
            .collect()
    })
}
pub fn language(value: &str) -> &str {
    if LANGUAGES.contains(&value) {
        value
    } else {
        "uk"
    }
}
pub fn translate(locale: &str, text: &str) -> String {
    let index = LANGUAGES.iter().position(|l| *l == locale).unwrap_or(0);
    if index == 0 || text.is_empty() {
        return text.into();
    }
    if let Some(entry) = catalog().get(text) {
        return entry[index].into();
    }
    // Compile message patterns once. Captured counts/platform errors remain verbatim.
    type Pattern = (regex::Regex, Vec<String>, Entry);
    static PATTERNS: std::sync::OnceLock<Vec<Pattern>> = std::sync::OnceLock::new();
    for (pattern, names, entry) in PATTERNS.get_or_init(|| {
        let placeholder = regex::Regex::new(r"\{[^}]*\}").unwrap();
        catalog()
            .values()
            .filter_map(|entry| {
                let names: Vec<_> = placeholder
                    .find_iter(entry[0])
                    .map(|m| m.as_str().to_owned())
                    .collect();
                if names.is_empty() {
                    return None;
                }
                let mut expression = String::from("^");
                let mut at = 0;
                for found in placeholder.find_iter(entry[0]) {
                    expression.push_str(&regex::escape(&entry[0][at..found.start()]));
                    expression.push_str("(.*?)");
                    at = found.end();
                }
                expression.push_str(&regex::escape(&entry[0][at..]));
                expression.push('$');
                Some((regex::Regex::new(&expression).unwrap(), names, *entry))
            })
            .collect()
    }) {
        if let Some(captures) = pattern.captures(text) {
            let mut result = entry[index].to_owned();
            for (n, name) in names.iter().enumerate() {
                let value = captures.get(n + 1).map(|m| m.as_str()).unwrap_or("");
                let value = if name == "{e}" {
                    translate(locale, value)
                } else {
                    value.to_owned()
                };
                result = result.replacen(name, &value, 1);
            }
            return result;
        }
    }
    if text.contains(" · ") {
        return text
            .split(" · ")
            .map(|part| translate(locale, part))
            .collect::<Vec<_>>()
            .join(" · ");
    }
    text.into()
}
pub fn install(w: &MainWindow) {
    w.global::<I18n>()
        .on_translate(|locale, text| translate(&locale, &text).into());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages_translate_without_changing_dynamic_values() {
        assert_eq!(
            translate("en", "Знайдено композицій: 42"),
            "Tracks found: 42"
        );
        assert_eq!(
            translate("ja", "Збережено: C:/Music/Пісня.flac"),
            "保存しました：C:/Music/Пісня.flac"
        );
        assert_eq!(
            translate("en", "Помилка аудіо (4): Decoder failed"),
            "Audio error (4): Decoder failed"
        );
        assert_eq!(language("future"), "uk");
        assert_eq!(translate("fr", "A song title"), "A song title");
    }
    #[test]
    fn catalogs_have_all_languages_and_unique_keys() {
        let mut keys = std::collections::HashSet::new();
        for line in include_str!("../locales/catalog.tsv").lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 8, "{line}");
            assert!(fields.iter().all(|s| !s.is_empty()), "{line}");
            assert!(keys.insert(fields[0]), "duplicate: {}", fields[0]);
        }
        for source in [
            include_str!("../ui/main.slint"),
            include_str!("../ui/controls.slint"),
            include_str!("../ui/braccato-lyrics.slint"),
            include_str!("../ui/stage-navigator.slint"),
        ] {
            for part in source.split("I18n.tr(\"").skip(1) {
                let key = part.split('"').next().unwrap();
                assert!(keys.contains(key), "missing translation: {key}");
            }
        }
    }
}
