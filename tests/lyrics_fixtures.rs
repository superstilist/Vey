use playervf::lyrics::*;
use slint::Model;

#[test]
fn rendering_preserves_real_cues_and_never_times_backing_phrases() {
    let (lyrics, format, sync_type) = parse_payload(
        "[00:01.00]<00:01.00>First <00:02.00>word [bg:(some backing words)]\n[00:04.00]Line only",
    )
    .unwrap();
    let alt = LyricsAlternative {
        provider: "test".into(),
        format: format.into(),
        sync_type,
        lyrics,
    };
    let original = alt.lyrics.clone();
    let display = playervf::braccato::display_lyrics(&alt, 6000);
    let lead = &display
        .iter()
        .find(|line| line.words == "First word")
        .unwrap()
        .parts
        .as_ref()
        .unwrap();
    assert_eq!((lead[0].start_time_ms, lead[0].duration_ms), (1000, 1000));
    assert_eq!((lead[1].start_time_ms, lead[1].duration_ms), (2000, 0));
    let backing = lead.iter().find(|part| part.is_background).unwrap();
    assert_eq!((backing.start_time_ms, backing.duration_ms), (1000, 0));
    assert_eq!(alt.lyrics, original);
}

#[test]
fn widget_mode_preserves_zero_origin_and_start_only_word_cues() {
    for (raw, expected) in [
        ("[00:00.00]First line", "synced"),
        ("plain lyrics", "none"),
        (
            r#"[{"startTimeMs":0,"words":"First word","parts":[{"startTimeMs":0,"words":"First "},{"startTimeMs":1000,"words":"word"}]}]"#,
            "richsync",
        ),
    ] {
        let (lyrics, format, sync_type) = parse_payload(raw).unwrap();
        let alt = LyricsAlternative {
            provider: "test".into(),
            format: format.into(),
            sync_type,
            lyrics,
        };
        let document: serde_json::Value =
            serde_json::from_str(&playervf::braccato::web_document(&alt, 2000)).unwrap();
        assert_eq!(document["options"]["syncType"], expected, "{raw}");
    }
}

#[test]
fn explicit_ttml_line_end_is_not_extended_to_fit_word_parts() {
    let parsed = parse_ttml("<tt><body><p begin='1s' end='3s'><span begin='1s' end='2s'>First </span><span begin='2s' end='4s'>word</span></p></body></tt>");
    assert_eq!(parsed.lines[0].duration_ms, 2000);
    assert_eq!(parsed.lines[0].parts[1].duration_ms, 2000);
}
#[test]
fn messy_ttml_wrappers_and_escaped_spaces_never_become_lyric_text() {
    let xml = "<tt><body><p begin='1s' end='3s'><span begin='1s' end='2s'>First</span>&#x20;<span begin='2s' end='3s'>line</span></p></body></tt>";
    let expected = parse_payload(xml).unwrap();
    assert_eq!(expected.0.lines[0].text, "First line");
    for input in [
        format!("````sql\nxml ```\n{xml}\n```\n````"),
        format!("xml ``` {xml} ```"),
        format!("Here is the XML:\n```xml\n{xml}\n```\nEnd of response."),
        format!("```sql\n{xml}"),
        format!("xml\n{xml}\n```"),
        html_escape::encode_text(xml).into_owned(),
        xml.replace("&#x20;", "&amp;#x20;"),
        xml.replace("&#x20;", "&amp;#32;"),
        xml.replace("&#x20;", "&#x20;\n    "),
    ] {
        let parsed = parse_payload(&input).unwrap();
        assert_eq!(parsed, expected, "input: {input}");
        let json = serde_json::json!({"response":{"xml":input}}).to_string();
        assert_eq!(parse_payload(&json).unwrap(), expected);
    }
    let literal = "<tt><body><p begin='1s' end='2s'>A &lt; B &amp; C</p></body></tt>";
    assert_eq!(parse_payload(literal).unwrap().0.lines[0].text, "A < B & C");
    let bad = "xml ``` <tt><body><p begin='1s'>broken</body></tt> ```";
    assert!(parse_payload(bad).is_none());
}

