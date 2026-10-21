# Capabilities and project status

Snapshot: 2026-10-21, working-tree source and checked-in reports. Version in
`Cargo.toml`: 0.1.14. Release packages are being prepared; platform build
evidence belongs to the current release workflow, not this status snapshot.
Implemented does not mean verified on every target.

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
| Personal local data | Likes, local playlists, history, sources, lyric offsets, backup/restore | Google account library APIs are intentionally excluded |
| Lyrics | Shared format detection for paste/providers/migration; LRC/enhanced LRC, TTML, QRC, SRT, WebVTT, plain and nested JSON | Provider dialect coverage is not exhaustive |
| Rich lyrics | Word timing, translations, romanization, backing vocals, instrumental gaps, scroll/stage, reduced motion | Full upstream effects require Windows/browser host |
| Authentication | Lyric-provider Turnstile exchange, separate JWT cache, startup prewarm, renewal/retry | Google sign-in is a separate system-browser action |
| Metadata tools | Review a YTM match, write local tags, create/list/restore tag backups | Format support is bounded by tag-writing implementation |
| Appearance | Dynamic, fixed, or background-only colors; six fixed hues; smooth palette changes; borderless player surfaces | Settings persist across native/browser state and backups; see `docs/APPEARANCE.md` |
| Mobile UX | Bottom tabs, mini-player, player overlay, focused settings subpages, retained scroll, browser history | Browser emulation does not establish Android device behavior |

## Platform matrix

| Target | Shell/audio | Lyrics | Runtime evidence boundary |
| --- | --- | --- | --- |
| Windows | Slint + native Rodio; desktop dialogs | Bundled upstream Braccato in WebView2 | Historical fixture/native smoke and local playback evidence; live online parity incomplete |
| Browser | Rust/WASM Slint canvas + HTML audio; JS file picker | Same-origin Braccato iframe | Historical headless Edge fixtures and touch emulation; files must be reselected after reload |
| Linux | Native Rust/Slint path; CI build configured | Rust/Slint fallback | Local runtime unverified; older WSL attempt failed to attach its disk |
| Android ARM64 | Slint native activity, library `playervf` | Rust/Slint fallback | APK packaging checks documented; no connected-device runtime acceptance |

Browser shell has no screen-reader support in the Slint canvas; lyric and
highlighted-editor surfaces use DOM. Static web hosting can play selected local
files; public YTM needs the Rust same-origin backend.

## Recent completed work, as recorded in repository reports

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

Sources: `docs/VEY-VERIFICATION.md`, `docs/MATERIAL-STYLE.md`,
`docs/PHONE-VERIFICATION.md`, `docs/RELEASE.md`, `docs/ANDROID-BUILD.md`.

## Verification inventory: historical, not freshly rerun

Latest 0.1.11 cleanup task: 161 Rust tests passed / seven ignored, including
messy TTML wrappers, escaped-space preservation and nested JSON/provider
alternatives; see `docs/LYRIC-INPUT.md`.

Latest 0.1.10 parser task: 159 persistent Rust tests passed / seven ignored,
plus an exact-input local check for all 46 supplied TTML cues; see
`docs/LYRIC-INPUT.md`. Full song text is not stored in repository fixtures.

Latest 0.1.9 task: 157 Rust tests passed / seven ignored; WASM and widget build,
appearance/material/phone/transitions browser fixtures, and 12 synthetic native
inset layouts passed. See `docs/APPEARANCE.md`; this does not prove Android
device runtime. The following older evidence retains its original scope.

- Rust reports progressed from 98 tests to 151, then 153 passing with seven
  environment-dependent checks ignored. Use actual new output for current counts.
- Reports cover widget effects, malformed-document retention, clocks, recovery,
  ambient output, interrupted navigation, material inputs and phone settings.
- `docs/vey-native-smoke.json`: dated 2026-10-19, version 0.1.4, exit 0,
  seven lyric lines and ambient frame width 256. Fixture smoke only.
- `docs/phone-check.json`, `docs/material-check.json`,
  `docs/braccato-widget-check.json`, `docs/braccato-wasm-check.json`,
  `docs/kawarp-check.json`: targeted recorded evidence; inspect for exact scope.
- `docs/navigation-performance.json`, `docs/braccato-performance.json`:
  environment-specific samples/microbenchmarks, not overall FPS guarantees.
- `.github/workflows/rust.yml`: Windows/Linux tests/builds, WASM build and
  Android library build are configured; configuration alone proves no CI run.
