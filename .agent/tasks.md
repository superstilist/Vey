# Pending work and acceptance criteria

Planning snapshot: 2026-10-27, source version 0.1.27. The implementation backlog
below is suggested, not work currently in progress. Follow the next user's task
first. Issue IDs refer to `issues.md`; do not mark a task done from an old report.
The 2026-10-27 cleanup removed `target/`, `dist/` and `web/pkg/`, so anything that
needs those rebuilds first.

## Current Home UI request — implemented (2026-11-03)

Shared mixed-media Home shelves and actions are in `src/discovery.rs`,
`ui/home-feed.slint`, the native/Web controllers and catalog service. Windows,
WASM and browser checks are recorded in `docs/HOME-DISCOVERY.md`; device runtime
and a future release remain separate. Regress with `npm run test:home` and the
native `home_library` fixture before changing shelf navigation or caching.

## First: establish reliable platform behavior

| Task | Issues | Where to start | Done when |
| --- | --- | --- | --- |
| Broaden public playback acceptance | I-01 | `src/native.rs`, `src/server.rs`; `docs/PLAYBACK-RECOVERY.md` | Reported IDs now cache and decode fully. Check more catalog entries and audible/device playback separately; preserve bounded retry and actionable failures |
| Android device acceptance | I-10 | `docs/ANDROID-BUILD.md`, `src/lib.rs`, build script | Install on a real device/emulator; check launch, local playback, seek/volume, pause/resume, rotation, settings persistence and accessible paths; record environment |
| Android storage integration (partial) | I-03 | `src/android.rs`, `src/dialogs.rs`, source/import/export callbacks | Device scanning now uses permission and MediaStore (0.1.5); verify grant/deny/revoke on device. Remaining SAF selection and import/export must round trip actual files without desktop path assumptions |
| Linux build/runtime acceptance | I-11 | CI workflow dependencies; native platform gates | Fresh build/test and launch/audio/lyrics smoke on working Linux; record fallback limits and actual CI result if checked |
| Platform lyric authentication | I-04 | `src/challenge.rs`, `src/lyrics_service.rs` | Supported platform challenge/token acquisition and cache/retry work end to end without Google session capture or secret logging |
| Confirm the 0.1.27 camera on non-Windows targets | I-08 | `src/native_lyric_camera.rs`, `ui/native-lyrics.slint` | `examples/native_line_flow.rs` runs on browser/Android (or an equivalent check exists), and the launch/settle numbers are recorded per renderer. Then decide whether `VEY_LEGACY_LYRIC_SCROLL` can be removed |

## Next: finish user-facing feature gaps

| Task | Issues | Completion criteria |
| --- | --- | --- |
| Inline clip renderer | I-02 | Define platform scope; real video renders in player, follows audio seek/pause/rate, handles unavailable clips and releases resources |
| Browser local metadata/artwork | I-05 | Extract supported tags/covers from selected file bytes; update real title/artist/album/duration; preserve stable identity and handle malformed/large files |
| Browser reselection UX | I-06 | Explain unavailable session files and provide reliable relinking while preserving likes/playlists; do not promise unsupported permanent file access |
| Public details and pagination | I-07 | Audit existing service APIs first; implement actual album/artist/playlist navigation plus continuation, loading/error/empty states and cancellation of stale requests |
| Download progress/cancellation | I-07 | Progress reflects transferred data, cancel stops work, partial files are handled and failures are recoverable |
| Settings/interaction parity audit | I-07, I-09 | Compare original reference and current controls; inventory supported, deliberately excluded and missing items before adding callbacks |
| Lyric dialect fixes | I-09 | Start with a failing real fixture; parser/provider conversion preserves translations, romanization and backing vocals across native/web output |
| Web shell accessibility plan | I-12 | Assess current Slint support and alternative accessible UI approach; validate keyboard/screen-reader flows for the chosen scope |

## Documentation debt

Cleared 2026-10-27: I-13/I-14 (README release text, tab styling, lyric surface
split), I-15 (PARITY 403 marked historical), I-16 (refinement sentence
rewritten) and I-17 (all 31 widget scripts have `npm` entries). Text-only check;
no fixture or build was re-run.

## Maintenance

- Spotify account removal/public playlist import (2026-11-04): implementation
  replaces Spotify OAuth/private sync with public URL metadata → exact YTM
  matching → `yt://` playlist tracks. A 100-row embed response is rejected as
  potentially truncated. Old Spotify account fields are removed from local
  account storage; acceptance subtasks: `checklist/11-spotify-public-import.md`.
  Record fresh build/fixture/device evidence before closing remaining boxes.

- Playlist import/connections and Home/search (2026-10-30): done within the
  documented desktop browser-login/public import scope.
  Public YouTube playlist URL import was separate from choosing Vey/YouTube/Spotify
  connections; YouTube used normal Google v3 browser login, Spotify used OAuth (retired
  2026-11-04). Six Home
  compositions and debounced/cached search are included. Windows/WASM builds,
  267 Rust checks (known locale excluded), desktop/phone captures, browser import/
  search and live public imports (7/552 tracks) passed. Paths and exact limits:
  docs/MUSIC-ACCOUNTS.md. YouTube needs no Google app credentials; the dedicated
  browser tab stays open for sync. Private account/cloud acceptance remains separate.

- Glyph artifact follow-up (2026-10-29): done in copied `LyricInk`.
  The previous glow-edge work did not fix sharp letters. A real component probe
  reproduces 8,812 wrong glyph-core pixels in 48 legacy samples at fractional
  transforms. Stable solid ink/mask composition passes 128 expanded samples at
  1x/1.25x, full phrase GPU/software/reduced and effect/quality checks. Rust 256
  pass (known locale excluded), WASM check and Windows release pass. Details
  and performance scope: `docs/NATIVE-BRACCATO.md`.

