use playervf::lyrics::*;
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
    assert_eq!(l.lines[0].parts[2].start_ms, 12500);
    assert_eq!(l.lines[0].parts[2].duration_ms, 500);
}

#[test]
fn final_lrc_word_uses_song_end_without_filling_explicit_singing_gaps() {
    let mut l =
        parse_lrc("[00:01.00]<00:01.00>Hello<00:02.00>\n[00:05.00]<00:05.00>Last <00:06.00>word");
    complete_timing(&mut l, 8000);
    assert_eq!(l.lines[0].duration_ms, 1000);
    assert_eq!(l.lines[1].duration_ms, 3000);
    assert_eq!(l.lines[1].parts[1].duration_ms, 2000);
}

#[test]
fn single_word_cues_keep_explicit_word_granularity() {
    assert_eq!(
        parse_payload("[00:10.00]<00:10.00>Hello<00:12.00>")
            .unwrap()
            .2,
        LyricsSyncType::Word
    );
    assert_eq!(
        parse_payload("[10000,2000]Hello(10000,2000)").unwrap().2,
        LyricsSyncType::RichSync
    );
    assert_eq!(parse_payload("<tt><body><p begin='10s' end='12s'><span begin='10s' end='12s'>Hello</span></p></body></tt>").unwrap().2, LyricsSyncType::RichSync);
}

#[test]
fn braccato_json_models_and_nested_envelopes_preserve_all_fields() {
    let source=serde_json::json!({"data":{"lyrics":[{"startTimeMs":1000,"durationMs":2000,"words":"Hello","agent":"v2","parts":[{"startTimeMs":1000,"durationMs":2000,"words":"Hello","explicit":true}],"translation":{"text":"Привіт","lang":"uk"},"romanization":"Pryvit"}]}}).to_string();
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
