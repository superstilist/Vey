# Bugs, limitations and unknowns

Reviewed 2026-10-27 against the working tree (0.1.27). No live reproductions in
this documentation/cleanup pass; nothing here was re-run.
Labels: **source-confirmed** = present in inspected implementation;
**reported** = historical repository evidence; **unverified** = missing runtime
evidence; **documentation** = inconsistent descriptions. Suggested priorities in
`tasks.md` are planning guidance, not newly assigned product requirements.

| ID | Type/status | Problem and impact | Evidence / entry points |
| --- | --- | --- | --- |
| I-01 | Reported tracks recovered 2026-10-21 | Android media URLs rejected ranges beyond 1 MiB. Public visionOS resolution now cached both reported tracks completely and decoded their full durations. Native load cancellation and range/cache validation also implemented. Audible listening and broader catalog/device acceptance remain separate. | `docs/PLAYBACK-RECOVERY.md`; `src/native.rs`, `src/app.rs`, `src/server.rs` audio handler |
| I-02 | Implemented adapters; platform acceptance pending | Muted video uses browser video, Android TextureView or desktop FFmpeg frames. FFmpeg must be installed for desktop clips; live streams and device playback need separate runtime evidence. | `src/video.rs`, `src/video_linux.rs` |
| I-03 | Partially implemented; device acceptance pending | Since 0.1.5, device scanning requests audio permission and reads MediaStore (`is_music != 0` since 2026-11-02); Android add-source starts device discovery. Document picker/save stubs still limit explicit selection and import/export; manual paths remain available. | `src/android.rs`, `src/native.rs`, `src/dialogs.rs`; `docs/ANDROID-FIXES.md` |
| I-04 | Embedded Windows flow implemented; runtime acceptance pending | Windows opens the provider Turnstile page in an opaque Wry child over the lyrics viewport, exchanges a user-completed token for a locally cached JWT and retries once. System browser is a fallback on WebView failure or user request; other native targets retain their existing browser flow. No CAPTCHA bypass. Real challenge completion and non-Windows runtime acceptance remain unverified. | `src/challenge.rs`, `src/lyrics_service.rs`, `src/app.rs`, `ui/main.slint`, `assets/lyrics-challenge.html` |
| I-05 | Source-confirmed browser gap | Selected local files lack real tag/artwork extraction; title is filename, artist unknown, album/artwork empty. | `web/bridge.js` file picker; `docs/PARITY.md` |
| I-06 | Browser session limitation | Local object URLs do not survive reload; files need reselection even when likes/playlists persist. | `web/bridge.js`; `README.md` |
| I-07 | Reported feature backlog | Public detail pages, continuation UX, download progress/cancellation, original shortcuts/context menus and exhaustive settings parity unfinished. | `docs/PARITY.md`; audit callbacks in `src/app.rs` and `src/web.rs` before implementation |
| I-08 | Native Rust renderer; platform acceptance separate | Native targets use the copied Rust Braccato Slint component (sweep/glow/waves/stage). Arbitrary browser CSS/RICS/DOM extensions remain browser-only; exact visual parity and device behavior require separate checks. | `docs/NATIVE-BRACCATO.md`, `third-party/BRACCATO-RUST-UPSTREAM.md` |
| I-09 | Reported compatibility gap | Some provider translation/romanization dialects and old preferences have no complete native/UI equivalent. | `docs/PARITY.md`; `src/lyrics.rs`, `src/braccato/ttml.rs`, `src/services.rs` |
| I-10 | Unverified Android runtime | APK signing/alignment/manifest and compile/link checks do not prove startup, audio, storage, browser intents, lifecycle or phone usability. | `docs/ANDROID-BUILD.md`; no device acceptance recorded |
| I-11 | Unverified Linux runtime | Prior local WSL failed because its disk was missing; local runtime and current CI execution are not established. | `docs/PARITY.md`, `.github/workflows/rust.yml` |
| I-12 | Documented accessibility limitation | Browser Slint canvas shell lacks screen-reader support; DOM lyric/editor accessibility does not cover the shell. | `README.md` |
| I-18 | Observed Windows software-renderer fixture failure, 2026-10-29 | `navigation_transition` failed its static stage-corner pixel assertion before the first route change under `SLINT_BACKEND=winit-software`; the GPU fixture passed. Software clipping conformance remains open. | `examples/navigation_transition.rs::check_corners`; `docs/PERFORMANCE.md` |
| I-19 | Native animation stutter partially addressed, 2026-11-06 | Layout/ambient work uses bounded workers. Ink capacity no longer replaces delegates during fill. A light 8ms clock pass retains 16ms housekeeping; same-executable audio runs reduced repeated fill-clock frames from about 60% to 27–28%. 41 lyric tests and 256 glyph-core samples pass. Render spikes remain, up to 94ms after player return in the new runs; general frame pacing stays open. | `src/native_lyrics.rs`, `src/native_lyric_layout.rs`, `src/ambient_native.rs`, vendored `LyricInk`; `docs/PERFORMANCE.md` |

2026-11-06 follow-up: the 15-second account catalog check repeatedly resolved
local identities by scanning the whole library on the UI thread. Account
snapshot/catalog preparation now uses an index; an isolated 560-track fixture
reduced that lookup from 510 ms to 0.58 ms. `account_timer_callback` now makes
the full timer visible in diagnostics. A fresh real-playback trace is still
needed to confirm the periodic stalls are gone; unrelated render spikes remain
open. See `docs/PLAYER-TIMING-DEBUG.md`.

