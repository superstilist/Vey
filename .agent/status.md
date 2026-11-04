# Capabilities and project status

Release update: 2026-11-03. Version in `Cargo.toml`: **0.1.47**.
Older product/platform reports below retain their original scope and date.
Implemented does not mean verified on every target.

### Home discovery follow-up (working tree, 2026-11-03)

Home now uses mixed media shelves with real browse/video targets, local and
private recommendation data, cached public metadata and the existing artwork
services. The default desktop dock is flat; all six Home choices and localized
picker diagrams match the new composition. Windows debug/native fixtures, WASM
build, browser Home scenarios and a live public catalog/playlist probe pass;
the library run passed 246 tests with 7 ignored. See `docs/HOME-DISCOVERY.md`
for exact scope. Windows x64 package `0.1.47` is built below; other platform
packages and a GitHub publication remain separate.

Local Windows x64 package: `dist/Vey-0.1.47-windows-x64.zip`, SHA-256
`bd66bc32413f7ed6a92c6e35f596887ea8074188f0c3630895ac22b3f31a8a7d`.
The archive and sidecar match, and the packaged executable matches
`target/release/vey.exe`.

### Mobile playlist view, library count, YouTube Music file identity and cover caching (0.1.46, 2026-11-02)

Mobile playlist pages now render their track list, and desktop sidebar playlist
rows now stretch across the `ListView` width and left-align thumbnails with text
(`ui/main.slint`). Android MediaStore queries pass `is_music != 0` so voice
notes and ringtones do not inflate the library (`src/android.rs`). Account
restores and scans build the library through `Store::library_tracks()`,
excluding remembered search results and remote `yt://` duplicates of local files
(`src/services.rs`, `src/app.rs`, `src/web.rs`). Downloaded songs and tag-sync
writes persist `ytmusic:<id>` in the file comment, and scans/account sync map
tagged or ` [id]`-suffixed local files to their canonical `yt://<id>` identity
across likes, playlists, history, offsets and lyrics choices
(`src/media_library.rs`, `src/native.rs`, `src/local_sync.rs`,
`src/account.rs`). Cover art workers now share memory and disk cache entries
across local paths and `yt://<id>` identities, check disk/embedded covers in a
single blocking pass, store disposable cache covers as JPEG with PNG fallback,
and run up to 6 concurrent fetches (+1 reserved slot for the active song) while
pre-queuing sidebar playlist covers (`src/artwork.rs`, `src/app.rs`,
`src/web.rs`). Unit tests in `artwork`, `services`, `account`, `media_library`
and `local_sync` pass, along with `wasm32-unknown-unknown` check. Release
packages built and verified: Windows x64 `dist/Vey-0.1.46-windows-x64.zip`
(SHA-256 `ff125b5579b9c86031fe288a9f17e18de9b93b4c1c06e121b0a4bc3ed2a4752f`,
packaged executable matches `target/release/vey.exe`), Android ARM64
`dist/Vey-0.1.46-android-arm64.apk` (SHA-256
`1d4dfda7c181cadf213daab53364712f7ab8c6196c864af9cc21918d750a746d`, verified
with `apksigner`, `zipalign` and `aapt`), and Web WASM bundle in `web/pkg/`. No
Android device or live audio runtime check was run.

### Published 0.1.45 release (2026-11-02)

GitHub release `v0.1.45` is published with Windows x64, Linux x64 (tar.gz and
.deb), macOS x64/arm64, Android ARM64 and Web packages, each with a SHA-256
file. Windows ZIP `dist/Vey-0.1.45-windows-x64.zip` SHA-256
`560a6f64a4823ad5e40e7b3e848b35f5c9a34b34b1c042fa5a5cdddc27b368a9`
was rebuilt from the tag source and matched its checksum. Other platform
artifacts were already attached to the draft from the same tag and were not
rebuilt in this session. No device, audio or UI runtime acceptance is claimed.

### Cleaner lyric view (0.1.40, 2026-11-01)

Credits and the manual time-offset badge are shown only while track tools are
open; the offset remains editable there. Windows ZIP verified:
`dist/Vey-0.1.40-windows-x64.zip`, SHA-256
`b560edeb06647f78c881a500d866ef1859f9312fc7de1601bfe1fdd3a8702370`.
Packaged executable hash matches the release binary. No live visual or other
platform runtime check is claimed.

