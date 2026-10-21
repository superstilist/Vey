# YouTube Music catalog parsing in Vey

Vey embeds the user-supplied yt-rust 0.1.0 source under
`third-party/yt-rust/` as a relative Cargo path dependency. Provenance and local
decoder changes are recorded in [YT-RUST-UPSTREAM](../third-party/YT-RUST-UPSTREAM.md).
The sibling checkout is not required to build or run Vey.

## Runtime path

Native search and the browser `/api/search` endpoint both call
`src/native.rs::ytm_search_songs`. Its public InnerTube requests retain the
existing homepage context, filters and bounded continuation fetching. Response
JSON is decoded by yt-rust and adapted back to Vey's existing YtmSong/Track
models, so audio selection, lyric matching and persistence retain their IDs.

The normalized decoder supplies linked artist/album metadata, title, duration
and artwork, including title-endpoint rows missing the previous overlay shape.
View counters and subtitle clocks are not treated as album names.
The host adapter retains ordering, high-resolution cover selection, availability
and video provenance from layouts it already supports. It also retains card,
carousel and other response variants absent from yt-rust. Direct cards and
bare shelf continuations are also normalized by the copied decoder.
Album/artist/playlist catalog IDs are not introduced as playable song IDs. Search accepts the callers'
requested limits up to 100, rather than silently reducing every request to 15.

Suggestions use yt-rust for normalized query text and deduplication while the
host preserves bold runs and history metadata where present. A suggestion can
use displayed text when its query endpoint is omitted. Empty text is excluded.

No account credentials are added. The integration uses the JSON decoders rather
than the library's HTTP client or optional session support. Vey continues to own
public browse/radio, clip matching, media resolution, audio decoding and playback;
yt-rust itself does not supply playable media URLs. Existing public detail-page
gaps remain separate from this parser integration.

If every search request fails, the service now returns an error instead of a
successful empty result. Supplemental clip lookups are capped at three attempts,
including unsuccessful ones. The existing audio/video pairing remains separate
from normalized metadata so a song is not mislabeled as a playable music video.

## Checks

The root fixture `tests/fixtures/yt-rust-search.json` covers a combined subtitle
with two linked artists, an album and a clock, fixed-column duration precedence,
multiple artwork sizes, an unavailable row and a nonplayable catalog album.
Adapter tests also check preservation of host clip/art hints, suggestion fallback
and zero limits. The existing parser/clip tests remain regression coverage.

Commands from the Vey root:

```powershell
cargo test --locked --lib --tests
cargo check --locked --lib --target wasm32-unknown-unknown
cargo test --manifest-path third-party/yt-rust/Cargo.toml --all-features --target-dir target/yt-rust
cargo run --locked --example yt_music_check -- "oasis wonderwall"
```

The example uses the actual Vey search service and prints normalized track
metadata. Add `--audio` to separately attempt audio resolution/cache download;
this writes only the existing temporary media cache and does not establish
audible device playback. A successful catalog response does not prove a media
server accepts playback. Network checks are opt-in and not part of ordinary
fixture tests. Windows/browser build evidence does not establish Android/Linux
runtime acceptance. Actual results for this change are recorded below.

## Recorded verification — 2026-10-21

- Root Rust tests: 183 passed, seven environment-dependent checks ignored.
  Includes four new adapter regressions and the existing search/clip tests.
- Copied library, all features: 14 tests passed; doc tests completed with no cases.
- Native debug build and WASM library compile check passed. Formatting checks
  passed for the changed Rust files, preserving the native source's CRLF style.
- The rebuilt Vey backend returned 40 playable tracks through `/api/search`
  for the public query `oasis wonderwall`. Checked track keys/video IDs and
  verified that view counts and clocks did not populate album fields. Artwork,
  artists, real album names and available durations reached the API response.
- The separate example audio check resolved a media format but failed during
  ranged download with HTTP 403, including after the existing fresh-URL retry.
  This reproduces I-01; catalog integration does not resolve that playback issue.
  No audible playback, GUI interaction or Android/Linux runtime pass is claimed.

Subsequent playback fix: both reported media IDs now download and decode fully
with the public visionOS player context. See [playback recovery](PLAYBACK-RECOVERY.md)
for the later evidence; the catalog checks above describe the earlier snapshot.

The verification backend used isolated state under target and was stopped after
the requests. Diagnostic output remains under ignored target files; no credentials
or media URLs are included in this report. The supplied sibling source hash
still matches the original snapshot.
