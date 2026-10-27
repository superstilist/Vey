# Efficient development

Run commands from the project root in PowerShell unless stated otherwise.

## Find only what you need

1. Read `AGENTS.md` and `.agent/project.md`, then check `git status --short`.
2. Choose the subsystem from the source map. Use `rg -n 'symbol' src ui` or
   `rg --files src ui tests scripts tools -g '!**/node_modules/**'`.
3. Read narrow source ranges; expand only when the call chain requires it.
   Avoid broad scans of `target/`, `dist/`, `web/pkg/`, `.reference/`, vendored
   sources, lockfiles, screenshots and generated bundles.
4. Edit source, regenerate affected assets, run the relevant checks once.
   Report what actually ran and any unverified behavior.

## Commands

| Purpose | Command |
| --- | --- |
| Native launch | `cargo run --bin vey` |
| Rust tests | `cargo test --locked --lib --tests` |
| Account sync and storage | `cargo test --locked --lib account::tests`; live fixture/browser checks: `docs/ACCOUNTS.md` |
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
| Continuous camera positions, rapid retarget and no overshoot | `cargo run --locked --example native_lyric_motion` |
| Imported fonts, custom themes and filled-only glow pixels | `cargo run --locked --example native_lyric_custom` |
| Backing-vocal phrase/wrapped fill pixels, including opt-in letter waves | `cargo run --locked --example native_lyric_backing` |
| Native render workload timings | `cargo run --locked --example native_lyric_performance -- label`; JSON under `target/native-lyrics-check/` |
| Same native workload with word wobble enabled | `cargo run --locked --example native_lyric_performance -- label --wobble` |
| Release event-loop/model-read benchmark without GPU snapshot readback | `cargo run --release --locked --example native_lyric_performance -- label --wobble --no-snapshots`; repeat three times per renderer |
| Native executable release build | `cargo build --release --bin vey --locked`; output `target/release/vey.exe` |
| Copied renderer unit checks | `cargo test --locked --manifest-path third-party/braccato-rust/Cargo.toml --target-dir target -p braccato-slint -p braccato-engine -p braccato-types -p braccato-themes` |
| Android bars/cutout layout fixtures (native simulation) | `cargo run --locked --example android_layout` |
| Windows search focus regression (generated video fixture required) | `cargo run --locked --example search_input` |
| Volume fill pixel regression | `cargo run --locked --example volume_fill` |
| Output-device smoke | `cargo test --test native_services native_audio -- --ignored` |
| Live lyric auth check | `cargo run --bin vey -- --lyrics-auth-check` |
| Android APK | `./scripts/build-android.ps1` (setup: `docs/ANDROID-BUILD.md`) |
| Windows packaging | `./scripts/build-release.ps1` (see `docs/RELEASE.md`) |

Clear `SLINT_BACKEND` after an explicit software-renderer check so later runs
use the normal backend selection.
The performance fixture restores only its own minimized test window without
activation; production playback still suspends while minimized. Compare matching
release workloads and separate event-loop intervals from snapshot costs or FPS.

Widget commands run in `tools/braccato-widget/`: `npm ci` for dependency setup,
`npm run build` to regenerate `web/braccato/`, `npm run serve` for the fixture
server, then in another terminal `npm test` or the relevant `npm run test:app`,
`test:clock`, `test:ambient`, `test:transitions`, `test:recovery`,
`test:stability`, `test:material`, `test:appearance`, `test:phone`, `test:playlists`, `test:sync`,
`test:video`, `test:search`, `test:syllables` or `test:refinement` script. Syllable checks run a
Rust example to generate the actual display document. Video/refinement checks generate a
small fixture with the FFmpeg executable; run `test:video` first. Check script setup
before running; application checks can also require the built WASM/backend.

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
- Hardware, live-network and device tests are separate from fixture passes.
  Prior reports are historical; never present them as fresh verification.
- For stateful smoke tests, set `PLAYERVF_DATA_DIR` to an isolated fixture
  directory before launch. Keep credentials and real user data out of reports.

CI definitions: `.github/workflows/rust.yml`. Do not infer local/device runtime
success from compilation or a supplied CI workflow.

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