### Configurable lyric spacing (0.1.39, 2026-11-01)

Adds saved 0–150% spacing control for romanization and translation with a 65%
default, shared by native and browser lyric renderers. Windows ZIP verified:
`dist/Vey-0.1.39-windows-x64.zip`, SHA-256
`4c98f91fd63adf6c3c546e76357b06a7888a0f2c358027d41d6b5e0d20e48905`.
Packaged executable hash matches the release binary. Linux, macOS, Android
runtime and in-app visual playback were not checked in this build.

### System media controls (0.1.38, 2026-11-01)

Added desktop SMTC/MPRIS/Now Playing publication, Android MediaSession metadata
and transport callbacks, plus browser Media Session metadata and actions. Windows
release compilation, WASM bundle generation and Android ARM64 cross-compilation
passed. The verified Windows ZIP is `dist/Vey-0.1.38-windows-x64.zip`, SHA-256
`d931094d5bbf635228f39622bac2f9d372d4385256edac64999794ff35c1acbf`. Its
packaged executable hash matches `target/release/vey.exe`. No Android device,
Linux or macOS runtime check is claimed.

### Lyric spacing, Home empty state and platform builds (0.1.37, 2026-11-01)

Native romanization/translation rows now reserve more vertical room and keep
their scroll offsets aligned. The Collection Home layout removes an empty
"Listen again" heading; unavailable recommendations now explain the empty state
and offer Explore. Windows, WASM and Android cross-target checks passed. Android
device audio and Linux/macOS runtime were not checked. The local Windows x64
ZIP's integrity and contents were verified: `dist/Vey-0.1.37-windows-x64.zip`,
SHA-256 `f17f3aab420083f0cfaeaa25756470d0476ce1f13bab68f46278299ddde801e6`.

### Player loading optimization (0.1.36, 2026-11-01)

### Player loading optimization (2026-11-01)

Native and browser artwork queues limit concurrent work and reserve a current
track slot; local artwork is decoded once and browser fetches have size/time
limits. Validated YouTube cache metadata avoids repeat network resolution;
parallel audio ranges refill immediately and assemble into one bounded buffer.
Native audio/artwork and browser fixtures passed; see `docs/PERFORMANCE.md` for
scope, tested navigation tradeoffs and remaining device/runtime limits.

### Desktop player and Windows package (0.1.35, 2026-11-01)

Standard, expanded and focus layouts use larger artwork and a split lyric view.
The isolated `player_layout_reference` fixture captured desktop/mobile layouts
and verified mode changes with pointer events. The UI's native and WASM compile
checks passed before packaging. `scripts/build-release.ps1` built the local Windows x64 ZIP; its archive
checksum, packaged executable identity and release README were verified.
SHA-256: `cc7483956df06af226ce042fecaf59392c6038cad586e049343e2d20bd5983ee`.
Real audio playback and other platform releases were not checked in this build.

### Home recommendations and Windows package (0.1.34, 2026-11-01)

Home blends up to three seeds from the current song, recent history and liked
tracks. Native and browser fetch public YouTube Music radio concurrently, rank
cross-radio matches, filter the 12 most recent plays/liked songs and vary artists;
public Home picks are the fallback. `cargo check --locked --lib --target wasm32-unknown-unknown`
passed. `scripts/build-release.ps1` built the local
Windows x64 ZIP `dist/Vey-0.1.34-windows-x64.zip`; archive and SHA-256 verified:
`286d3ca2ad56125e4ca5a8ff91d2cc8f97ead85192940e1529c4389422a470ca`. No runtime
smoke test or test suite was run. Slint reported LyricInk/LyricsView export
warnings during the release build.

## Product

Vey is a standalone music player rewritten from the original PlayerVF
TypeScript/Tauri application into Rust/Slint. It plays local music, exposes
public YouTube Music discovery, and displays synchronized lyrics. Ukrainian
labels, bundled Roboto/Lucide assets and the Vey plate logo are used in the UI.
Full feature and visual parity with the original application is unfinished.

