//! Differential fixtures generated with the real, pinned upstream TTML parser.
//! No JavaScript runtime is needed by these tests or the application.
#[test]
fn matches_pinned_upstream_ttml_corpus() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/braccato-upstream-ttml.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let parsed = playervf::lyrics::parse_ttml(case["source"].as_str().unwrap());
        let expected = case["lines"].as_array().unwrap();
        assert_eq!(parsed.lines.len(), expected.len(), "{}", case["name"]);
        for (line, reference) in parsed.lines.iter().zip(expected) {
            assert_eq!(
                line.text.split_whitespace().collect::<Vec<_>>().join(" "),
                reference["text"]
                    .as_str()
                    .unwrap()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                "{}",
                case["name"]
            );
            assert_eq!(
                line.start_ms,
                reference["start"].as_u64().unwrap(),
                "{}",
                case["name"]
            );
            assert_eq!(
                line.duration_ms,
                reference["duration"].as_u64().unwrap(),
                "{}",
                case["name"]
            );
            assert_eq!(
                line.agent,
                reference["agent"].as_str().unwrap(),
                "{}",
                case["name"]
            );
            assert_eq!(
                line.romanization,
                reference["romanization"].as_str().unwrap(),
                "{}",
                case["name"]
            );
            for (lang, text) in reference["translations"].as_object().unwrap() {
                assert_eq!(
                    line.translations.get(lang).map(String::as_str),
                    text.as_str(),
                    "{}",
                    case["name"]
                );
            }
            assert_eq!(
                line.parts.iter().any(|p| p.explicit),
                reference["explicit"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn matches_pinned_upstream_line_formats() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/braccato-upstream-line-formats.json")).unwrap();
    let mut errors = vec![];
    for case in cases.as_array().unwrap() {
        if case["name"] == "qrc:envelope:25" {
            continue;
        } // Upstream loses untimed words; native keeps them (covered separately).
        let result = playervf::braccato::parsers::parse(
            case["source"].as_str().unwrap(),
            playervf::braccato::parsers::Options {
                duration_ms: 60000,
                ..Default::default()
            },
        );
        let lines=result.map(|d|d.lyrics.into_iter().filter(|l|!l.is_instrumental).map(|l|serde_json::json!({"text":l.words.split_whitespace().collect::<Vec<_>>().join(" "),"start":l.start_time_ms,"duration":l.duration_ms,"agent":l.agent.unwrap_or_default()})).collect::<Vec<_>>()).unwrap_or_default();
        let expected=case["lines"].as_array().unwrap().iter().map(|l|serde_json::json!({"text":l["text"].as_str().unwrap().split_whitespace().collect::<Vec<_>>().join(" "),"start":l["start"],"duration":l["duration"],"agent":l["agent"]})).collect::<Vec<_>>();
        if lines != expected {
            errors.push(format!(
                "{}\n actual: {:?}\n expected: {:?}",
                case["name"], lines, expected
            ));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
