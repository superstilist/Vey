# Windows host and public audio recovery

## Changes

- Slint can report a visible window before its native HWND is available.
  The Braccato and video WebView2 hosts now check the raw handle and defer
  construction until it is Win32. Handle readiness does not consume the
  five-second retry interval reserved for real WebView initialization failures.
- Public audio resolution prefers visionOS 1.04 with its device, OS, visitor
  and User-Agent context. WEB_REMIX and Android remain fallbacks. Responses
  identifying another video are rejected. No account session is used.
- Download requests use the selected client's User-Agent. Known-size ranges
  must match their Content-Range and byte length. Failed/cancelled downloads
  abort outstanding range tasks. Cache size is checked against the stream's
  advertised length when available; complete files are published by rename.
- A new native selection cancels the previous load future. Repeated clicks on
  the same loading track are ignored. Completion requires the current load
  generation as well as the track key, covering A -> B -> A selection races.

Client reference: [NewPipeExtractor client constants](https://github.com/TeamNewPipe/NewPipeExtractor/blob/dev/extractor/src/main/java/org/schabi/newpipe/extractor/services/youtube/ClientsConstants.java)
and [player request context](https://github.com/TeamNewPipe/NewPipeExtractor/blob/dev/extractor/src/main/java/org/schabi/newpipe/extractor/services/youtube/YoutubeStreamHelper.java).
The [yt-dlp token guide](https://github.com/yt-dlp/yt-dlp/wiki/PO-Token-Guide)
documents why an OK Android player response does not establish media access.

## Live diagnostics — 2026-10-21

Before the change, kOn-HdEg6AQ returned 206 for bytes 0–524287 and
524288–1048575, then 403 for 1048576–1572863, including sequential requests.
Refreshing its URL did not resolve the rejection. A visionOS request returned
206 beyond that boundary using the same local network.

The actual Vey cache service and rodio decoder then produced:

| Video ID | Complete bytes | Decoded duration | Catalog duration |
| --- | ---: | ---: | ---: |
| kOn-HdEg6AQ | 4,185,103 | 258.44 s | 258 s |
| xukbqwRuN5w | 3,752,751 | 231.80 s | 232 s |

Each file downloaded in eight ranges without a 403 or URL refresh. Reproduction:

```powershell
cargo run --locked --example stream_probe -- --cache kOn-HdEg6AQ xukbqwRuN5w
```

The diagnostic prints sizes, durations and HTTP ranges, never signed URLs.
Cached entries can be reused on subsequent runs. Full decoding establishes
media integrity; it does not establish audible device playback.

A normal launch of the rebuilt Windows app with isolated state logged
`[braccato] WebView2 initialized` without the previous handle error.
Native debug build, WASM library compile check and formatting of the affected
formatted Rust modules passed. The diagnostic app process was stopped afterward.
Runtime checks here cover
these Windows/media paths; Android/Linux acceptance and broader catalog
availability are separate.