- Filled lead glow / edge artifact (2026-10-29): done in native word
  paint/follow residency, copied Slint renderer and `ui/native-lyrics.slint`.
  Reported-phrase GPU/software/1.25x wave/wobble/reduced, effect, spatial and
  fractional-scale letter-quality fixtures pass. Rust 256 checks pass with
  known locale failure excluded; WASM check and Windows release build pass.
  Evidence and scope: `docs/NATIVE-BRACCATO.md`.

- Faster shell motion (2026-10-29): done in `ui/main.slint`, `motion.slint`,
  `src/navigation_fx.rs`, `lyrics_widget.rs` and the camera bundle. Live page
  surfaces, 100–180ms travel, immediate Settings and shorter panel/sheet motion.
  Rust 254 checks, GPU visual/latency, WASM camera/navigation/gesture and transport
  fixtures pass; Windows release built. Median route settle 491→185ms on the
  local fixture. Software corner assertion remains I-18; `docs/PERFORMANCE.md`.

- Player interaction performance (2026-10-29): done for indexed catalog/list
  dispatch and coalesced browser saves in `src/services.rs`, `track_model.rs`,
  `app.rs`, `web.rs` and `web/bridge.js`. Same-process 10k-track benchmark,
  254 Rust checks (known locale failure excluded), native release/WASM builds
  and browser interaction/lifecycle fixture verified; scope: `docs/PERFORMANCE.md`.

- Whole-player themes (0.1.33, 2026-10-29): 20 offline profiles, four new Vey
  styles, custom CSS/RICS roles and persisted lyrics-only opt-out. GPU, 360px,
  software and browser input/pixel fixtures pass; GSAP camera tests/navigation
  pass. Library: 202 pass, 7 ignored, existing blank catalog row test excluded
  and separately reproduced. Scope and references: docs/LYRIC-THEMES.md.

- Lyric appearance (0.1.32, 2026-10-29): shared draft panel, typography/effect
  controls, independent Braccato sample and fixed actions. Native GPU/software,
  Ukrainian 360px and browser input fixtures pass. Native CSS color/variable
  conversion and RICS diagnostics corrected; 199 library checks pass, with the
  existing blank catalog row failure preserved. See `docs/LYRIC-THEMES.md`.

- Lyric presets (0.1.31, 2026-10-28): implemented 13 new styles (16 total),
  preview/credit picker and shared saved/custom CSS restoration. Native GPU
  desktop/360px and software fixtures, all 16 browser profiles, targeted theme
  tests and WASM build pass. The existing locale-catalog test still fails.
  Scope and source references: `docs/LYRIC-THEMES.md`.

- Native ink/letter quality (0.1.30, 2026-10-28): done for the Windows bundled-font
  path. Shaped cluster geometry, common Text origins, neutral short-cue waves,
  physical-pixel feathers and translucent colors have GPU/software regressions
  at 1x/1.25x. 38 native tests pass; the unrelated locale test still fails.
  Source, reproduction and measured limits: `docs/NATIVE-BRACCATO.md`.

- Native polling/seek handoff (0.1.29, 2026-10-28): reported song transition
  reproduced; timestamped status, monotone presentation and explicit seek
  revisions replace inferred camera resets. Controlled GPU/software and muted
  native-worker replays pass, including confirmed seeks. Full library run has
  an unrelated unchanged locale-catalog failure; details in `docs/NATIVE-BRACCATO.md`.

- Native lyric stabilization (0.1.28, 2026-10-28): prepared geometry targets,
  bounded upcoming-row preparation, eased rest launch and first-row line-sync
  focus. Windows release line-flow/gap and GPU/software stress fixtures pass;
  34 native tests, nine clock/policy tests and WASM check pass. See
  `docs/NATIVE-BRACCATO.md` for numbers and the original-song acceptance limit.

- Native line-flow camera (2026-10-27): implemented and measured on Windows
  release builds; five unit tests plus `examples/native_line_flow.rs`. The A/B
  switch and the non-Windows gap are tracked above, not closed.
- Native autoscroll follow-up (2026-10-25): done. Real wheel/resume restores
  follow, wrapped rows follow source word cues, idle source scans are cached.
  Details and benchmark scope: `docs/NATIVE-BRACCATO.md`.
- Native Rust Braccato integration (2026-10-25): implemented using the copied
  local Rust-port snapshot; Windows native visual fixture and copied-crate tests
  passed. See `docs/NATIVE-BRACCATO.md` for exact checks and remaining
  platform/device acceptance boundaries.
- For fallback work (I-08), describe exact platform support instead of promising
  all upstream effects. Keep upstream vendored files unchanged.
- For performance work, measure before/after on the same workload/environment;
  retain save flushing, bounded artwork/event work, hidden-frame suspension and
  interruption behavior. Avoid unmeasured global speedup/FPS claims.
- Before release, use current source version, rebuild affected widget/WASM/native
  assets, run relevant checks, inspect UI and verify package checksums. Android
  development signing is documented; production signing requires a deliberate
  release decision and the appropriate existing key.

## Updating task state

For newly assigned work record only: task, status (planned/in progress/blocked/
done), affected paths, next concrete step and verification evidence. Keep this
file a backlog, not a running conversation log. Link substantive evidence in
`docs/` and move fixed issues out of the open list when fresh checks support it.
