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
| Rust build | `cargo build --locked` |
| Formatting check | `cargo fmt -- --check` |
| WASM compile check | `cargo check --locked --lib --target wasm32-unknown-unknown` |
| WASM + bindings | `./scripts/build-web.ps1 -Debug` |
| Browser backend | `cargo run --bin vey -- --web-server` (http://127.0.0.1:4173) |
| Native fixture screenshots | `cargo run --bin vey -- --screenshots` |
| Android bars/cutout layout fixtures (native simulation) | `cargo run --locked --example android_layout` |
| Volume fill pixel regression | `cargo run --locked --example volume_fill` |
| Output-device smoke | `cargo test --test native_services native_audio -- --ignored` |
| Live lyric auth check | `cargo run --bin vey -- --lyrics-auth-check` |
| Android APK | `./scripts/build-android.ps1` (setup: `docs/ANDROID-BUILD.md`) |
| Windows packaging | `./scripts/build-release.ps1` (see `docs/RELEASE.md`) |

Widget commands run in `tools/braccato-widget/`: `npm ci` for dependency setup,
`npm run build` to regenerate `web/braccato/`, `npm run serve` for the fixture
server, then in another terminal `npm test` or the relevant `npm run test:app`,
`test:clock`, `test:ambient`, `test:transitions`, `test:recovery`,
`test:stability`, `test:material`, `test:appearance`, `test:phone`, `test:playlists`, `test:sync`,
`test:video`, `test:syllables` or `test:refinement` script. Syllable checks run a
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
scripts/clean-artifacts.ps1 -RequireAndroid to retain only the current Cargo
version under dist/, remove root build logs, web/pkg, docs/smoke-data and target/
(including debug and release caches). Every deletion checks checkout containment
and refuses tracked files or links. -KeepBuildCache retains target/ for repeated
builds. This never deletes GitHub releases, source fixtures, reference checkouts
or external signing keys/user state. Build scripts recreate generated assets.

The widget builder publishes changed output files by replacement, avoiding
Windows mapped-file overwrite errors when embedding generated assets.