| Area | Implemented behavior | Important qualification |
| --- | --- | --- |
| Navigation | Home, Explore, Search, Library, Liked, Playlist, Queue, Settings, Player | Public album/artist/playlist detail UX remains incomplete |
| Playback | Local audio including Opus; play/pause, seek, volume, next/previous, repeat, shuffle, automatic advance | Playback speed controls removed in 0.1.6; old saved speeds normalize to 1x |
| Library | Recursive file/folder scan, metadata, artwork caching, deduplication, unavailable-source reporting | Android selection and browser metadata differ from desktop |
| Public YTM | Search, suggestions, browse, radio/recommendations, stream resolution/cache, downloads, clip lookup | Search evidence is separate from successful streaming; see I-01 |
| Personal local data | Likes, playlists, history, sources, lyric offsets, backup/restore; public YouTube Music/Spotify playlist import | YouTube private library uses desktop browser login; Spotify public tracks map to YTM `yt://` IDs with no Spotify account; docs/MUSIC-ACCOUNTS.md |
| Lyrics | Shared format detection for paste/providers/migration; LRC/enhanced LRC, TTML, QRC, SRT, WebVTT, plain and nested JSON | Provider dialect coverage is not exhaustive |
| Rich lyrics | Real word/syllable timing, translations, romanization, backing vocals, instrumental gaps, scroll/stage, reduced motion | Native Rust Braccato provides sweep/glow/waves; arbitrary browser CSS remains browser-only |
| Native lyric scroll | Prepared destinations, eased rest launch, retained-velocity retargets and stable first wrapped row (0.1.28) | Windows GPU/software fixtures; other platforms not re-measured |
| Native lyric ink/letters | Shaped kerning/ligature/mark clusters, shared Text origin, neutral cue-bounded waves and physical-pixel feather (0.1.30) | Windows GPU/software at 1x/1.25x; imported-font/device limits remain in `docs/NATIVE-BRACCATO.md` |
| Authentication | Lyric-provider Turnstile exchange, separate JWT cache, startup prewarm, renewal/retry | Google sign-in is a separate system-browser action |
| Metadata tools | Review a YTM match, write local tags, create/list/restore tag backups | Format support is bounded by tag-writing implementation |
| Appearance | Dynamic, fixed, or background-only colors; six fixed hues; smooth palette changes; borderless player surfaces | Settings persist across native/browser state and backups; see `docs/APPEARANCE.md` |
| Player themes | 20 offline styles, whole-player profiles, independent Braccato preview, CSS/RICS roles and lyrics-only opt-out | Native GPU/software, 360px and browser fixtures; native ports and supported scope in `docs/LYRIC-THEMES.md` |
| Mobile UX | Bottom tabs, mini-player, player overlay, focused settings subpages, retained scroll, browser history | Browser emulation does not establish Android device behavior |

## Platform matrix

Renderer rows updated 2026-10-27: native targets use the copied Rust Braccato
component. Exact evidence and remaining device limits: `docs/NATIVE-BRACCATO.md`.

| Target | Shell/audio | Lyrics | Runtime evidence boundary |
| --- | --- | --- | --- |
| Windows | Slint + native Rodio; desktop dialogs | Copied Rust Braccato Slint component | Native renderer visual/behavior fixtures passed (release builds, 0.1.17–0.1.27); live online parity remains separate |
| Browser | Rust/WASM Slint canvas + HTML audio; JS file picker | Same-origin Braccato iframe | Historical headless Edge fixtures and touch emulation; files must be reselected after reload |
| Linux | Native Rust/Slint path; CI build configured | Copied Rust Braccato Slint component | Local runtime unverified; older WSL attempt failed to attach its disk |
| Android ARM64 | Slint native activity, library `playervf` | Copied Rust Braccato Slint component | ARM64 library compile check passed; no connected-device runtime acceptance |

Browser shell has no screen-reader support in the Slint canvas; lyric and
highlighted-editor surfaces use DOM. Static web hosting can play selected local
files; public YTM needs the Rust same-origin backend.

## Recent completed work, as recorded in repository reports

### Windows release rebuild (2026-10-30)

Rebuilt the current working tree as Vey 0.1.33 Windows x64 with
`scripts/build-release.ps1`. The ZIP includes the executable, release README,
third-party notices and `MUSIC-ACCOUNTS.md`. Archive integrity, checksum and
packaged executable identity passed; the packaged app completed its isolated
18-screenshot startup fixture. This is a local build, not a published release;
private provider sync and other platform runtime acceptance remain separate.

