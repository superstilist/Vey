# Bugs, limitations and unknowns

Reviewed 2026-10-20. No live reproductions in this documentation task.
Labels: **source-confirmed** = present in inspected implementation;
**reported** = historical repository evidence; **unverified** = missing runtime
evidence; **documentation** = inconsistent descriptions. Suggested priorities in
`tasks.md` are planning guidance, not newly assigned product requirements.

| ID | Type/status | Problem and impact | Evidence / entry points |
| --- | --- | --- | --- |
| I-01 | Reported tracks recovered 2026-10-21 | Android media URLs rejected ranges beyond 1 MiB. Public visionOS resolution now cached both reported tracks completely and decoded their full durations. Native load cancellation and range/cache validation also implemented. Audible listening and broader catalog/device acceptance remain separate. | `docs/PLAYBACK-RECOVERY.md`; `src/native.rs`, `src/app.rs`, `src/server.rs` audio handler |
| I-02 | Implemented adapters; platform acceptance pending | Muted video replaces the art slot through browser, WebView2, Android and FFmpeg adapters. Live streams and native target playback still require separate runtime evidence. | `src/video.rs`, `src/video_linux.rs`; `docs/PLAYER-REFINEMENT.md` |
| I-03 | Partially implemented; device acceptance pending | Since 0.1.5, device scanning requests audio permission and reads MediaStore; Android add-source starts device discovery. Document picker/save stubs still limit explicit selection and import/export; manual paths remain available. | `src/android.rs`, `src/native.rs`, `src/dialogs.rs`; `docs/ANDROID-FIXES.md` |
| I-04 | Android host implemented; device acceptance pending | Android now has a native WebView challenge bridge sharing Windows JWT exchange/renewal/parsing. Device acquisition remains unverified; Linux still has no challenge host. | `src/challenge.rs`, `src/android.rs`, `android/java/app/vey/VeyBridge.java` |
| I-05 | Source-confirmed browser gap | Selected local files lack real tag/artwork extraction; title is filename, artist unknown, album/artwork empty. | `web/bridge.js` file picker; `docs/PARITY.md` |
| I-06 | Browser session limitation | Local object URLs do not survive reload; files need reselection even when likes/playlists persist. | `web/bridge.js`; `README.md` |
| I-07 | Reported feature backlog | Public detail pages, continuation UX, download progress/cancellation, original shortcuts/context menus and exhaustive settings parity unfinished. | `docs/PARITY.md`; audit callbacks in `src/app.rs` and `src/web.rs` before implementation |
| I-08 | Intentional renderer difference | Linux/Android fallback cannot execute every browser CSS/RICS/Web Animations effect. Full visual parity cannot be claimed. | `src/braccato/README.md`, `src/themes.rs`, `ui/braccato-lyrics.slint` |
| I-09 | Reported compatibility gap | Some provider translation/romanization dialects and old preferences have no complete native/UI equivalent. | `docs/PARITY.md`; `src/lyrics.rs`, `src/braccato/ttml.rs`, `src/services.rs` |
| I-10 | Unverified Android runtime | APK signing/alignment/manifest and compile/link checks do not prove startup, audio, storage, browser intents, lifecycle or phone usability. | `docs/ANDROID-BUILD.md`; no device acceptance recorded |
| I-11 | Unverified Linux runtime | Prior local WSL failed because its disk was missing; local runtime and current CI execution are not established. | `docs/PARITY.md`, `.github/workflows/rust.yml` |
| I-12 | Documented accessibility limitation | Browser Slint canvas shell lacks screen-reader support; DOM lyric/editor accessibility does not cover the shell. | `README.md` |
| I-13 | Documentation drift | README still describes plain underlined tabs while current tonal notes describe filled pills; older theme/fallback and parity palette descriptions also predate current UI. | `README.md`, `docs/MATERIAL-STYLE.md`, `src/braccato/README.md`, `docs/PARITY.md` |

