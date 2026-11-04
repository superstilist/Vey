# Project map

Source version in `Cargo.toml`: 0.1.46 (mobile playlists, library count and YouTube Music file tagging, 2026-11-02).
This is an orientation cache, not a claim that builds or runtime checks passed on
that date.

## Detailed information index

| Need | Read |
| --- | --- |
| What the app does and recent completed work | [status.md](status.md) |
| Runtime/data flow and compatibility contracts | [architecture.md](architecture.md) |
| Current source gaps, historical bugs, unknowns | [issues.md](issues.md) |
| Next tasks and acceptance criteria | [tasks.md](tasks.md) |
| UI and full functionality refinement proposals | [UI proposal](../docs/UI-REFINEMENT-PLAN.md), [functionality proposal](../docs/FUNCTIONALITY-REFINEMENT-PLAN.md) |
| Commands and efficient verification | [workflow.md](workflow.md) |

Only load the relevant detail file; this map remains the short starting point.

## Identity and boundaries

- Vey: native music player with synchronized lyrics; Rust 2021 + Slint 1.18.1.
  `Cargo.toml` defines binary/package `vey` and library `playervf`.
- Desktop native, browser WASM and Android share the Rust/Slint shell. No Tauri.
  The original reference checkout is `C:/Users/superstilist/WebstormProjects/playervf`;
  inspect it only when needed and keep changes in this standalone project.
- Native targets use the copied Rust Braccato Slint component in
  `third-party/braccato-rust/`; web retains the bundled upstream iframe.
- Preserve upstream Braccato source in `third-party/braccato/`; integration
  changes belong in the host/widget. Provenance: `third-party/BRACCATO-UPSTREAM.md`.
- YouTube login uses the normal Google v3 account chooser in a Vey-owned
  Chrome/Edge profile, independently of Vey login. No Google OAuth app setup.
  Read-only site requests stay in that browser; ordinary profiles are untouched.
  Public playlist import needs no account. Migration is allowlisted and
  excludes account data, cookies, JWTs and Turnstile tokens.
- Keep PlayerVF storage/bridge identities compatible. Windows state is
  `%APPDATA%/PlayerVF-Rust/state.json`; `PLAYERVF_DATA_DIR` overrides storage.
  Lyrics auth is separate in `lyrics-auth.json` and excluded from backups.

## Repository shape

41 files at `src/*.rs` (24.2k lines) plus 12 `src/braccato/` modules (2.6k
lines), 26.8k together; 12 Slint files under `ui/` (3.1k lines); 5 integration suites and 8 fixtures in
`tests/`; 29 runnable fixtures in `examples/`; 27 documents in `docs/`;
31 browser scripts in `tools/braccato-widget/`, each with an `npm` entry
(`build`, `serve`, `test` and 28 `test:*` aliases).
`src/native.rs` alone is 7.9k lines and carries audio, scanning, YTM and captions.

Generated output is not kept in the checkout: `target/`, `dist/`, `web/pkg/`,
`tools/braccato-widget/node_modules/` and `.reference/` are ignored and were
removed on 2026-10-27. The first build after that is a cold build.

## Where to edit

