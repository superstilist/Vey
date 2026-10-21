use playervf::{braccato, lyrics::*};
use slint::Model;

fn alternative(raw: &str) -> LyricsAlternative {
    let (lyrics, format, sync_type) = parse_payload(raw).unwrap();
    LyricsAlternative {
        provider: "fixture".into(),
        format: format.into(),
        sync_type,
        lyrics,
    }
}

#[test]
fn genuine_syllables_survive_ttml_lrc_qrc_json_and_unison_envelopes() {
    let ttml = "<tt><body><p begin='1s' end='3s'><span begin='1s' end='1.5s'>Sin</span><span begin='1.5s' end='2s'>gle</span></p></body></tt>";
    let sources = [
        ttml.to_string(),
        "[00:01.000]<00:01.000>Sin<00:01.500>gle<00:02.000>".into(),
        "[1000,2000]Sin(1000,500)gle(1500,500)".into(),
        serde_json::json!({"lyrics":[{"startTimeMs":1000,"durationMs":2000,"words":"Single","parts":[
            {"startTimeMs":1000,"durationMs":500,"words":"Sin"},
            {"startTimeMs":1500,"durationMs":500,"words":"gle"}
        ]}]}).to_string(),
        serde_json::json!({"data":{"format":"ttml","syncType":"richsync","lyrics":ttml}}).to_string(),
    ];
    for source in sources {
        let alt = alternative(&source);
        assert_eq!(alt.sync_type, LyricsSyncType::RichSync, "{source}");
        assert!(has_syllable_level_timing(&alt.lyrics));
        let original = alt.lyrics.clone();
        let document: serde_json::Value =
            serde_json::from_str(&braccato::web_document(&alt, 5000)).unwrap();
        assert_eq!(document["options"]["syncType"], "richsync");
        let line = document["lyrics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|line| line["words"] == "Single")
            .unwrap();
        assert_eq!(line["parts"][0]["startTimeMs"], 1000);
        assert_eq!(line["parts"][0]["durationMs"], 500);
        assert_eq!(line["parts"][1]["startTimeMs"], 1500);
        assert_eq!(line["parts"][1]["durationMs"], 500);
        assert_eq!(alt.lyrics, original);
        let row = braccato::rows(&alt, 5000)
            .into_iter()
            .find(|row| row.text == "Single")
            .unwrap();
        assert_eq!(row.words.row_count(), 2);
        assert_eq!(row.words.row_data(1).unwrap().start_ms, 1500);
    }
}

#[test]
fn background_syllables_and_whitespace_survive_without_estimating_untimed_phrases() {
    let alt = alternative(include_str!("fixtures/syllables.ttml"));
    assert_eq!(alt.lyrics.lines[0].text, "Syllable test");
    let line = braccato::display_lyrics(&alt, 5000)
        .into_iter()
        .find(|line| line.words == "Syllable test")
        .unwrap();
    let parts = line.parts.unwrap();
    let bg = parts
        .iter()
        .filter(|part| part.is_background)
        .collect::<Vec<_>>();
    assert_eq!(bg.len(), 2);
    assert_eq!((bg[0].start_time_ms, bg[0].duration_ms), (2000, 200));
    assert_eq!((bg[1].start_time_ms, bg[1].duration_ms), (2200, 500));
    assert_eq!(
        parts
            .iter()
            .filter(|part| !part.is_background)
            .map(|part| part.words.as_str())
            .collect::<String>(),
        "Syllable test"
    );
    let exported = serialize_lrc(&alt.lyrics);
    let reparsed = alternative(&exported);
    assert!(has_syllable_level_timing(&reparsed.lyrics));
    assert!(reparsed.lyrics.lines[0]
        .parts
        .iter()
        .any(|part| part.is_background && part.start_ms == 2200));
}

#[test]
fn timed_romanization_syllables_retain_their_own_clock() {
    let raw = serde_json::json!({"lyrics":[{"startTimeMs":1000,"durationMs":2000,"words":"你好","parts":[
        {"startTimeMs":1000,"durationMs":500,"words":"你"}, {"startTimeMs":1500,"durationMs":500,"words":"好"}
    ],"romanization":"nihao","timedRomanization":[
        {"startTimeMs":1100,"durationMs":400,"words":"ni"}, {"startTimeMs":1600,"durationMs":600,"words":"hao"}
    ]}]}).to_string();
    let alt = alternative(&raw);
    let rendered = braccato::display_lyrics(&alt, 4000);
    let roman = rendered
        .iter()
        .find(|line| line.words == "你好")
        .unwrap()
        .timed_romanization
        .as_ref()
        .unwrap();
    assert_eq!(
        (roman[0].start_time_ms, roman[1].start_time_ms),
        (1100, 1600)
    );
    assert_eq!((roman[0].duration_ms, roman[1].duration_ms), (400, 600));
}

#[test]
fn malformed_or_simultaneous_fragments_do_not_gain_syllable_sync() {
    for parts in [
        serde_json::json!([{"words":"Sin"},{"words":"gle"}]),
        serde_json::json!([{"words":"Sin","startTimeMs":1000},{"words":"gle","startTimeMs":1000}]),
        serde_json::json!([{"words":"Sin","startTimeMs":1500},{"words":"gle","startTimeMs":1000}]),
        serde_json::json!([{"words":"Wrong","startTimeMs":1000},{"words":"text","startTimeMs":1500}]),
        serde_json::json!([{"words":"Sin","startTimeMs":1000},{"words":"gle"}]),
    ] {
        let raw = serde_json::json!({"lyrics":[{"words":"Single","startTimeMs":1000,"durationMs":2000,"parts":parts}]}).to_string();
        let alt = alternative(&raw);
        assert_eq!(alt.sync_type, LyricsSyncType::Synced, "{raw}");
        assert!(!has_syllable_level_timing(&alt.lyrics));
        let document: serde_json::Value =
            serde_json::from_str(&braccato::web_document(&alt, 4000)).unwrap();
        assert_eq!(document["options"]["syncType"], "synced");
    }
}

#[test]
fn real_background_cues_do_not_invent_lead_or_phrase_timings() {
    let raw = "[00:01.00]Hold [bg:<00:01.500>ye<00:02.000>ah<00:02.500>]\n[00:04.00]Line only";
    let alt = alternative(raw);
    assert_eq!(alt.sync_type, LyricsSyncType::RichSync);
    let display = braccato::display_lyrics(&alt, 6000);
    let line = display.iter().find(|line| line.words == "Hold").unwrap();
    let parts = line.parts.as_ref().unwrap();
    assert!(parts
        .iter()
        .filter(|part| !part.is_background)
        .all(|part| part.duration_ms == 0));
    assert_eq!(
        parts
            .iter()
            .filter(|part| part.is_background)
            .map(|part| part.start_time_ms)
            .collect::<Vec<_>>(),
        [1500, 2000]
    );
    let line_only = display
        .iter()
        .find(|line| line.words == "Line only")
        .unwrap();
    assert!(line_only
        .parts
        .as_ref()
        .unwrap()
        .iter()
        .all(|part| part.duration_ms == 0));
}

#[test]
fn syllable_matching_preserves_non_latin_text_and_source_boundaries() {
    for (first, second) in [("при", "віт"), ("مر", "حبا"), ("こん", "にちは")] {
        let raw = format!("<tt><body><p begin='1s' end='3s'><span begin='1s' end='1.5s'>{first}</span><span begin='1.5s' end='2s'>{second}</span></p></body></tt>");
        let alt = alternative(&raw);
        assert_eq!(alt.sync_type, LyricsSyncType::RichSync);
        let display = braccato::display_lyrics(&alt, 4000);
        let expected = format!("{first}{second}");
        let line = display.iter().find(|line| line.words == expected).unwrap();
        let parts = line.parts.as_ref().unwrap();
        assert_eq!(
            parts
                .iter()
                .map(|part| part.words.as_str())
                .collect::<String>(),
            expected
        );
        assert_eq!(parts[1].start_time_ms, 1500);
    }
}

#[test]
fn provider_selection_prefers_actual_syllables_without_trusting_quality_labels() {
    let word = alternative("[00:01.00]<00:01.00>Hello <00:02.00>world");
    let syllable = alternative(include_str!("fixtures/syllables.ttml"));
    let envelope = serde_json::json!({"provider":"golyrics","results":{"lyrics":include_str!("fixtures/syllables.ttml")}});
    let from_sse = parse_provider_sse(&format!("event: provider\ndata: {envelope}\n\n"));
    assert_eq!(from_sse.len(), 1);
    assert!(has_syllable_level_timing(&from_sse[0].lyrics));
    let mut alternatives = vec![word, syllable];
    braccato::providers::normalize_alternatives(&mut alternatives);
    assert!(has_syllable_level_timing(&alternatives[0].lyrics));
}
