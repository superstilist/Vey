//! Display-only enrichment, following Better Lyrics' Unison/Google batch flow.
use crate::{lyrics::LyricsAlternative, services::Settings};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

fn non_latin(text: &str) -> bool {
    use unicode_script::{Script, UnicodeScript};
    text.chars().any(|c| {
        c.is_alphabetic()
            && !matches!(
                c.script(),
                Script::Latin | Script::Common | Script::Inherited
            )
    })
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Request {
    pub lines: Vec<String>,
    pub to: String,
    pub from: Option<String>,
    pub video_id: Option<String>,
    pub translation: bool,
    pub romanization: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Line {
    pub translation: Option<String>,
    pub romanization: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Enrichment {
    pub lines: Vec<Line>,
}
impl Request {
    pub fn for_song(
        alt: &LyricsAlternative,
        s: &Settings,
        video_id: Option<String>,
    ) -> Option<Self> {
        if (!s.lyrics_translation && !s.lyrics_romanization) || alt.lyrics.lines.is_empty() {
            return None;
        }
        Some(Self {
            lines: alt.lyrics.lines.iter().map(|l| l.text.clone()).collect(),
            to: if s.lyrics_translation_language == "auto" {
                crate::i18n::language(&s.language).into()
            } else {
                crate::i18n::language(&s.lyrics_translation_language).into()
            },
            from: alt.lyrics.language.clone(),
            video_id,
            translation: s.lyrics_translation,
            romanization: s.lyrics_romanization,
        })
    }
    pub fn key(&self) -> String {
        format!(
            "{}-{:x}",
            if self.romanization { "v2" } else { "v1" },
            Sha1::digest(serde_json::to_vec(self).unwrap_or_default())
        )
    }
    pub fn needs_network(&self, alt: &LyricsAlternative) -> bool {
        alt.lyrics.lines.iter().any(|line| {
            let text = line.text.trim();
            !text.is_empty()
                && text != "♪"
                && ((self.translation && !line.translations.contains_key(&self.to))
                    || (self.romanization
                        && non_latin(text)
                        && line.romanization.is_empty()
                        && line.timed_romanization.is_empty()))
        })
    }
    pub fn apply(&self, value: &Enrichment, alt: &mut LyricsAlternative) {
        if value.lines.len() != alt.lyrics.lines.len() {
            return;
        }
        for (line, extra) in alt.lyrics.lines.iter_mut().zip(&value.lines) {
            // Prefer an explicit source translation in the requested language.
            if let Some(text) = line
                .translations
                .get(&self.to)
                .cloned()
                .or_else(|| extra.translation.clone())
            {
                line.translation = text.clone();
                // The widget otherwise picks the first language, often uk/en.
                line.translations.clear();
                line.translations.insert(self.to.clone(), text);
            } else if self.translation {
                // A translation with an unknown or different language cannot
                // satisfy the language selected in the player menu.
                line.translation.clear();
                line.translations.clear();
            }
            if line.romanization.is_empty() && line.timed_romanization.is_empty() {
                if let Some(text) = &extra.romanization {
                    line.romanization = text.clone();
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod network {
    use super::*;
    use serde_json::Value;
    const SEPARATOR: &str = "\n\n;\n\n";
    fn different(text: Option<&str>, original: &str) -> Option<String> {
        text.map(str::trim)
            .filter(|s| !s.is_empty() && s.to_lowercase() != original.trim().to_lowercase())
            .map(str::to_owned)
    }
    fn romanized(text: Option<&str>, original: &str) -> Option<String> {
        different(text, original).filter(|text| !non_latin(text))
    }
    fn language_hint(raw: &str) -> Option<String> {
        let language = raw.split(['-', '_']).next()?.to_lowercase();
        let language = match language.as_str() {
            "jpn" => "ja",
            "kor" => "ko",
            "zho" | "chi" => "zh",
            "rus" => "ru",
            "ukr" => "uk",
            "ara" => "ar",
            "hin" => "hi",
            "auto" | "und" | "" => return None,
            other => other,
        };
        Some(language.into())
    }
    // Infer the source independently of translation. Per-line script groups keep
    // mixed Japanese/Korean/Chinese songs out of a single, wrong *-Latn request.
    fn romanization_language(text: &str, hint: Option<&str>, song: Option<&str>) -> Option<String> {
        use unicode_script::{Script, UnicodeScript};
        let hint = hint.and_then(language_hint);
        if text
            .chars()
            .any(|c| matches!(c.script(), Script::Hiragana | Script::Katakana))
        {
            return Some("ja".into());
        }
        if text.chars().any(|c| c.script() == Script::Hangul) {
            return Some("ko".into());
        }
        let has_ukrainian_letters = text.chars().any(|c| "іїєґІЇЄҐ".contains(c));
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for c in text.chars().filter(|c| c.is_alphabetic()) {
            let language = match c.script() {
                Script::Han => match hint
                    .as_deref()
                    .filter(|lang| matches!(*lang, "ja" | "ko" | "zh"))
                    .or(song)
                {
                    Some("ja") => "ja",
                    Some("ko") => "ko",
                    _ => "zh",
                },
                Script::Cyrillic => match hint.as_deref() {
                    Some("uk") => "uk",
                    Some("be") => "be",
                    Some("bg") => "bg",
                    Some("sr") => "sr",
                    Some("mk") => "mk",
                    _ if has_ukrainian_letters => "uk",
                    _ => "ru",
                },
                Script::Arabic => match hint.as_deref() {
                    Some("fa") => "fa",
                    Some("ur") => "ur",
                    _ => "ar",
                },
                Script::Devanagari => match hint.as_deref() {
                    Some("mr") => "mr",
                    Some("ne") => "ne",
                    _ => "hi",
                },
                Script::Greek => "el",
                Script::Hebrew => "he",
                Script::Thai => "th",
                Script::Bengali => "bn",
                Script::Tamil => "ta",
                Script::Telugu => "te",
                Script::Malayalam => "ml",
                Script::Kannada => "kn",
                Script::Gujarati => "gu",
                Script::Gurmukhi => "pa",
                Script::Sinhala => "si",
                Script::Myanmar => "my",
                Script::Georgian => "ka",
                Script::Khmer => "km",
                Script::Lao => "lo",
                _ => continue,
            };
            *counts.entry(language).or_default() += 1;
        }
        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(language, _)| language.to_owned())
    }
    async fn json(response: reqwest::Response) -> Result<Value, String> {
        let response = response.error_for_status().map_err(|e| e.to_string())?;
        if response.content_length().is_some_and(|n| n > 2_000_000) {
            return Err("Translation response too large".into());
        }
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        if bytes.len() > 2_000_000 {
            return Err("Translation response too large".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }
    pub async fn fetch(request: &Request) -> Result<Enrichment, String> {
        static GATE: std::sync::LazyLock<tokio::sync::Semaphore> =
            std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(3));
        let _permit = GATE.acquire().await.map_err(|e| e.to_string())?;
        if request.lines.len() > 1500
            || request.lines.iter().map(String::len).sum::<usize>() > 150_000
            || !crate::i18n::LANGUAGES.contains(&request.to.as_str())
        {
            return Err("Invalid translation request".into());
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        let mut result = Enrichment {
            lines: vec![Line::default(); request.lines.len()],
        };
        // Chunk by encoded size, deduplicate repeated chorus lines and skip gaps.
        let mut unique = Vec::<String>::new();
        for text in &request.lines {
            let text = text.trim();
            if !text.is_empty() && text != "♪" && !unique.iter().any(|s| s == text) {
                unique.push(text.into());
            }
        }
        let mut chunks: Vec<Vec<String>> = vec![];
        let mut size = 0;
        for text in unique {
            let encoded =
                reqwest::Url::parse_with_params("https://example.invalid", &[("q", text.as_str())])
                    .map_err(|e| e.to_string())?
                    .as_str()
                    .len();
            if encoded > 12000 {
                return Err("Lyric line too long to translate".into());
            }
            if chunks.is_empty()
                || size + encoded > 12000
                || chunks.last().is_some_and(|c| c.len() >= 200)
            {
                chunks.push(vec![]);
                size = 0;
            }
            size += encoded + 50;
            chunks.last_mut().unwrap().push(text);
        }
        let mut any_success = false;
        let song_language =
            romanization_language(&request.lines.join("\n"), request.from.as_deref(), None);
        for chunk in chunks {
            let mut extras = vec![Line::default(); chunk.len()];
            let mut detected = request.from.clone().unwrap_or_else(|| "auto".into());
            let mut body = serde_json::json!({"lines":chunk,"to":request.to});
            if let Some(from) = &request.from {
                body["from"] = from.clone().into();
            }
            if let Some(id) = &request.video_id {
                body["videoId"] = id.clone().into();
            }
            if chunk.iter().all(|line| line.encode_utf16().count() <= 500) {
                if let Ok(response) = client
                    .post("https://unison.betterlyrics.org/translate")
                    .json(&body)
                    .send()
                    .await
                {
                    if let Ok(value) = json(response).await {
                        if let Some(lines) =
                            value["lines"].as_array().filter(|l| l.len() == chunk.len())
                        {
                            any_success = true;
                            if let Some(lang) = value["detectedLang"].as_str() {
                                detected = lang.into();
                            }
                            for ((extra, line), source) in extras.iter_mut().zip(lines).zip(&chunk)
                            {
                                if request.translation
                                    && line["needsTranslation"].as_bool() == Some(true)
                                {
                                    extra.translation =
                                        different(line["translation"].as_str(), source);
                                }
                                if request.romanization && non_latin(source) {
                                    extra.romanization =
                                        romanized(line["romanization"].as_str(), source);
                                }
                            }
                        }
                    }
                }
            }
            // Separate Latin/non-Latin groups as upstream does for mixed-script songs.
            for is_non_latin in [true, false] {
                let missing: Vec<_> = (0..chunk.len())
                    .filter(|&i| {
                        request.translation
                            && extras[i].translation.is_none()
                            && non_latin(&chunk[i]) == is_non_latin
                    })
                    .collect();
                if missing.is_empty() {
                    continue;
                }
                let text = missing
                    .iter()
                    .map(|&i| chunk[i].as_str())
                    .collect::<Vec<_>>()
                    .join(SEPARATOR);
                if let Ok(response) = client
                    .get("https://translate.googleapis.com/translate_a/single")
                    .query(&[
                        ("client", "gtx"),
                        ("sl", "auto"),
                        ("tl", request.to.as_str()),
                        ("dt", "t"),
                        ("q", text.as_str()),
                    ])
                    .send()
                    .await
                {
                    if let Ok(value) = json(response).await {
                        if let Some(parts) = value[0].as_array() {
                            let translated = parts
                                .iter()
                                .filter_map(|p| p[0].as_str())
                                .collect::<String>();
                            let mut lines: Vec<_> = translated.split(SEPARATOR).collect();
                            if lines.len() != missing.len() {
                                lines = translated
                                    .split(';')
                                    .filter(|s| !s.trim().is_empty())
                                    .collect();
                            }
                            // Never attach a merged or shifted translation to the wrong line.
                            if lines.len() == missing.len() {
                                any_success = true;
                                for (&i, text) in missing.iter().zip(lines) {
                                    extras[i].translation = different(Some(text), &chunk[i]);
                                }
                            }
                            if detected == "auto" {
                                detected = value[2].as_str().unwrap_or("auto").into();
                            }
                        }
                    }
                }
            }
            let mut groups = std::collections::BTreeMap::<String, Vec<usize>>::new();
            for i in 0..chunk.len() {
                if request.romanization && extras[i].romanization.is_none() && non_latin(&chunk[i])
                {
                    if let Some(language) =
                        romanization_language(&chunk[i], Some(&detected), song_language.as_deref())
                    {
                        groups.entry(language).or_default().push(i);
                    }
                }
            }
            for (language, missing) in groups {
                let mut separator = "0000".to_string();
                while chunk.iter().any(|s| s.contains(&separator)) {
                    separator.push('0');
                }
                let text = missing
                    .iter()
                    .map(|&i| chunk[i].as_str())
                    .collect::<Vec<_>>()
                    .join(&format!(" {separator} "));
                let target = format!("{language}-Latn");
                if let Ok(response) = client
                    .get("https://translate.googleapis.com/translate_a/single")
                    .query(&[
                        ("client", "gtx"),
                        ("sl", language.as_str()),
                        ("tl", target.as_str()),
                        ("dt", "t"),
                        ("dt", "rm"),
                        ("q", text.as_str()),
                    ])
                    .send()
                    .await
                {
                    if let Ok(value) = json(response).await {
                        if let Some(parts) = value[0].as_array() {
                            let text = parts
                                .iter()
                                .filter_map(|p| p[3].as_str().or_else(|| p[2].as_str()))
                                .collect::<String>();
                            let lines: Vec<_> = text.split(&separator).collect();
                            if lines.len() == missing.len() {
                                any_success = true;
                                for (&i, text) in missing.iter().zip(lines) {
                                    extras[i].romanization = romanized(Some(text), &chunk[i]);
                                }
                            }
                        }
                    }
                }
            }
            for (source, extra) in chunk.iter().zip(extras) {
                for (i, text) in request.lines.iter().enumerate() {
                    if text.trim() == source {
                        result.lines[i] = extra.clone();
                    }
                }
            }
        }
        if request.romanization
            && !request.translation
            && request.lines.iter().any(|text| non_latin(text))
            && result.lines.iter().all(|line| line.romanization.is_none())
        {
            return Err("Романізація недоступна. Спробуйте оновити слова.".into());
        }
        if any_success
            || request
                .lines
                .iter()
                .all(|l| l.trim().is_empty() || l.trim() == "♪")
        {
            Ok(result)
        } else {
            Err("Переклад недоступний. Спробуйте оновити слова.".into())
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use network::fetch;