#[test]
fn fenced_composer_ttml_preserves_line_timing_agents_and_metadata() {
    let raw = include_str!("fixtures/fenced-line-ttml.txt");
    let (lyrics, format, sync) = parse_payload(raw).unwrap();
    assert_eq!((format, sync), ("ttml", LyricsSyncType::Synced));
    assert_eq!(lyrics.lines.len(), 2);
    assert_eq!(lyrics.lines[0].text, "First line");
    assert_eq!(
        (lyrics.lines[0].start_ms, lyrics.lines[0].duration_ms),
        (5684, 2388)
    );
    assert_eq!(
        (lyrics.lines[1].start_ms, lyrics.lines[1].duration_ms),
        (63824, 6974)
    );
    assert_eq!(lyrics.lines[0].agent, "v1");
    assert_eq!(lyrics.title.as_deref(), Some("Fixture title"));
    assert_eq!(lyrics.artist.as_deref(), Some("First & Second"));
    assert_eq!(lyrics.album.as_deref(), Some("Fixture album"));
    let nested = serde_json::json!({"response":{"xml":raw},"language":"en"}).to_string();
    let encoded = serde_json::to_string(&nested).unwrap();
    let wrapped = format!(" \u{feff} ```json\r\n{encoded}\r\n``` ");
    let (restored, _, _) = parse_payload(&wrapped).unwrap();
    assert_eq!(restored.lines, lyrics.lines);
    assert_eq!(restored.language.as_deref(), Some("en"));
    let event = serde_json::json!({"provider":"fixture","results":{"xml":raw}});
    let alternatives = parse_provider_sse(&format!("data: {event}\n\n"));
    assert_eq!(alternatives.len(), 1);
    assert_eq!(alternatives[0].format, "ttml");
    assert_eq!(alternatives[0].lyrics, lyrics);
}

#[test]
fn fenced_lyric_formats_use_content_detection_and_reject_broken_xml() {
    for (label, content, expected) in [
        ("lrc", "[00:01.00]First line", "lrc"),
        ("qrc", "[1000,2000]First(1000,2000)", "qrc"),
        ("srt", "1\n00:00:01,000 --> 00:00:03,000\nFirst line", "srt"),
        (
            "vtt",
            "WEBVTT\n\n00:01.000 --> 00:03.000\nFirst line",
            "vtt",
        ),
        ("text", "First line\nNext line", "txt"),
        ("json", r#"{"syncedLyrics":"[00:01.00]First line"}"#, "lrc"),
    ] {
        for fence in ["```", "~~~~"] {
            let raw = format!("{fence}{label}\n{content}\n{fence}");
            let (parsed, format, _) = parse_payload(&raw).unwrap();
            assert_eq!(format, expected);
            assert!(!parsed.lines[0].text.contains(fence));
        }
    }
    for raw in [
        "```xml\n<tt invalid\n```",
        "```xml\n<tt/>",
        "~~~xml\n<tt/>\n```",
        "```xml\n<error>unavailable</error>\n```",
        "```\n\n```",
    ] {
        assert!(parse_payload(raw).is_none(), "must reject {raw:?}");
    }
    assert_eq!(
        parse_payload("A literal ``` marker").unwrap().0.lines[0].text,
        "A literal ``` marker"
    );
}

#[test]
fn enhanced_and_background_fixture() {
    let l = parse_lrc(include_str!("fixtures/enhanced.lrc"));
    assert_eq!(l.lines[0].start_ms, 1250);
    assert_eq!(l.lines[0].text, "Перший рядок");
    assert!(l.lines[0].parts[2].is_background);
    assert_eq!(l.lines[0].parts[2].start_ms, 1850);
}
#[test]
fn translations_fixture() {
    let l = parse_lrc(include_str!("fixtures/translations.lrc"));
    assert_eq!(l.lines.len(), 2);
    assert_eq!(l.lines[0].text, "Original");
    assert_eq!(l.lines[0].translation, "Переклад");
}
#[test]
fn nested_ttml_does_not_duplicate_background() {
    let l = parse_ttml(include_str!("fixtures/nested.ttml"));
    assert_eq!(l.lines[0].text, "Lead");
    assert_eq!(l.lines[0].parts.len(), 2);
    assert_eq!(l.lines[0].translation, "Переклад");
    assert!(l.lines[0].parts[1].is_background);
}
#[test]
fn malformed_provider_does_not_hide_fallback() {
    let l=parse_provider_sse("data: broken\n\ndata: {\"provider\":\"bad\",\"results\":{\"lyrics\":\"<tt invalid\"}}\n\ndata: {\"provider\":\"lrclib\",\"results\":{\"synced\":\"[00:01.00]Good\"}}\n\n");
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].provider, "lrclib");
}
#[test]
fn nested_json_ttml_provider() {
    let raw = serde_json::json!({"ttml":include_str!("fixtures/nested.ttml")}).to_string();
    let sse = format!(
        "data: {}\n",
        serde_json::json!({"provider":"test","results":{"lyrics":raw}})
    );
    let l = parse_provider_sse(&sse);
    assert_eq!(l[0].lyrics.lines[0].text, "Lead");
}

