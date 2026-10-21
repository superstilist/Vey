# Lyric input normalization — 0.1.11

The reported Composer TTML document was enclosed in a Markdown `xml` fence.
The old shared detector saw backticks and treated the entire response as plain
lyrics. Paste, migration, native/Android and WASM all use `parse_payload`.

Complete backtick and tilde fences now unwrap before content detection. JSON
strings/envelopes can wrap fenced documents, and fenced JSON can wrap XML.
Unwrapping is bounded by the existing recursion limit. Other fenced formats
require complete matching fences; empty documents and malformed/unknown XML
are rejected. Ordinary lyric
text containing an inline backtick marker is preserved. Fence labels are hints;
the underlying content still determines format and timing classification.

Since 0.1.11, complete TTML can also be recovered from surrounding prose,
standalone `xml` markers, nested or mislabeled (`sql`) fences and incomplete
outer fences. Only the root document is parsed; surrounding debris is dropped.
HTML-escaped XML transport is decoded in bounded passes, while valid XML is
never globally decoded again. Numeric `&#x20;` / `&#32;` space artifacts in
TTML text, including double-escaped forms, become spaces. Explicit spaces
between spans survive formatting newlines; ordinary indentation is ignored.

0.1.11 validation (2026-10-20): all 160 Rust tests passed, seven environment
checks ignored. The messy-wrapper regression compares full parsed output,
including word timing, against the clean document. It covers nested `sql`/`xml`
fences, inline markers, surrounding prose, missing outer closing fences,
HTML-escaped transport, numeric and double-escaped spaces, indentation between
spans, nested JSON and malformed XML. Provider response tests retain multiple
named alternatives and also exercise the messy TTML input.

Known envelope fields additionally include `xml`, `lrc`, `qrc`, `srt`, `vtt`,
`syncedLyrics` and `plainLyrics`. Provider SSE/direct JSON use the same fields;
outer fences and JSON strings preserve named provider alternatives. Composer
`ttm:title` and `meta` artist(s)/album entries retain metadata and XML entities.
Line-synchronized TTML and QRC remain line-level: the UI highlights and scrolls
the full line without inventing a word sweep. Genuine per-word cues retain their
word-level animation, and provider ranking continues to prefer those alternatives.
The embedded renderer's synthetic line-synced word delay is set to zero so its
fallback text appears together at the line cue.

Regression coverage: `cargo test --locked --lib --tests`, especially
`tests/lyrics_fixtures.rs` and the response-decoding test in `lyrics_service`.
`tests/fixtures/fenced-line-ttml.txt` is a short synthetic reproduction of the
reported namespaces, metadata and clock syntax, without the full song text.

2026-10-20: the full native Rust suite passed (161 persistent tests, seven
environment-dependent tests ignored). A temporary local test also checked the
exact supplied attachment: all 46 line cues, first cue at 5684 ms for 2388 ms,
last cue at 150521 ms for 3500 ms, metadata, clean lyric text and nested-JSON
round trip passed. The temporary test and the full song were not retained in
the repository. No live provider service or physical Android run was tested.

The debug WASM build and optimized Windows/Android builds passed. Version
0.1.11 ZIP CRC and SHA-256 checks passed; Android signature, 16 KiB alignment,
manifest and SHA-256 checks passed. Artifact hashes and sizes are recorded in
`lyric-input-check.json`; packages are under `dist/`.
