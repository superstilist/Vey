# Project map

Checked against the working tree on 2026-10-20. This is an orientation cache,
not a claim that builds or runtime checks passed on that date.

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
- Windows uses embedded WebView2; web uses an iframe for the bundled upstream
  Braccato renderer. Linux/Android use the Rust/Slint lyric fallback.
- Preserve upstream Braccato source in `third-party/braccato/`; integration
  changes belong in the host/widget. Provenance: `third-party/BRACCATO-UPSTREAM.md`.
- Google login uses the system browser. No Google session/cookie capture,
  account playlist/liked APIs or voice modules. Migration is allowlisted and
  excludes account data, cookies, JWTs and Turnstile tokens.
- Keep PlayerVF storage/bridge identities compatible. Windows state is
  `%APPDATA%/PlayerVF-Rust/state.json`; `PLAYERVF_DATA_DIR` overrides storage.
  Lyrics auth is separate in `lyrics-auth.json` and excluded from backups.

## Where to edit

| Concern | Start here |
| --- | --- |
| Startup, CLI modes, target gates | `src/main.rs`, `src/lib.rs`, `Cargo.toml` |
| Native callbacks, async results, UI models | `src/app.rs` |
| Local audio, scanning, public YTM | `src/native.rs`, `src/audio.rs` |
| Vendored YT Music catalog parser | `third-party/yt-rust/`, `third-party/YT-RUST-UPSTREAM.md`, `docs/YT-RUST-INTEGRATION.md`; native service also serves browser search |
| Android media permission and device audio discovery | `src/android.rs`, `src/native.rs` (`scan_android_library`) |
| Store, queue, migration, persistence | `src/services.rs`, `src/storage.rs` |
| Shared lyric detection/parsing | `src/lyrics.rs` (`parse_payload`), `src/braccato/` |
| Per-song lyric choices, display layers and compact settings | `Store.lyrics_choices`, `src/preferences.rs`, `ui/lyrics-settings.slint`; `docs/LYRICS-PREFERENCES.md` |
| Syllable timing and upstream review | `docs/SYLLABLE-LYRICS.md`, `tests/syllable_lyrics.rs`, widget `test:syllables` |
| Automatic lyric translation/romanization | src/lyrics_translation.rs, Store.lyrics_enrichment, player track menu; browser /api/lyrics/enrich |
| Providers, caching, JWT/challenge | `src/lyrics_service.rs`, `src/challenge.rs` |
| Native lyric host, geometry, playback bridge | `src/lyrics_widget.rs` |
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
- Keep pages mounted and navigation interruptible; bound background-result work
  and coalesce disk saves. Avoid artwork decoding, palette extraction, lyric
  document rebuilds and theme compilation on playback ticks.
- Hidden/paused widget work should suspend appropriately; preserve seek and
  rate changes, retained lyrics and startup retry behavior.

## Status references: load only when relevant

- `README.md`: broad architecture and usage; verify details against source.
- `docs/PARITY.md`: incomplete features and platform differences.
- `docs/VEY-VERIFICATION.md`, `docs/VERIFICATION.md`: historical evidence.
- `src/braccato/README.md`: upstream renderer integration and platform coverage.
- `docs/ANDROID-BUILD.md`, `docs/RELEASE.md`: packaging/signing.
- Android 0.1.5 adds runtime audio permission and MediaStore library discovery;
  shared UI uses native safe-area insets. See `docs/ANDROID-FIXES.md` for checks
  and remaining device acceptance.
- Known documented limits: public streaming had upstream HTTP 403; video
  stream availability and some public detail navigation remain incomplete. Video
  surface adapters now exist; platform runtime acceptance is still separate. Android
  import/export dialogs remain stubs and device runtime is unverified; Linux runtime is also
  unverified. Recheck before asserting current behavior or full parity.