#[test]
fn lrc_export_roundtrip_keeps_words_background_and_translation() {
    let mut original = parse_lrc(include_str!("fixtures/enhanced.lrc"));
    original.lines[0].translation = "Переклад".into();
    let exported = serialize_lrc(&original);
    let restored = parse_lrc(&exported);
    assert_eq!(restored.lines[0].text, original.lines[0].text);
    assert_eq!(restored.lines[0].translation, "Переклад");
    assert_eq!(
        restored.lines[0]
            .parts
            .iter()
            .map(|p| (&p.text, p.start_ms, p.is_background))
            .collect::<Vec<_>>(),
        original.lines[0]
            .parts
            .iter()
            .map(|p| (&p.text, p.start_ms, p.is_background))
            .collect::<Vec<_>>()
    );
}

#[test]
fn qrc_word_times_are_absolute_even_after_nonzero_line_start() {
    let (l, format, sync) =
        parse_payload("[10000,2000]Hello (10000,900)world(10900,1100)").unwrap();
    assert_eq!(format, "qrc");
    assert_eq!(sync, LyricsSyncType::RichSync);
    assert_eq!(l.lines[0].parts[0].start_ms, 10000);
    assert_eq!(l.lines[0].parts[1].start_ms, 10900);
}

#[test]
fn metadata_does_not_hide_qrc_detection() {
    let (l, f, _) = parse_payload("[ti:Song]\n[10000,2000]Hello(10000,2000)").unwrap();
    assert_eq!(f, "qrc");
    assert_eq!(l.title.as_deref(), Some("Song"));
}

#[test]
fn enhanced_lrc_terminal_marker_keeps_silence_between_lines() {
    let (l, _, sync) =
        parse_payload("[00:10.00]<00:10.00>Hello <00:11.00>world<00:12.00>\n[00:20.00]Next")
            .unwrap();
    assert_eq!(sync, LyricsSyncType::Word);
    assert_eq!(l.lines[0].duration_ms, 2000);
    assert_eq!(l.lines[0].parts[1].duration_ms, 1000);
}

#[test]
fn repeated_enhanced_lrc_moves_word_times_with_line() {
    let l = parse_lrc("[00:10.00][00:20.00]<00:10.00>Hello <00:11.00>world<00:12.00>");
    assert_eq!(l.lines[1].parts[0].start_ms, 20000);
    assert_eq!(l.lines[1].parts[1].start_ms, 21000);
    assert_eq!(l.lines[1].duration_ms, 2000);
}

#[test]
fn parses_srt_cues_and_plain_text_without_fake_word_sync() {
    let (l, f, sync) = parse_payload("1\r\n00:00:01,500 --> 00:00:03,000\r\nHello\r\nWorld\r\n\r\n2\r\n00:00:04,000 --> 00:00:05,000\r\nNext").unwrap();
    assert_eq!(f, "srt");
    assert_eq!(sync, LyricsSyncType::Synced);
    assert_eq!(l.lines[0].text, "Hello\nWorld");
    assert_eq!(l.lines[0].duration_ms, 1500);
    assert_eq!(
        parse_payload("Hello\nWorld").unwrap().2,
        LyricsSyncType::Plain
    );
}

#[test]
fn line_ttml_and_zero_time_lrc_are_line_synced() {
    assert_eq!(
        parse_payload("<tt><body><p begin='0s' end='2s'>Hello</p></body></tt>")
            .unwrap()
            .2,
        LyricsSyncType::Synced
    );
    assert_eq!(
        parse_payload("[00:00:00]Hello").unwrap().2,
        LyricsSyncType::Synced
    );
    assert_eq!(
        parse_payload("<tt><body><p>Hello without timestamps</p></body></tt>")
            .unwrap()
            .2,
        LyricsSyncType::Plain
    );
}

