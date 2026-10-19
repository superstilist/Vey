use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricPart {
    pub start_time_ms: i64,
    pub words: String,
    pub duration_ms: i64,
    pub is_background: bool,
    pub explicit: bool,
}

impl Default for LyricPart {
    fn default() -> Self {
        Self {
            start_time_ms: 0,
            words: String::new(),
            duration_ms: 0,
            is_background: false,
            explicit: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Lyric {
    pub start_time_ms: i64,
    pub words: String,
    pub duration_ms: i64,
    pub key: Option<String>,
    pub parts: Option<Vec<LyricPart>>,
    pub agent: Option<String>,
    pub translations: Option<HashMap<String, String>>,
    pub translation: Option<Translation>,
    pub romanization: Option<String>,
    pub timed_romanization: Option<Vec<LyricPart>>,
    pub is_instrumental: bool,
}

impl Default for Lyric {
    fn default() -> Self {
        Self {
            start_time_ms: 0,
            words: String::new(),
            duration_ms: 0,
            key: None,
            parts: None,
            agent: None,
            translations: None,
            translation: None,
            romanization: None,
            timed_romanization: None,
            is_instrumental: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Translation {
    pub text: String,
    pub lang: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LyricSyncType {
    RichSync,
    Synced,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct LyricMetadata {
    pub songwriters: Vec<String>,
}