### Playlist import, account connections and Home/search (historical snapshot, 2026-10-30)

At that time, public YouTube playlist URL import needed no login; the account chooser connected
Vey, YouTube Music and Spotify independently. Spotify OAuth was subsequently retired on 2026-11-04; current scope is public playlist import to YTM IDs only. YouTube uses normal Google v3
account chooser in an owned Chrome/Edge profile, with read-only in-page requests. Live private acceptance stays separate. Six Home compositions,
diagram previews/random entry, local term search, debounce and bounded query caching
are implemented. Windows/WASM, 267 Rust checks (known locale excluded), native/browser
fixtures and 7/552-track public probes passed; scope: docs/MUSIC-ACCOUNTS.md.

### Spotify account removal and public import (2026-11-04)

Spotify OAuth/account sync was removed. A public playlist import reads
unauthenticated embed metadata, requires an exact YTM match and stores `yt://`
tracks. Old Spotify account fields are dropped from `music-accounts.json` at
startup; `checklist/11-spotify-public-import.md` tracks remaining fixtures and
desktop/Web/Android acceptance. Do not inherit the 2026-10-30 Spotify UI/test
results as current verification.

### Sharp-letter artifacts (0.1.33, 2026-10-29)

The previous halo fix missed damage inside GPU text. Actual-component captures
reproduce 8,812 wrong glyph-core pixels in 48 old-path samples. Stable solid
text/mask composition keeps one GPU surface path across all coverage states;
128 expanded Latin/Cyrillic/accent samples at 1x/1.25x have zero wrong cores.
Full phrase GPU/software/reduced, effect and fractional-quality fixtures pass;
256 Rust checks pass (known locale failure excluded), WASM checks and Windows
release build pass. Evidence and device limits: `docs/NATIVE-BRACCATO.md`.

### Filled lead glow and edge sampling (0.1.33, 2026-10-29)

Sung lead words keep a soft halo through the active line. Glow clips share one
full-tile sampling origin and a physical-pixel grid; late-seek/residency rules
match the line lifetime. Backing cue release and sharp fill clocks are retained.
GPU/software/fractional-scale fixtures and the Windows build are recorded in
`docs/NATIVE-BRACCATO.md`.

### Player interaction performance (0.1.33, 2026-10-29)

Shell motion now uses 100–180ms live page travel, immediate Settings sections,
100ms panels and a 140ms mobile player sheet. Native GPU route settle measured
491→185ms median in the 300-track fixture; browser navigation performs no page
readbacks. Software static corner clipping remains I-18.

Indexed catalog/playlist work, retained page rows, targeted cover/like updates
and coalesced browser saves reduce controller work. Rapid browser track changes
suppress obsolete play errors. Windows release/WASM builds and targeted checks
are recorded with measured workloads and runtime boundaries in `docs/PERFORMANCE.md`.

### Native lyric renderer (0.1.17–0.1.27, 2026-10-26/27)

Every entry below was measured on the local Windows release fixtures and is
recorded with its exact workload in `docs/NATIVE-BRACCATO.md`.

- 0.1.17: lighter native renderer; software fallback path.
- 0.1.18: scroll timing and quieter glow.
- 0.1.19: long/hyphenated text, flowing fill.
- 0.1.20: cue boundaries and complete fill.
- 0.1.21: smooth line handoff.
- 0.1.22: stable whole-word wobble, independent letter waves.
- 0.1.23: fill consistency, cue-bounded glow lifetime.
- 0.1.24: renderer-aware spatial fill (GPU vs software).
- 0.1.25: retained word deadlines, shared prepared glow textures; median
  word-model reads 788→40 (GPU) and 806→32 (software) on the 300-line fixture.
- 0.1.26: player-entry and lyric-handoff stalls; viewport-windowed row delegates,
  retained focus, font-metric caches, bounded navigation blur worker. Median p95
  event-loop interval 61ms→31ms; first-entry max 803ms→64–81ms.