Later 2026-11-06 follow-up: native account preparation, cloud-restore merging,
playlist-import merging, account/session writes, backup/migration IO, and
local-file existence checks moved to workers. Library projection no longer
queries the filesystem. An isolated live catalog run measured account timer
p95 0.002 ms, maximum 0.868 ms over 441 samples; it did not include playback.
Startup store loading and UI model application remain on the main thread, and
the browser's preparation is not worker-backed. Real playback, restoration
and network-drive acceptance remain open.
The native audio-worker play/pause/seek/volume fixture passed with silent audio;
it does not exercise the complete UI selection path.

2026-11-04: a new `navigation_transition` run with the default backend also
failed static corner equality before motion, with a one-level RGB difference.
The actual renderer was not identified; see `docs/PERFORMANCE.md`. This does
not establish whether the clipping itself is wrong or the assertion is too strict.

## Already addressed: retain regression coverage

These are historical fixes, not newly discovered open bugs:

- 2026-11-02 mobile playlist rows, library count and YouTube Music file tagging:
  mobile playlist pages now render their `ListView` (`ui/main.slint`); Android
  MediaStore excludes non-music audio via `is_music != 0` (`src/android.rs`);
  `Store::library_tracks()` keeps local files plus liked/playlisted remote songs
  while hiding search history and remote duplicates of local files; downloads and
  tag sync persist `ytmusic:<id>` in the file comment and migrate saved keys to
  `yt://<id>` (`src/media_library.rs`, `src/native.rs`, `src/local_sync.rs`,
  `src/services.rs`, `src/account.rs`, `src/app.rs`, `src/web.rs`). Untracked
  local files without a YouTube Music identity still cannot sync across devices
  without private audio upload; Android device acceptance remains separate.

- 2026-10-29 sharp glyph artifacts: the prior halo comparison missed damage
  inside letters. Actual `LyricInk` captures reproduce the old GPU failure;
  stable solid/mask composition passes 128 Latin/Cyrillic/accent samples at
  1x/1.25x with zero wrong glyph cores. Full-view glow/effect/quality checks
  pass; synthetic cues and Windows fixture scope are in `docs/NATIVE-BRACCATO.md`.

- 2026-10-29 filled lead glow/edge sampling: the supplied phrase crop was
  reproduced with synthetic timings. Native lead-prefix glow now persists to
  line end; feather clips share a full-tile sampling origin. GPU/software/1.25x
  pixel and timing regressions pass. This fixed halo lifetime/sampling only;
  the separate sharp-letter repair above handles ink. `docs/NATIVE-BRACCATO.md`
  records the scope.

- 2026-10-28 native audio/lyric clock handoff (0.1.29): reproduced the reported
  Smooth Criminal line 2→3 snap using cached cues and the native polling cadence.
  Stale worker positions rewound presentation time and were mistaken for seeks.
  Timestamped audio positions, monotone clock slew and confirmed seek serials
  replace that cancellation path. GPU/software and silent native-audio replay
  pass; see `docs/NATIVE-BRACCATO.md`. This does not establish all-song/device
  acceptance.

- 2026-10-27 documentation pass (I-13 to I-17): `README.md` "Latest
  release" now matches `Cargo.toml` 0.1.27 and separates the published 0.1.16
  artifacts; tab styling and the native/browser lyric split were corrected.
  `docs/PARITY.md` marks the HTTP 403 as historical before 2026-10-21 and links
  `docs/PLAYBACK-RECOVERY.md`. The corrupted
  `docs/UI-REFINEMENT-PLAN.md` sentence was rewritten from
  `refinement-test.mjs`. All 31 widget scripts now have `npm` entries. Only the
  text was checked; no fixture or build was re-run.

- 2026-10-27 native line retargeting: every cue restarted the scroll from zero
  velocity, so a settled camera spent its first ~60 ms barely moving and a
  mid-move retarget braked at the cue boundary. `src/native_lyric_camera.rs`
  now retains position and forward velocity and launches front-loaded from rest;
  `examples/native_line_flow.rs` samples the real window. Keep the
  `VEY_LEGACY_LYRIC_SCROLL=1` A/B switch until the behaviour is accepted on the
  other platforms.

- 2026-10-25 native autoscroll: real wheel/drag input replaced the Flickable's
  initial content-y binding, so enabling follow did not restore its position.
  An observed target explicitly restores it on cue/resume changes. Native follow
  now indexes real word cues within wrapped lines; the actual-wheel fixture is
  `examples/native_autoscroll.rs`. See `docs/NATIVE-BRACCATO.md` for checks and
  the measured elimination of repeated source-model scans.

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

- Artwork-loading UI freeze: bounded decoding/workers and row-indexed models
  already existed, but each completed cover still rebuilt the sidebar playlist
  list and the whole queue. Cover completion now patches matching rows in place;
  per-track download progress no longer scans every page, and the playlist
  lookup is indexed once per full refresh. Windows release smoke on 2026-11-05
  opened a populated Library and Playlist after rapid route switches with covers
  visible. This was one no-playback session; audio soak and Android runtime remain
  unverified. `src/artwork.rs`, `src/app.rs`, `docs/PERFORMANCE.md`.
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
The 2026-10-30 user correction requests normal Google account-chooser v3 login.
YouTube uses a dedicated browser profile and read-only in-page requests; ordinary
profiles and exported cookies remain excluded. Scope: docs/MUSIC-ACCOUNTS.md.
The 2026-10-27 cleanup deleted `target/`, so every recorded fixture report under
`target/native-lyrics-check/` is gone; the committed JSON in `docs/` and the
tables in `docs/NATIVE-BRACCATO.md` remain the only copies.
