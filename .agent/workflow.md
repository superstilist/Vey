# Efficient development

Run commands from the project root in PowerShell unless stated otherwise.
`target/`, `dist/`, `web/pkg/` and `tools/braccato-widget/node_modules/` were
removed on 2026-10-27, so the first build after that is a cold build.

## Find only what you need

1. Read `AGENTS.md` and `.agent/project.md`, then check `git status --short`.
2. Choose the subsystem from the source map. Use `rg -n 'symbol' src ui` or
   `rg --files src ui tests scripts tools -g '!**/node_modules/**'`.
3. Read narrow source ranges; expand only when the call chain requires it.
   Avoid broad scans of `target/`, `dist/`, `web/pkg/`, vendored sources,
   lockfiles, screenshots and generated bundles.
4. Edit source, regenerate affected assets, run the relevant checks once.
   Report what actually ran and any unverified behavior.

## Commands

| Purpose | Command |
| --- | --- |
| Native launch | `cargo run --bin vey` |
| Rust tests | `cargo test --locked --lib --tests` |
| Account sync and storage | `cargo test --locked --lib account::tests`; live fixture/browser checks: `docs/ACCOUNTS.md` |
| Playlist import and Home layouts | `cargo run --locked --example home_library -- target/NEW_OUTPUT_DIRECTORY` (`--mobile` optional); docs/MUSIC-ACCOUNTS.md |
| Library categories and player queue layouts | `cargo run --locked --example music_library_layout -- target/NEW_OUTPUT_DIRECTORY`; generated fixture art and captures only |
| Rust build | `cargo build --locked` |
| Formatting check | `cargo fmt -- --check` |
| WASM compile check | `cargo check --locked --lib --target wasm32-unknown-unknown` |
| WASM + bindings | `./scripts/build-web.ps1 -Debug` |
| Browser backend | `cargo run --bin vey -- --web-server` (http://127.0.0.1:4173) |
| Native fixture screenshots | `cargo run --bin vey -- --screenshots` |
| Copied Rust Braccato native visual/behavior fixture | `cargo run --locked --example native_lyrics`; inspect `target/native-lyrics-check/` |
| Native autoscroll, real wheel/resume and source-read counter | `cargo run --locked --example native_autoscroll` |
| Continuous 2x native timing, grouped cues and interruptible retarget | `cargo run --locked --example native_lyric_timing` |
| Word deadline pixels and exact scroll/stage handoff | `cargo run --locked --example native_lyric_boundary` |
| Wrapped previous-row completion before next-row fill | `cargo run --locked --example native_lyric_wrapped_boundary` |
| Long hyphenated/unbroken text, mobile/stage bounds and cue continuation | `cargo run --locked --example native_lyric_overflow` |
| Native Braccato reference typography/focus/sweep visuals | `cargo run --locked --example native_lyric_effects` |
| Focus crossfade pixels, no additive duplicate and reduced motion | `cargo run --locked --example native_lyric_handoff` |
| Whole-word/syllable motion, unsung-word pixels and independent letter waves | `cargo run --locked --example native_lyric_wobble` |
| Continuous repeated-word/backing fill with no backwards ink | `cargo run --locked --example native_fill_continuity` |
| Spatial fill inside glyphs, Latin/Cyrillic and reduced motion | `cargo run --locked --example native_fill_spatial`; add `-- --motion` for short cues with wobble/prefix; repeat with `$env:SLINT_BACKEND='winit-software'` |
| Active/expired glow, atlas feather and letter/whole-run fill conformance | `cargo run --locked --example native_fill_effects` |
| Reported phrase, persistent filled-prefix halo and sharp ink | `cargo run --locked --example native_filled_glow -- target/NEW_OUTPUT_DIRECTORY --assert`; optional `--wave`, `--wobble`, `--reduced`; repeat with software or 1.25x scale |
| Actual glyph-core artifacts under fractional placement/transforms | `cargo run --locked --example native_ink_artifacts -- target/NEW_OUTPUT_DIRECTORY --assert`; repeat at `SLINT_SCALE_FACTOR=1.25`; `--legacy` reproduces the old GPU path without assertions |
| Kerning/ligatures/accents, letter endpoints and translucent fill | `cargo run --locked --example native_lyric_quality -- NEW_OUTPUT_DIRECTORY`; repeat with `SLINT_BACKEND=winit-software` and `SLINT_SCALE_FACTOR=1.25`; use a new directory each time |
| Real line-retarget velocity, prompt launch and exact settling | `cargo run --release --locked --example native_line_flow -- continuous`; compare `-- legacy --legacy` with the same executable; repeat with software renderer |
| Long cue gap, delayed UI frame, first wrapped row and prewarming | `cargo run --release --locked --example native_lyric_gap -- label --wrapped`; add `--mobile --capture` for a local visual fixture |
| Native audio polling phase and confirmed-seek regression | `cargo run --release --locked --example native_lyric_song -- tests/fixtures/native_audio_polling.json target/native-lyrics-check/polling-run --sampled-audio --assert-smooth`; add `--confirm-seek`; optional `--audio=ABSOLUTE_WAV_PATH` uses the real muted native worker |
| Continuous camera positions, rapid retarget and no overshoot | `cargo run --locked --example native_lyric_motion` |
| Imported fonts, custom themes and filled-only glow pixels | `cargo run --locked --example native_lyric_custom` |
| All lyric presets, saved/custom CSS and chooser layout | `cargo run --locked --example lyric_themes -- target/theme-preview-new`; add `--mobile`; repeat with software renderer |
| Backing-vocal phrase/wrapped fill pixels, including opt-in letter waves | `cargo run --locked --example native_lyric_backing` |
| Native render workload timings | `cargo run --locked --example native_lyric_performance -- label`; JSON under `target/native-lyrics-check/` |
| Same native workload with word wobble enabled | `cargo run --locked --example native_lyric_performance -- label --wobble` |
| Release event-loop/model-read benchmark without GPU snapshot readback | `cargo run --release --locked --example native_lyric_performance -- label --wobble --no-snapshots`; repeat three times per renderer |
| Cold/repeated player entry and dense line handoff stalls | `cargo run --release --locked --example native_lyric_stutter -- label --audio --soak --no-blur`; lock `SLINT_SCALE_FACTOR=1`, compare `VEY_LEGACY_LYRIC_CADENCE=1` with unset using the same executable; heartbeat and rendered clock reports under `target/native-lyrics-check/stutter-*.json` |
| Native executable release build | `cargo build --release --bin vey --locked`; output `target/release/vey.exe` |
| Windows release package | `./scripts/build-release.ps1`; `-SkipBuild` repackages the existing executable. ZIP includes `MUSIC-ACCOUNTS.md` alongside the release README and notices |
| Copied renderer unit checks | `cargo test --locked --manifest-path third-party/braccato-rust/Cargo.toml --target-dir target -p braccato-slint -p braccato-engine -p braccato-types -p braccato-themes` |
| Android bars/cutout layout fixtures (native simulation) | `cargo run --locked --example android_layout` |
| Windows search focus regression (same MP4 fixture as `video_surface`) | `cargo run --locked --example search_input` |
| Volume fill pixel regression | `cargo run --locked --example volume_fill` |
| Page direction/clipping/settings visual fixture | `cargo run --locked --example navigation_transition -- target/NEW_OUTPUT_DIRECTORY`; optional `VEY_LEGACY_NAVIGATION_CAPTURE=1` enables snapshot blur for diagnostics |
| Native route launch/settle latency | `cargo run --release --locked --example navigation_latency -- target/NEW_OUTPUT_DIRECTORY`; compare matching workloads/renderers |
| Native ambient cadence during navigation | `cargo run --locked --example ambient_navigation`; set `VEY_LEGACY_AMBIENT_TRAVEL=1` for the prior cadence |
| Browser live-navigation/readback regression | Built WASM and fixture host 4181: `npm run test:navigation-latency -- ../../target/NEW_OUTPUT_DIRECTORY` |
| Song row, rounded cover corner and badge preview | `cargo run --locked --example song_layout_preview` |
| Shared lyric-settings menu preview | `cargo run --locked --example lyrics_settings_preview` |
| Browser syllable document from the real parser | `cargo run --locked --example lyric_syllables`; writes `target/lyric-syllables.json` (also run automatically by `npm run test:syllables`) |
| LRC parser throughput microbenchmark | `cargo run --release --locked --example lyrics_bench` |
| Catalog, playlist, cover and like dispatch microbenchmarks | `cargo run --release --locked --example player_performance`; scope in `docs/PERFORMANCE.md` |
| Browser transport, likes and coalesced-save lifecycle | With built WASM and fixture host 4181: `npm run test:player-performance -- ../../target/NEW_OUTPUT_DIRECTORY` in `tools/braccato-widget` |
| Native Windows FFmpeg video decode/transport check | `cargo run --locked --example video_surface`; needs `target/video-surface-fixture.mp4` from `npm run test:video` and FFmpeg on PATH |
| Opt-in public catalog and media probe | `cargo run --locked --example yt_music_check -- "oasis wonderwall" [--audio]` |
| Opt-in media resolver/cache probe | `cargo run --locked --example stream_probe -- [--cache] VIDEO_ID [VIDEO_ID ...]` |
| Output-device smoke | `cargo test --test native_services native_audio -- --ignored` |
| Live lyric auth check | `cargo run --bin vey -- --lyrics-auth-check` |
| Android APK | `./scripts/build-android.ps1` (setup: `docs/ANDROID-BUILD.md`) |
| Windows packaging | `./scripts/build-release.ps1` (see `docs/RELEASE.md`) |

`stream_probe`, `yt_music_check` and `playlist_import` are opt-in examples that
touch live public services; they never print signed media URLs or credentials. Do not treat a
successful probe as playback acceptance.

`youtube_browser_login` is an opt-in visible normal Google v3 login launch check.
It requires isolated `PLAYERVF_DATA_DIR`, never enters authentication data and
does not persist a Vey account connection. The product Connect action does.
Helper/transport fixtures: `npm run test:youtube-browser`; docs/MUSIC-ACCOUNTS.md.

Clear `SLINT_BACKEND` after an explicit software-renderer check so later runs
use the normal backend selection.
The performance fixture restores only its own minimized test window without
activation; production playback still suspends while minimized. Compare matching
release workloads and separate event-loop intervals from snapshot costs or FPS.
`VEY_DEBUG_PLAYER=1` writes bounded asynchronous player timing data to
`%APPDATA%\PlayerVF-Rust\debug\player-timing-*.jsonl`; see
`docs/PLAYER-TIMING-DEBUG.md`. It measures per-route render cadence, renderer
callback duration, lyric update/fill timing, startup store-load timing, and
synchronous navigation-handler timing without recording track names.
`VEY_TRACE_LYRIC_FRAMES=1` remains a compatibility alias. Clear either flag
after diagnostics.
`VEY_LEGACY_LYRIC_SCROLL=1` restores the pre-0.1.27
ease-restart scroll for A/B checks; clear it after use. Warm steady playback
alone does not cover player entry, retargeting or line-boundary stalls.

## Widget commands

GSAP camera source is tools/braccato-widget/player-motion-source.js; the widget
build generates web/player-motion.js. Run npm run build before build-web.ps1.
Whole-player captures: cargo run --locked --example player_themes --
NEW_OUTPUT_DIRECTORY, optionally --mobile or SLINT_BACKEND=winit-software.
test:navigation accepts a new output directory to avoid rewriting tracked shots.

Run these in `tools/braccato-widget/`: `npm ci` for dependency setup,
`npm run build` to regenerate `web/braccato/`, `npm run serve` for the fixture
server on 127.0.0.1:4181, then in another terminal `npm test` or one of:

`test:app`, `test:clock`, `test:ambient`, `test:transitions`, `test:recovery`,
`test:stability`, `test:material`, `test:appearance`, `test:phone`,
`test:playlists`, `test:sync`, `test:syllables`, `test:video`, `test:search`,
`test:refinement`, `test:cue-boundary`, `test:custom-theme`,
`test:camera`, `test:navigation`, `test:stage-navigator`.
`test:theme-presets` takes the native fixture's catalog JSON and a new output
directory; see `docs/LYRIC-THEMES.md`. It uses only the 4181 fixture host.
`test:lyric-appearance -- NEW_OUTPUT_DIRECTORY` checks the shared appearance
dialog in the WASM app with isolated browser storage and fixture media.
Native input/preview checks: `cargo run --locked --example lyric_appearance --
NEW_OUTPUT_DIRECTORY`; repeat with `--mobile --uk` and software rendering.

Every script has an npm entry; the alias is the supported way to run it. They
do not all share one server:

- Widget fixture server on 4181 only (`npm run serve`): `test:cue-boundary`,
  `test:custom-theme`, `test:navigation`, `test:stage-navigator`.
- Account Worker fixture on 4879 (`VEY_API_URL`, see `docs/ACCOUNTS.md`):
  `test:account-site`, `test:account-web-qr`, and — together with 4181 —
  `test:account-oauth`, `test:account-qr`, `test:account-transfer`,
  `test:account-wasm`, `test:site-material`.
- Composer Worker fixtures on 4891 (site) and 4892 (API): `test:composer`.
- No server: `test:camera` reads `web/player-motion.js` and renders its own page.

Syllable checks run a Rust example (`lyric_syllables`) to generate the actual
display document. Video/refinement checks generate a small fixture with the
FFmpeg executable; run `test:video` first. Check script setup before running;
application checks can also require the built WASM/backend.

## Match checks to the change

- Documentation only: inspect the diff and check referenced paths/commands;
  no application rebuild required.
- Rust behavior: focused unit/integration tests; build when target wiring or
  Slint interfaces change. Existing fixture suites live in `tests/`.
- Shared/native/web controller changes: check the affected native and WASM
  targets. `src/lib.rs` contains platform gates.
- Widget changes: rebuild the bundle and run the relevant browser checks.
- UI changes: native screenshot review and reachable browser/manual checks
  for affected platforms and sizes. Compilation alone is not visual validation.
- Native lyric scroll changes: unit tests in `src/native_lyric_camera.rs` plus
  `examples/native_line_flow.rs` and `native_lyric_motion`; compare the legacy
  switch before claiming an improvement.
- Hardware, live-network and device tests are separate from fixture passes.
  Prior reports are historical; never present them as fresh verification.
- For stateful smoke tests, set `PLAYERVF_DATA_DIR` to an isolated fixture
  directory before launch. Keep credentials and real user data out of reports.

CI definitions: `.github/workflows/rust.yml` (desktop Windows/Ubuntu tests and
builds, WASM build, Android ARM64 library build). Do not infer local/device
runtime success from compilation or a supplied CI workflow.

## Release cleanup

Build the Windows ZIP and Android APK before cleanup. Run
scripts/clean-artifacts.ps1 -RequireAndroid to retain every platform artifact for the current Cargo
version under dist/, remove root build logs, web/pkg, docs/smoke-data and target/
(including debug and release caches). Every deletion checks checkout containment
and refuses tracked files or links. -KeepBuildCache retains target/ for repeated
builds. This never deletes GitHub releases, source fixtures, reference checkouts
or external signing keys/user state. Build scripts recreate generated assets.

The widget builder publishes changed output files by replacement, avoiding
Windows mapped-file overwrite errors when embedding generated assets.

## Additional platform packaging

- Web: scripts/package-web.ps1 builds optimized WASM/bindings and archives the
  web/ directory with launch helpers and notices. -SkipBuild packages existing
  generated assets. Online features require the same-origin Vey backend.
- Native Linux: bash scripts/build-linux.sh produces tar.gz/.deb plus hashes.
  Build on a native Linux host; current x64 packages target Mint 22/Ubuntu
  24.04+ (glibc 2.39). scripts/prepare-linux-user-deps.sh optionally extracts
  Mint/Ubuntu development packages into a user cache without sudo. Source its
  env.sh before building. No system packages or services are changed by it.
- Android x86_64: scripts/build-android.ps1 -Architecture x64; default arm64 is
  unchanged. ABI/version/non-debuggable checks guard output naming.
- Cleanup preserves all Vey-VERSION-* artifacts for the current version.
  Published tags and earlier GitHub release assets are retained.
- Native macOS/iOS requires an Apple host/SDK; do not label browser compatibility
  as a native Apple release. Package checks do not establish device/UI runtime.