| Concern | Start here |
| --- | --- |
| Platform release archives | scripts/package-web.ps1, scripts/build-linux.sh, scripts/build-android.ps1; docs/WEB-RELEASE.md, docs/LINUX-RELEASE.md |
| Startup, CLI modes, target gates | `src/main.rs`, `src/lib.rs`, `Cargo.toml` |
| Native callbacks, async results, UI models | `src/app.rs` |
| Retained track-list models, indexed cover/like updates | `src/track_model.rs`; benchmarks and boundaries in `docs/PERFORMANCE.md` |
| Local audio, scanning, public YTM | `src/native.rs`, `src/audio.rs` |
| Vendored YT Music catalog parser | `third-party/yt-rust/`, `third-party/YT-RUST-UPSTREAM.md`, `docs/YT-RUST-INTEGRATION.md`; native service also serves browser search |
| Android media permission and device audio discovery | `src/android.rs`, `src/native.rs` (`scan_android_library`) |
| Vey accounts, metadata sync, private audio | `src/account.rs`, `web/account.js`, `docs/ACCOUNTS.md`; backend in `blueberry_melody` |
| Playlist import, YTM browser login and public Spotify→YTM import | `src/library_import.rs`, `src/music_accounts.rs`, `src/spotify_import.rs`, `src/youtube_browser.rs`, `native::import_public_playlist`; `docs/MUSIC-ACCOUNTS.md` |
| Home compositions and local search/cache | `src/home.rs`, `ui/home-preview.slint`, `src/search_index.rs`; `docs/MUSIC-ACCOUNTS.md` |
| Store, queue, migration, persistence | `src/services.rs`, `src/storage.rs` |
| Shared lyric detection/parsing | `src/lyrics.rs` (`parse_payload`), `src/braccato/` |
| Per-song lyric choices, display layers and compact settings | `Store.lyrics_choices`, `src/preferences.rs`, `ui/lyrics-settings.slint`; `docs/LYRICS-PREFERENCES.md` |
| Syllable timing and upstream review | `docs/SYLLABLE-LYRICS.md`, `tests/syllable_lyrics.rs`, widget `test:syllables` |
| Automatic lyric translation/romanization | src/lyrics_translation.rs, Store.lyrics_enrichment, player track menu; browser /api/lyrics/enrich |
| Providers, caching, JWT/challenge | `src/lyrics_service.rs`, `src/challenge.rs` |
| Native lyric adapter, clock, wrapping, follow lookup and bounded glyph preparation | `src/native_lyrics.rs`, `src/native_lyric_follow.rs`, `ui/native-lyrics.slint`; `docs/NATIVE-BRACCATO.md` |
| Native scroll camera and prepared destinations | `src/native_lyric_camera.rs`, `src/native_lyric_follow.rs`; `examples/native_line_flow.rs`, `examples/native_lyric_gap.rs`; `docs/NATIVE-BRACCATO.md` |
| Native focus masks and reference typography | `src/native_lyric_focus.rs`, `src/native_lyric_masks.rs`, `src/native_lyric_fonts.rs`; `examples/native_lyric_effects.rs` |
| Custom lyric fonts, CSS controls and filled-only glow | `src/themes.rs`, `src/native_lyric_fonts.rs`, `src/native_lyric_paint.rs`; `examples/native_lyric_custom.rs` |
| Offline lyric presets, chooser and credits | `src/theme_presets.rs`, `ui/lyric-theme-picker.slint`, `assets/lyric-themes/`; `docs/LYRIC-THEMES.md` |
| Upstream widget, clock, ambient texture | `tools/braccato-widget/widget.ts`, `clock.ts`, `ambient.ts`, `widget.css` |
| WASM controller and browser bridges | `src/web.rs`, `web/bridge.js`, `web/player-motion.js` |
| Same-origin API/streaming server | `src/server.rs` |
| Layout, controls, tokens | `ui/main.slint`, `ui/controls.slint`, `ui/tokens.slint` |
| Cover decoding, tonal palette | `src/artwork.rs`, `src/material.rs` |
| Permanent downloads, embedded covers and triangle marks | `src/media_library.rs`, `src/app.rs`, `Store.downloads`; `docs/DOWNLOADS-ARTWORK.md` |
| Languages, shared motion, ambient preferences | `src/i18n.rs`, `locales/catalog.tsv`, `ui/motion.slint`, `src/preferences.rs` |
| Video art slot and platform adapters | `src/video.rs`, `src/video_linux.rs`, `ui/media-slot.slint`, `web/video-surface.js` |
| Color modes, fixed hues, smooth changes | `docs/APPEARANCE.md`, `Settings` in `src/services.rs`, Appearance in `ui/main.slint` |
| Metadata/tag backups | `src/local_sync.rs` |
| Screenshot fixtures | `src/snapshots.rs` |

## Design and performance contracts

- Read `docs/MATERIAL-STYLE.md` before UI work, `docs/PAGE-MOTION.md` for
  navigation, and `docs/PHONE-VERIFICATION.md` for mobile. Current design uses
  cover-derived tonal surfaces and organic artwork shapes; older flat-background
  notes and some README descriptions predate this design.
- Keep artwork/lyrics open and borderless. Braccato is the selected lyric theme;
  preserve upstream timing, backing vocals, translations and reduced motion.
- Settings follows the YouTube Music pattern: one centered column capped at 840px
  (32px mobile gutters), flat `SettingsRow`/`SwitchRow` groups separated by
  `SectionLabel`, and a fixed subpage header with a back arrow. Keep settings
  rows borderless; do not reintroduce `SettingsTile` cards.
- Keep pages mounted and navigation interruptible; bound background-result work
  and coalesce disk saves. Avoid artwork decoding, palette extraction, lyric
  document rebuilds and theme compilation on playback ticks. `CachedLyrics` uses
  shared `Arc` across `Store` snapshots; `Event::Art` refreshes only matching
  track rows, and track start updates player-bar flags directly without
  rescanning all page models. Navigation skips redundant timer delays once warmed.
- Hidden/paused widget work should suspend appropriately; preserve seek and
  rate changes, retained lyrics and startup retry behavior.
- Native lyric scroll ownership is explicit: the retained-velocity camera drives
  position only while follow is active; manual scroll, stage, passive and
  reduced-motion paths keep their own. Do not run two drivers at once.

## Status references: load only when relevant

- `README.md`: broad architecture and usage. "Latest release" was reconciled
  with `Cargo.toml` 0.1.27 on 2026-10-27 and still separates the published
  0.1.16 artifacts; recheck it against `Cargo.toml` after each release.
- `docs/PARITY.md`: incomplete features and platform differences. Its HTTP 403
  note is now dated historical evidence; the 2026-10-21 native recovery is in
  `docs/PLAYBACK-RECOVERY.md` (I-01) and the browser path stays unverified.
- `docs/VEY-VERIFICATION.md`, `docs/VERIFICATION.md`: historical evidence.
- `src/braccato/README.md`: upstream renderer integration and platform coverage.
- `docs/ANDROID-BUILD.md`, `docs/RELEASE.md`: packaging/signing.
- Android 0.1.5 adds runtime audio permission and MediaStore library discovery;
  shared UI uses native safe-area insets. See `docs/ANDROID-FIXES.md` for checks
  and remaining device acceptance.
- Known documented limits: video stream availability and some public detail
  navigation remain incomplete; platform runtime acceptance is separate. Android
  import/export dialogs remain stubs and device runtime is unverified; Linux
  runtime is also unverified. Recheck before asserting current behavior.