- 0.1.27: `src/native_lyric_camera.rs` retained-velocity scroll camera. 20px
  launch latency 121.4ms→65.6ms (66.6ms software); moving retargets keep exact
  incoming velocity; five unit regressions plus `examples/native_line_flow.rs`.

### Earlier product releases

- 0.1.0: Vey branding; bounded event processing, coalesced background saves,
  mounted page surfaces and shared camera/lyric-host motion.
- 0.1.1: cover-derived tonal palette, organic shapes and material controls.
- 0.1.2: phone settings subpages, file-import checks, Back/Forward restoration,
  transparent logo, retained-page mobile player overlay.
- 0.1.3: transparent cover/metadata panels, quieter secondary actions, removal
  of redundant captions/provider footer, consistent rounded covers.
- 0.1.4: Home/Explore shelf scrollbar hidden; mouse dragging and horizontal
  scrolling retained. Android build support is present in the working tree.
- 0.1.5: Android audio permission and MediaStore library discovery; native
  status/navigation/cutout insets; safe modal placement and scrolling.
- 0.1.6: spaced playlist picker with actual destinations and create-and-add;
  volume thumb/drag geometry and keyboard endpoints; speed controls removed.
- 0.1.7: explicitly left-anchor the volume fill; pixel regression at empty,
  quarter, half, three-quarter and full volume (`examples/volume_fill.rs`).
- 0.1.8: remove the enclosing volume focus frame, retaining keyboard focus.
  Pixel fixtures also cover focused 84/64-pixel desktop sliders.
- 0.1.9: Appearance settings with dynamic/static/background-only palettes and
  six fixed hues; one-second interrupted color fades, eased page travel and
  settings fades. Hidden page painting pauses during palette changes.
- 0.1.10: unwrap complete Markdown lyric fences before detecting formats,
  including nested JSON/provider results; preserve Composer TTML title/artist/
  album metadata. Invalid fenced XML is rejected; see `docs/LYRIC-INPUT.md`.
- 0.1.11: recover complete TTML from chat/fence debris and escaped transport;
  normalize numeric space artifacts without decoding valid XML text twice.
- 0.1.12: local media and lyric tools release.
- 0.1.13: platform release; Web, Linux and Android x64 packaging.
- 0.1.14: platform release.
- 0.1.15: system audio output following and device reconnection.
- 0.1.16: mobile lyric scrolling, player gestures, responsive search input,
  macOS packaging.

## Verification inventory: historical, not freshly rerun

The checkout contains 229 `#[test]` functions and 9 `#[ignore]` attributes across
`src/` and `tests/` (counted 2026-10-27). No suite was executed during the
documentation/cleanup pass that produced this snapshot.

- 0.1.27 change: five new camera unit tests plus `examples/native_line_flow.rs`
  release runs; the 0.1.27 note records line-flow, timing, boundary, autoscroll,
  focus-handoff and the 300-line stutter fixture passing after the change.
- 0.1.26/0.1.25: release performance fixtures with recorded JSON under
  `target/native-lyrics-check/` — those directories no longer exist after the
  2026-10-27 cleanup; the numbers live in `docs/NATIVE-BRACCATO.md`.
- 0.1.11 cleanup task: 161 Rust tests passed / seven ignored.
- 0.1.10 parser task: 159 Rust tests passed / seven ignored.
- 0.1.9 task: 157 Rust tests passed / seven ignored; WASM and widget build,
  appearance/material/phone/transitions browser fixtures, 12 synthetic native
  inset layouts. See `docs/APPEARANCE.md`; does not prove Android device runtime.
- `docs/vey-native-smoke.json`: dated 2026-10-19, version 0.1.4, exit 0, seven
  lyric lines and ambient frame width 256. Fixture smoke only.
- `docs/phone-check.json`, `docs/material-check.json`,
  `docs/braccato-widget-check.json`, `docs/braccato-wasm-check.json`,
  `docs/kawarp-check.json`: targeted recorded evidence; inspect for exact scope.
- `docs/navigation-performance.json`, `docs/braccato-performance.json`,
  `docs/native-lyrics-performance-2026-10-26.json`: environment-specific
  samples/microbenchmarks, not overall FPS guarantees.
- `.github/workflows/rust.yml`: Windows/Linux tests/builds, WASM build and
  Android library build are configured; configuration alone proves no CI run.
