//! Shared, serializable lyric data used by parsers, providers and renderers.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricPart {
    pub start_time_ms: f64,
    pub words: String,
    pub duration_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_background: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyric {
    pub start_time_ms: f64,
    pub words: String,
    pub duration_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<LyricPart>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translations: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<Translation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romanization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed_romanization: Option<Vec<LyricPart>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_instrumental: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Translation {
    pub text: String,
    pub lang: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricSyncType {
    Richsync,
    Synced,
    None,
}

impl Lyric {
    pub fn is_instrumental(&self) -> bool {
        self.is_instrumental.unwrap_or(false)
    }

    pub fn sync_type(&self) -> LyricSyncType {
        if self.parts.as_ref().is_some_and(|parts| !parts.is_empty()) {
            LyricSyncType::Richsync
        } else if self.start_time_ms != 0.0 || self.duration_ms != 0.0 {
            LyricSyncType::Synced
        } else {
            LyricSyncType::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyric_json_keeps_camel_case_fields() {
        let lyric = Lyric {
            start_time_ms: 125.0,
            words: "hello".into(),
            duration_ms: 300.0,
            parts: Some(vec![LyricPart {
                start_time_ms: 125.0,
                words: "hello".into(),
                duration_ms: 300.0,
                is_background: Some(false),
                explicit: None,
            }]),
            ..Lyric::default()
        };
        let value = serde_json::to_value(&lyric).unwrap();
        assert_eq!(value["startTimeMs"], 125.0);
        assert_eq!(value["parts"][0]["isBackground"], false);
    }
}