#[test]
fn real_word_cues_are_required_for_word_sync_across_formats() {
    for (raw, expected) in [
        ("[00:01.00]<00:01.00>Hello <00:02.00>world", LyricsSyncType::Word),
        ("[1000,2000]Hello (1000,900)world(1900,1100)", LyricsSyncType::RichSync),
        ("<tt><body><p begin='1s' end='3s'><span begin='1s' end='2s'>Hello </span><span begin='2s' end='3s'>world</span></p></body></tt>", LyricsSyncType::RichSync),
    ] {
        assert_eq!(parse_payload(raw).unwrap().2, expected, "{raw}");
    }
    for raw in [
        "[00:01.00]Hello world",
        "[1000,2000]Hello world",
        "<tt><body><p begin='1s' end='3s'><span begin='1s' end='3s'>Hello </span><span begin='1s' end='3s'>world</span></p></body></tt>",
        "One line without timestamps",
    ] {
        let (_, _, actual) = parse_payload(raw).unwrap();
        assert!(matches!(actual, LyricsSyncType::Synced | LyricsSyncType::Plain), "{raw}: {actual:?}");
    }
}

#[test]
fn qrc_chinese_word_cues_are_not_rejected_as_single_whitespace_token() {
    let raw = "[1000,3000]你好世界(1000,500)美丽(1500,700)歌词(2300,900)";
    let (lyrics, format, sync_type) = parse_payload(raw).unwrap();
    assert_eq!(format, "qrc");
    assert_eq!(sync_type, LyricsSyncType::RichSync);
    assert_eq!(lyrics.lines[0].text, "你好世界美丽歌词");
    assert_eq!(
        lyrics.lines[0]
            .parts
            .iter()
            .map(|part| (part.text.as_str(), part.start_ms, part.duration_ms))
            .collect::<Vec<_>>(),
        [
            ("你好世界", 1000, 500),
            ("美丽", 1500, 700),
            ("歌词", 2300, 900)
        ]
    );

    let alternative = LyricsAlternative {
        provider: "qq".into(),
        format: format.into(),
        sync_type,
        lyrics,
    };
    let rows = playervf::braccato::rows(&alternative, 5000);
    let lyric_row = rows
        .iter()
        .find(|row| row.text == "你好世界美丽歌词")
        .unwrap();
    assert_eq!(lyric_row.words.row_count(), 3);
    let rendered = playervf::braccato::web_document(&alternative, 5000);
    let document: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(document["options"]["syncType"], "richsync");
    assert_eq!(
        document["lyrics"][0]["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| part["startTimeMs"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [1000, 1500, 2300]
    );
}

#[test]
fn line_only_qrc_does_not_become_word_synced_from_envelope_timing() {
    let (lyrics, format, sync_type) = parse_payload("[1000,3000]你好世界").unwrap();
    assert_eq!(format, "qrc");
    assert_eq!(sync_type, LyricsSyncType::Synced);
    assert!(!has_word_level_timing(&lyrics));
    let alternative = LyricsAlternative {
        provider: "qq".into(),
        format: format.into(),
        sync_type,
        lyrics,
    };
    assert_eq!(
        playervf::braccato::effective_sync_type(&alternative),
        LyricsSyncType::Synced
    );
    assert_eq!(
        playervf::braccato::rows(&alternative, 5000)[0]
            .words
            .row_count(),
        0
    );
}

#[test]
fn nested_qrc_xml_and_broken_xml_provider_fallback() {
    let raw = serde_json::json!({"lyrics":"<QrcInfos><Lyric_1 LyricContent='[10000,2000]Hello(10000,1000)world(11000,1000)'/></QrcInfos>"}).to_string();
    assert_eq!(
        parse_payload(&raw).unwrap().0.lines[0].parts[1].start_ms,
        11000
    );
    assert!(parse_payload("<tt invalid").is_none());
    assert!(parse_payload("<QrcInfos invalid").is_none());
}

#[test]
fn backing_vocals_do_not_upgrade_line_lrc_to_word_sync() {
    assert_eq!(
        parse_payload("[00:01.00]Hello [bg:(hey)]\n[00:02.00]Next")
            .unwrap()
            .2,
        LyricsSyncType::Synced
    );
}

#[test]
fn qrc_preserves_literal_parentheses_and_untimed_tail() {
    let l = parse_qrc("[10000,3000]Hello (again)(10000,1500) world(11500,1000)!");
    assert_eq!(l.lines[0].text, "Hello (again) world!");
    assert_eq!(l.lines[0].parts[0].text, "Hello (again)");
    assert_eq!(l.lines[0].parts[1].start_ms, 11500);
    assert_eq!(l.lines[0].parts[1].duration_ms, 1000);
    assert!(l.lines[0].parts[1].text.ends_with("!"));
}

#[test]
fn final_lrc_word_uses_song_end_without_filling_explicit_singing_gaps() {
    let mut l =
        parse_lrc("[00:01.00]<00:01.00>Hello<00:02.00>\n[00:05.00]<00:05.00>Last <00:06.00>word");
    complete_timing(&mut l, 8000);
    assert_eq!(l.lines[0].duration_ms, 1000);
    assert_eq!(l.lines[1].duration_ms, 3000);
    assert_eq!(l.lines[1].parts[1].duration_ms, 0);
}

#[test]
fn explicit_word_cues_keep_granularity_and_line_cues_stay_line_synced() {
    assert_eq!(
        parse_payload("[00:10.00]<00:10.00>Hello<00:12.00>")
            .unwrap()
            .2,
        LyricsSyncType::Synced
    );
    assert_eq!(
        parse_payload("[10000,2000]Hello(10000,2000)").unwrap().2,
        LyricsSyncType::Synced
    );
    assert_eq!(parse_payload("<tt><body><p begin='10s' end='12s'><span begin='10s' end='12s'>Hello</span></p></body></tt>").unwrap().2, LyricsSyncType::Synced);
}

#[test]
fn braccato_json_models_and_nested_envelopes_preserve_all_fields() {
    let source=serde_json::json!({"data":{"lyrics":[{"startTimeMs":1000,"durationMs":2000,"words":"Hello world","agent":"v2","parts":[{"startTimeMs":1000,"durationMs":900,"words":"Hello","explicit":true},{"startTimeMs":1900,"durationMs":1100,"words":"world"}],"translation":{"text":"Привіт","lang":"uk"},"romanization":"Pryvit"}]}}).to_string();
    let (parsed, format, sync) = parse_payload(&source).unwrap();
    assert_eq!(format, "json");
    assert_eq!(sync, LyricsSyncType::RichSync);
    assert_eq!(parsed.lines[0].translation, "Привіт");
    assert_eq!(parsed.lines[0].translations["uk"], "Привіт");
    assert!(parsed.lines[0].parts[0].explicit);
    assert_eq!(parsed.lines[0].agent, "v2");
    let mut deep = source;
    for _ in 0..10 {
        deep = serde_json::json!({"lyrics":deep}).to_string();
    }
    assert!(parse_payload(&deep).is_none());
}

#[test]
fn json_part_times_without_distinct_word_cues_remain_line_synced() {
    let source = serde_json::json!({"lyrics":[{"words":"One line","parts":[{"words":"One ","startTimeMs":0,"durationMs":2000},{"words":"line","startTimeMs":0,"durationMs":2000}]}]}).to_string();
    let (parsed, format, sync) = parse_payload(&source).unwrap();
    assert_eq!(format, "json");
    assert_eq!(sync, LyricsSyncType::Synced);
    assert_eq!(
        playervf::braccato::effective_sync_type(&playervf::lyrics::LyricsAlternative {
            provider: "fixture".into(),
            format: format.into(),
            lyrics: parsed,
            sync_type: sync,
        }),
        LyricsSyncType::Synced
    );
}

#[test]
fn zero_origin_json_lines_keep_sync_and_real_syllable_parts_use_rich_sync() {
    let line = serde_json::json!({
        "lyrics": [{"startTimeMs": 0, "durationMs": 0, "words": "At the start"}]
    })
    .to_string();
    assert_eq!(parse_payload(&line).unwrap().2, LyricsSyncType::Synced);

    let syllables = serde_json::json!({
        "lyrics": [{
            "startTimeMs": 1000,
            "durationMs": 2000,
            "words": "single",
            "parts": [
                {"startTimeMs": 1000, "durationMs": 500, "words": "sin"},
                {"startTimeMs": 1500, "durationMs": 500, "words": "gle"}
            ]
        }]
    })
    .to_string();
    let (parsed, _, sync) = parse_payload(&syllables).unwrap();
    assert_eq!(sync, LyricsSyncType::RichSync);
    assert!(has_syllable_level_timing(&parsed));
    assert!(!has_word_level_timing(&parsed));
}

#[test]
fn one_inline_lrc_word_cue_does_not_upgrade_a_line_to_word_sync() {
    let (lyrics, _, sync) = parse_payload("[00:10.00]first <00:11.00>second").unwrap();
    assert_eq!(sync, LyricsSyncType::Synced);
    assert!(!playervf::lyrics::has_word_level_timing(&lyrics));
}

#[test]
fn qrc_untimed_envelope_keeps_text_instead_of_upstream_empty_line() {
    let result = playervf::braccato::parsers::parse(
        "<QrcInfos><Lyric_1 LyricContent='[1000,2000]Hello world'/></QrcInfos>",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        result
            .lyrics
            .iter()
            .find(|l| !l.is_instrumental)
            .unwrap()
            .words,
        "Hello world"
    );
}