## Already addressed: retain regression coverage

These are historical fixes, not newly discovered open bugs:

- 2026-10-22 search input: native Windows WebView-to-shell clicks left Win32
  keyboard focus in the child. Pointer focus handoff and hidden-child release
  now have a real WebView regression fixture. Shared search input and Android
  silent-auth focus handling were hardened; device IME acceptance remains open.
  See `docs/SEARCH-INPUT.md`.

- 2026-10-21 syllable timing: real adjacent timed parts such as Sin + gle now
  retain RichSync through import/provider/cache display, native rows and the
  Braccato document. Lead/backing/romanization are checked independently; no
  timestamps are fabricated from a provider quality label. Other dialect gaps
  under I-09 remain. See `docs/SYLLABLE-LYRICS.md` and `test:syllables`.

- 0.1.11 TTML cleanup: complete documents can be recovered from surrounding
  prose, nested/mislabeled or broken outer fences. Unwrap valid fences/JSON
  before extraction, or quoted XML can swallow provider alternatives. Numeric
  spaces, including double escapes and whitespace between spans, are retained.

- 0.1.10 lyric input: Markdown-fenced TTML previously fell through to plain
  text. Unwrap complete fences before XML/JSON detection; reject malformed XML
  so existing lyrics survive a failed import. Preserve line-sync timing.

- 0.1.9 appearance: palette changes run on a finite 32 ms timer; avoid
  repainting every mounted hidden page during fades. Retain paused-background
  checks in `test:transitions` and mode/persistence checks in `test:appearance`.

- 0.1.6 playlist picker: real destinations exclude native navigation-only rows;
  64-pixel choices have spacing, titles/counts and centered icons. Creating from
  the picker adds the current track. Volume rail and drag coordinates align;
  old saved speeds reset to normal after removing playback speed controls.
  Regression: `npm run test:playlists`, `npm run test:phone`, `tests/theme_settings.rs`.
- 0.1.7 volume fill: decorative fill must explicitly set x/y to zero within
  the rail. Partial volume must grow from the left edge. Pointer/keyboard
  checks alone missed this visual bug; `examples/volume_fill.rs` samples
  actual rendered rail pixels at five volume levels.
- 0.1.8 volume frame: removed the slider-wide focus outline requested in the
  desktop screenshot. Focus is forwarded to the standard slider for keyboard
  input. Pixel checks cover focused 84-pixel and narrow 64-pixel controls as
  well as the wide settings slider; do not restore a border on the wrapper.

- Artwork-loading UI freeze: bounded decoding/workers and individual model-row
  updates replaced repeated full-list rebuild/PNG reload. `src/artwork.rs`,
  `src/app.rs`, `docs/VERIFICATION.md`.
- Auth challenge blocked by WebView2 data navigation and large IPC output:
  exact bundled-host navigation handling and output draining were added.
  `src/challenge.rs`, `src/lyrics_service.rs`, `README.md`.
- Lyric clock jitter, hidden animation work and malformed-document replacement:
  interpolation, duplicate suppression, visibility suspension and retention
  checks exist in widget clock/stability/recovery tests.
- Native lyric composition artifacts: opaque matching backdrop and crop
  updates documented in README; inspect host geometry if artifacts recur.
- Navigation stutter/double movement: shared camera and grouped host transform;
  test interrupted routes and stable local lyric bounds before changing it.
- Home/Explore shelf line: horizontal scrollbar hidden in 0.1.4; drag/wheel
  behavior retained. Do not confuse it with the playback seek bar.

## Evidence discipline

There is no complete fresh bug audit here and no external issue tracker was
queried. Reports are fixtures unless they explicitly record real hardware/live
services. A missing test is an unknown, not proof of a defect. Reproduce before
closing a historical issue; record date, environment, exact check and outcome.
Do not add Google account/session features to resolve an intentional exclusion.
