# Native Rust Braccato

Native Vey imports `LyricsView` from the copied Rust Braccato crate in
`third-party/braccato-rust/crates/slint`. Source provenance and host adaptations
are in `third-party/BRACCATO-RUST-UPSTREAM.md`.
The original reference working tree and the previous JavaScript snapshot
are preserved. Browser/WASM still uses the original iframe integration.

`src/native_lyrics.rs` adapts Vey's canonical display rows to Braccato models;
the Rust crate supplies wrapping, glyph sweep plans, cached glow masks,
letter waves, wobble, stage roles and instrumental frames. Vey's wrapper
binds palette, layout, playback, seek, reduced motion and manual-scroll resume.
Translation and romanization visibility remains controlled by display clones,
without altering cached/exported sources.

## Stable per-fill models (2026-11-06)

`LyricInk` keeps its original conditional neutral Text path. A clock-dependent
integer model previously replaced feather delegates as its length changed.
During partial coverage, pool capacity now depends only
on static word/glyph ramp bounds and layout width, capped at 64 cells. Normal
narrow feathers use only the needed capacity. Playback changes pixel-aligned
coverage and color without resizing that pool. Solid words release the partial
body; retaining hidden pools for every viewport word regressed render time.
Retaining hidden neutral Text during partial coverage also regressed the measured
viewport workload, so the final change stabilizes only the per-fill repeater.

Neutral Text keeps its original unclipped origin. Rendering it inside a feather
clip changed fractional ligature rasterization at wave handoffs; the original
unclipped run preserves the established glyph reference. Canonical cues,
fragment offsets, wrapping, fades and reduced-motion fill are unchanged.

Fresh Windows checks on the final ink code, with scale fixed explicitly:

- `native_lyric_quality`: eight short/long shape cases and translucent-color
  cases passed; neutral handoff differences were at most one pixel.
- `native_fill_spatial --motion`: Latin/Cyrillic, punctuation, wobble and
  reduced-motion cases passed. Its oracle now follows the 1000ms timed cue,
  combines its fragments and keeps it unwrapped at 48px. The old oracle
  selected the first row's prefix after a 72px cue wrapped.
- `native_fill_continuity`: 8,129 forward pixel transitions, zero backwards
  transitions, and visible advancement at 37 of 40 sampled cue positions.
- `native_ink_artifacts --assert`: 256 narrow/broad, GPU, 1.25x and software
  samples passed with no wrong solid glyph-core pixels.

Logs: `target/{quality,spatial,continuity}-final-scale1-1106.log` and
`target/ink-final-{scale1-narrow,scale1-broad,scale125,scale1-software}-1106.log`.

These are glyph/coverage checks. See `docs/PERFORMANCE.md` for separate timing
measurements; sampled cue screenshots are not an FPS measurement.
The native host now samples visible playing fill time every 8ms through a light
pass, retaining a 16ms deadline for full processing. Same-executable audio runs
and the remaining frame-pacing limits are recorded there. Final-source 2x,
retarget, paused-seek and continuity fixtures passed after this host change.

## Sharp-letter artifact follow-up (2026-10-29)

The filled-glow change below did not resolve the reported sharp-letter artifacts.
Its halo-on/off comparison used the same sharp renderer on both sides, so it
could not detect damage produced by that renderer itself.

`examples/native_ink_artifacts.rs` now imports the actual `LyricInk` component
and compares partial fill with solid base/highlight reference images under
fractional placement and 1.0/1.011/1.025/1.04 transforms. The old GPU brush path
reproduces slivered/dark letter interiors in “bedroom” and “AVATAR”: 8,812 wrong
fully-sung/unsung core pixels over 48 samples, with maximum error 160/255.

Native ink now defaults to `stable-fill`: solid, fully shaped text painted
through fixed physical-pixel fill clips. GPU ink is composed into a local
surface before word/letter/line transforms. Empty, partial and completed ink
keep that same composition path, avoiding a renderer handoff. The common glyph
origin and source timings stay intact. The previous gradient brush remains only
as the probe's explicit `--legacy` diagnostic; software retains its solid path.

Commands: `cargo run --locked --example native_ink_artifacts --
target/NEW_OUTPUT_DIRECTORY --assert`, or `--legacy` to reproduce the old path.
Expanded stable runs include Latin, Cyrillic, ligatures and combining accents:
64 samples at 1x and 64 at 1.25x both report zero wrong core pixels outside
the feather. These are glyph-rendering checks, separate from the halo
measurements below. The old-path comparison used the original six-case,
48-sample fixture; its broken interiors were also inspected in screenshots.

The full phrase fixture passes with wave/wobble on GPU and software and with
reduced motion. The effect fixture reports zero backing-cue residue, zero
transparent-margin violations and zero letter-return deficit; identical-radius
glow feathers differ by at most 1/255. The fractional-scale quality fixture
passes short/long cues, glyph endpoints, ligatures, marks and translucent colors.
Rust lib/integration checks: 256 pass, eight ignored, one known locale-catalog
test excluded. WASM compile and Windows release build pass. These use synthetic
cues on Windows; the user's original track and other devices are not certified.

The optimized 300-line/2,400-word wobble fixture, without snapshot readback,
records 80 timer samples: median 16.32ms, p95 17.15ms, and 78 word-model reads
(`performance-stable-fill-release.json` under `target/native-lyrics-check`).
This is a current event-loop observation, not a matched speedup comparison or
an FPS guarantee. A debug run during release compilation measured 66.13/92.77ms
and is not comparable to the optimized run.

Rebuilt `target/release/vey.exe` (0.1.33, 2026-10-29) SHA-256:
`08CFB381E9A628AB9D98224713C9A7F82ED725789AC90EB22E1BBF91C433B4C0`.
Close an already running Vey and launch this binary to load the repair. No
package/release was published.

## Filled-prefix glow and edge sampling (2026-10-29)

The supplied “So she ran into the bedroom” crop was reproduced with explicit
fixture cues. Completed lead words previously lost their halo at each word's
cue end, even while the source line remained active. The native host now opts
into `filled-glow`: a soft halo remains on the sung lead prefix until the line
ends. Secondary layers and backing cues keep their existing cue-bounded release.
The existing Glow control and reduced-motion setting remain authoritative.

Glow feather strips now clip one complete radius tile using a common image
origin and physical-pixel boundaries. Independently cropped/rescaled strips
could expose different bilinear sampling phases at their seams. Tint/atlas
pixels remain immutable; the fix does not create per-frame glow uploads or
Gaussian filters. Sharp text still uses the same shaped run and fill clock.

Lead paint deadlines are shared by the follow index, stale-result checks and
release path, and extend through the line. A late seek can prepare its sung
prefix without reviving finished background cues. Assets still use the bounded
worker and existing generation checks; canonical word/syllable times are unchanged.

Reproduction: `cargo run --locked --example native_filled_glow --
target/NEW_OUTPUT_DIRECTORY --assert`. Optional `--wave`, `--wobble` and
`--reduced` modes cover presentation settings. Use new output directories for
software and `SLINT_SCALE_FACTOR=1.25` runs. This fixture uses synthetic timings
for the supplied phrase, not the user's original track payload.

At the 1.3s sample, the sung-prefix background halo changed 4,287 pixels, versus
zero before the change. Across the paused samples no sharp ink pixels were
erased, and the halo had zero residual pixels after the line ended. The existing
effect fixture reports zero expired backing-cue glow, zero transparent-margin
violations, zero letter-return fill deficit, and an identical-radius feather
maximum difference of 1/255. Rust checks: 256 pass, eight ignored, known
locale-catalog failure excluded. WASM compile check passes; browser lyrics keep
their existing iframe renderer.

The phrase fixture passed GPU/software at 1x, GPU at 1.25x with letter waves
and wobble, and reduced motion (zero halo). Spatial fill passed Latin/Cyrillic
and phrase samples on both renderers, including reduced motion. The existing
short/long cue, kerning/ligature/combining-mark and translucent-color quality
fixture passed at 1.25x. An initial concurrent GPU spatial run could not take a
FemtoVG snapshot during startup; its isolated rerun passed. These are Windows
fixture results, not original-song or other-device acceptance.

`cargo build --release --locked --bin vey` completed after the changes; the
updated executable is `target/release/vey.exe`. No package/release was published.

The adapter explicitly removes the Rust demo's optional synthetic 50ms
line-word stagger. Line-synced rows use whole shaped text runs after native
wrapping; only actual word/syllable cues drive word effects. Wrapped fragments
retain the original source index and timestamp. Timed romanization seeks
resolve against its own part timings. Overlapping source lines stay active
independently. Existing offsets and rate-aware sample interpolation are retained.

Wrapping/models are rebuilt on source, viewport, size, weight or layout changes,
not on playback samples. The 16ms view clock stops updating when the lyric tab is hidden
or the window is minimized. Unchanged paused rows are not rewritten. Glyph/glow
preparation uses a dedicated worker with one outstanding word, prioritizing active
and imminent cues. Only owned RGBA and glyph timing data cross threads; Slint
image/model upload stays on the UI thread. Layout generations and seek windows
reject stale results. Focus masks run independently so dense syllable cues cannot
starve nearby-line blur. The cache and glyph-plan queue have bounded storage.
Native ambient textures use the existing Rust adapter on Windows as well as Android/Linux;
no lyric child HWND, matching-background paint or lyric IPC is involved.

Autoscroll retains an observed target even after real wheel/drag input replaces
the Flickable's initial content-y binding. Resume explicitly reapplies that
target. A retained source/visual index follows real word cues into wrapped rows;
line-only input retains line sync. Fast adjacent lines no longer wait for the
old lookahead group's last line before becoming the scroll anchor.

The controller caches romanization presence and source-to-visual ranges. Playback
visits active/previous sources and resident glow rows, not every dormant UI row.
Stage roles update only on stage transitions; scrolling mode does not allocate
stage-role vectors per frame. Glyph preparation prioritizes the current sung
fragment after a seek before older parts of a long line.

`cargo run --locked --example native_autoscroll` dispatches actual native wheel
events and checks normal 100ms samples, manual suspension, resume, and a long
word-timed mobile line. A 300-line fixture counted 15,600 canonical model row
reads over two seconds before caching and 0 after, with the same input cadence
and reduced-motion setting. This counts eliminated source reads, not overall FPS.

Supported CSS/RICS declarations map to native appearance controls. Arbitrary
CSS/DOM execution remains browser-only.

Reference appearance refinement: desktop lyrics use Satoshi at 48px, weight 700,
1.333 line spacing and 32px source-line padding, matching the upstream typography
defaults. Noto Sans covers Cyrillic and Noto Sans JP covers CJK. Native word runs
retain supplied spaces as nonbreaking display spaces so adjacent timed parts do
not collapse word gaps; actual syllable boundaries and canonical data stay intact.
The active lyric is sharp; nearby inactive lyrics use cached soft-focus glyph
masks. Palette/background still follow Vey's artwork preferences.

Focus masks are built by a dedicated worker. The UI queues at most one request,
uploads completed RGBA, rejects stale generations after document/layout changes,
and keeps at most 14 nearby rows. The worker's bounded glyph cache is reused across
tracks/resizes. There is no per-frame SVG parsing or Gaussian filtering for focus
blur on the UI thread. Paused, unchanged focus produces no new raster requests.
`cargo run --locked --example native_lyric_effects` captures the supplied English
and Ukrainian reference phrases, focus handoff, and mobile/reduced-motion views.
Browser theme behavior remains separate. Android/Linux/macOS device acceptance
must not be inferred from Windows screenshots or compilation.

Focus/performance refinement, 2026-10-25: native unsung ink is dimmer during
playback, scrolling uses Braccato's cubic easing, and overlapping active vocals
remain sharp. Word glow rasterization and glyph planning moved off the event loop.
All 13 focused `native_lyric` tests passed, including identical glow pixels/glyph
timing and rejection after reload, seek or reduced motion. The four-phase
`native_lyric_effects` fixture passed; English/Ukrainian desktop and mobile
reduced-motion screenshots were inspected. WASM compilation and formatting passed.
The native autoscroll fixture also passed actual wheel input, resume and wrapped
cue follow, with zero canonical source-row reads during its two-second sample.
This establishes fixture correctness, not a measured FPS improvement.

Further motion refinement: scrolling now starts up to 500 ms before an upcoming
source line or wrapped word row while sung activation remains timestamp-exact.
The existing lookahead setting can shorten/disable that lead. Pause and reduced
motion use the exact current cue. Small late media samples slew toward the source
clock using upstream's 60 ms resync boundary; seeks and pause remain immediate.
The word worker prepares a four-radius glow atlas (12.8, 9.6, 6.4, 3.2 px) and
the GPU crossfades adjacent radii down to the foreground's zero-radius state.
Glow follows the word's highlight sweep instead of lighting every unsung letter.
Glyph plans use normalized positions inside a stable whole-word width, preventing
layout changes when asynchronous plans arrive. Browser upstream sources remain
unchanged; native cached-radius interpolation approximates the browser's filter.
Validation for this refinement: 160 library tests passed, 7 environment-dependent
tests ignored, and the previously documented account-label localization test
excluded. The native wheel/resume fixture also passed its new live-playback check:
scrolling moved to the upcoming line before 4 s while only the 0 s source remained
sung. Its canonical source-read counter stayed at zero over the two-second sample.
The seven-phase effects fixture passed; attack/contract/settle glow frames and
desktop/mobile focus frames were visually inspected. WASM compilation passed.
The optimized Windows executable built successfully and passed the isolated
18-frame UI smoke; its player screenshot was inspected. The Windows ZIP was
regenerated with this refinement. Physical-device acceptance remains separate.

## Custom appearance and filled-only glow

Open the lyric controls → **Вигляд слів** (Lyrics appearance). Import a desktop
TTF/OTF font, select a preset or paste CSS/RICS, then Apply. CSS and the selected
family persist in settings. Fonts are copied into the app's `lyric-fonts/`
directory and restored at startup; moving/deleting the original file is safe.
Font files are local and are not included in account metadata sync. The desktop
importer accepts up to eight fonts, each at most 20 MiB. Android's existing file
picker limitation still applies; its import action is hidden.

Native text, wrapping, glow and inactive masks use the chosen font and weight.
Missing script coverage falls back to the bundled Satoshi/Noto fonts. Importing
a font invalidates prepared glyph/layout data and updates both worker caches.
Slint's versioned `unstable-fontique-011` feature supplies runtime registration;
the lockfile retains the tested Slint version.

Supported appearance properties include font family/size/weight, active/inactive
and glow colors, line height, line scales, inactive blur, glow radius/duration,
swipe lead/duration, letter-wave duration, and glow/swipe/wave/wobble/fade toggles.
Existing scroll/timing RICS settings remain supported. Unsupported declarations
are listed in the panel; native Slint does not execute arbitrary CSS/DOM rules.
The browser receives applied CSS and family selection, with reduced motion kept
as the final override. Draft edits do not recompile its theme on playback ticks.

Example:

```css
:root {
  --blyrics-default-font-family: "Roboto";
  --blyrics-font-size: 36px;
  --blyrics-font-weight: 700;
  --blyrics-glow-color: #66ccffff;
  --blyrics-highlight-glow-radius-from: 18px;
  --blyrics-highlight-glow-min-duration: 1.5s;
  --blyrics-swipe-duration-ratio: 1.2;
  --blyrics-animate-word-wobble: 0;
}
```

Glow is clipped to the visible fill edge, rather than tinting a full-word shadow.
The custom native fixture imports a real font, compares glow on/off at a fixed
word timestamp, requires changed pixels under filled ink and exactly zero changed
pixels in the unsung tail, and captures desktop/mobile appearance panels. Its
pixel assertions and visual review passed. All Rust library/integration suites
passed with the previously documented account-label localization test excluded
and environment-dependent tests ignored. The browser custom-theme fixture passed
100 unchanged playback updates without recompiling or rebuilding. The separate
browser stability check still reports the existing grouped-camera assertion
(`matrix(1, 0, 0, 1, 0, 0)` versus `none`); its camera source was not changed by
this appearance work.
WASM compilation and formatting passed. The Windows release executable was
rebuilt, passed its isolated UI smoke and had its settled player frame inspected;
the Windows ZIP was regenerated. Native autoscroll/wheel/resume checks still pass.

Verification commands:

```
cargo test --locked --lib --tests
cargo test --locked --manifest-path third-party/braccato-rust/Cargo.toml --target-dir target -p braccato-slint -p braccato-engine -p braccato-types -p braccato-themes
cargo run --locked --example native_lyrics
cargo run --locked --example native_autoscroll
cargo check --locked --lib --target wasm32-unknown-unknown
```

The native example opens the actual Slint application surface with fixture
lyrics and writes screenshots to `target/native-lyrics-check/`. It checks
real syllable seeks, overlaps, line-only wrapping, mobile/stage views, offsets,
pause/reduced-motion frame stability and manual-scroll resume without reading
user state or requiring audio hardware. Inspect the screenshots separately;
successful assertions alone are not a visual-parity claim.

Windows verification, 2026-10-25: native Vey build, WASM check and Android ARM64
library check passed (installed SDK/NDK/JDK; no APK publication or device run);
203 Vey tests passed with 8 environment-dependent tests ignored and the existing
account-label localization failure excluded. The unfiltered suite reports
`i18n::tests::catalogs_have_all_languages_and_unique_keys`: missing `Акаунт Vey`
in the pre-existing account UI changes. All 18 copied engine/Slint/theme/type
unit tests passed. The native example passed all 14 phases; desktop (1440x900),
mobile (390x844), stage, overlapping vocals, line-only wrapping, instrumental,
plain lyrics and Chinese/timed-romanization snapshots were visually inspected.
Paused reduced-motion frames were byte-identical. The viewport callback and
immediate seek/resize positioning prevent retained-layout clipping of the
active line. Physical Android/Linux/macOS interaction remains unverified.

Autoscroll follow-up, 2026-10-25: the actual-wheel regression, wrapped-word
follow check and all 14 native renderer phases passed. `cargo build --release
--bin vey --locked` completed in 5m41s. `target/release/vey.exe` then completed
the native `--screenshots` fixture from isolated `target/release-ui-check/`
with exit code 0; the release player screenshot was visually inspected.
SHA-256: `a891bf9546fd038179583f258cfbebf35d208fe799ce96e37085a8c217584b41`.

## 0.1.17 light renderer, 2026-10-26

Normal fill now uses one shaped gradient brush per word, rather than base text
plus a second highlight copy. Inactive focus switches between one sharp run and
one cached blur image; it does not blend two differently scaled text copies.
Scrolling keeps lightweight row geometry for the whole document and mounts word
views only in/near the viewport. Stage mode mounts only visible roles. Explicit
row extents and follow restoration after layout changes preserve mobile reflow,
manual scroll, seek and resume.

The default is a restrained 8px glow with no letter wave or word wobble. Custom
CSS can opt into the original motion. Glow-only jobs skip glyph plans, use two
cached blur radii up to 8px, and allocate no masks when glow is disabled; broader
custom glows retain four radii. Filled-only clipping and custom fonts remain.

Native lyric updates no longer serialize a redundant browser document. There is
no native lyric/editor WebView. Rust handles API requests directly; the Windows
and Android challenge WebViews remain for lyric authentication. Windows video
creates a WebView only for a visible selected clip, destroys it on return to
cover mode, suspends hidden updates and coalesces unchanged bounds/payloads.
Startup/readiness retries and a low-frequency paused heartbeat are retained.
Android video uses TextureView, not WebView. The browser build retains its DOM
Braccato integration.

Controlled Windows fixture: 300 source lines / 2,400 words, 1440×900 logical
window, 80 timer samples, 8px glow at alpha 0.5 and waves/wobble disabled in both
runs. A synchronous snapshot is taken every fourth callback. Evidence is in
[native-lyrics-performance-2026-10-26.json](native-lyrics-performance-2026-10-26.json).

| Measurement | Before | Optimized |
| --- | ---: | ---: |
| Median timer interval | 91.95 ms | 18.88 ms |
| p95 timer interval | 678.89 ms | 91.17 ms |
| Median snapshot | 85.89 ms | 74.11 ms |
| p95 snapshot | 120.77 ms | 86.63 ms |

This is a synthetic rendering workload with GPU readback, not an audio playback
FPS claim. These timings precede the backing-vocal follow-up below. Full Rust
suites passed with the known account-label localization
test excluded and environment-dependent tests ignored. Focused light-glow/motion
tests passed. All 14 native renderer phases, filled-only glow/font tests and
actual wheel/resume/wrapped-cue checks passed after reflow stabilization. The real
Windows WebView2 fixture passed eight playback/focus cycles and asserted child
window destruction after every return to cover mode. WASM compilation passed.

The final optimized Windows build completed in 5m47s. The packaged 0.1.17
executable passed the isolated native `--screenshots` fixture with exit code 0;
desktop and mobile lyric frames were visually inspected. The packaged executable
matches `target/release/vey.exe` (SHA-256
`9d3376618f651680d3443b07121d4717b17831a1f498a5ee490dbba63605fee6`).
Local ZIP: `dist/Vey-0.1.17-windows-x64.zip`, SHA-256
`0362138c8cb264b2e2a950ccbae547892c83737e4f62d2389ffff93684431ba7`.
This is a local Windows release; no new tag or cross-platform publication was
performed for 0.1.17.

### Backing-vocal phrase fill follow-up, 2026-10-26

The reported mobile backing phrase `(куди йти…)` was split at spaces into
independent visual words sharing the same timing. Each word restarted its fill.
Wrapping now retains a single shaped run within a row, then assigns measured
cue offset/span to actual wrapped fragments. Both gradient and optional letter
paths project the same cue sweep into those spans; glow follows that edge.
Separately timed words remain separate, and source times/indices are unchanged.

Shared display conversion also retains supplied backing phrase intervals as
one cue, including an explicit start matching the lead. Untimed backing phrases
stay line-synced; the adapter does not manufacture word timestamps.

`examples/native_lyric_backing.rs` compares before/mid/after native pixels for
the short mobile phrase and a wrapped phrase, with normal and optional letter
rendering. Later words/rows must stay at their unfilled color at mid-cue.
Snapshots are saved under `target/native-lyrics-check/backing-*.png`.

Fresh checks passed: six native adapter tests, eight syllable/backing integration
tests, seven copied Slint-crate tests, all 12 backing pixel frames (four cases),
all 14 native renderer phases, formatting and the WASM compile check. At mid-cue,
the four pixel cases found zero prematurely lit later-word/row glyph cores
(125, 761, 752 and 114 sampled cores respectively).
The rebuilt packaged executable also passed the isolated screenshot fixture
with exit code 0; the matching desktop/mobile backing phrase was visually
inspected. A sandboxed GUI launch terminated with Windows invalid-handle status
before initialization; the same executable passed when launched outside that
sandbox. Release snapshots are in `target/release-backing-fill-check/`.

## 0.1.18 scroll timing and quieter glow, 2026-10-26

The standard native glow is now 4px with alpha 38/255 (about 15%), down from
8px at 25%. The soft-glow preset is 8px at 25%; explicitly authored themes
retain their requested settings. Glow still follows only the filled part.

Native planning now ports the group/commit rules from the pinned JavaScript
`packages/core/src/engine.ts`: a separate 500ms `--blyrics-scroll-timing-offset`
advances the scroll clock, while the 540ms `blyrics-early-scroll-consider-s`
window includes nearby lines in a committed move. Entering lookahead alone
does not move the view, and included cues do not move it again at their start.
The 750ms motion duration is independent; a new group retargets immediately.
The active-band solver handles overlapping tails and trims ending lines with
the original 500ms threshold. A tall native source pins its actual wrapped word
row. Layout changes, seeks and manual resume can recommit without a new cue.

Sung line/word clocks remain unchanged. Focus follows the scroll clock so the
next line is readable before singing starts. Paused/reduced-motion positioning
disables the early offset and lookahead. CSS scroll offset and comment settings
for grouping/ending threshold are handled independently.

The original engine's self-check passed across two instances / 133 built nodes
without modifying its source. Rust library/integration suites passed 217 tests,
with eight environment-dependent tests ignored and the known account-label
localization test excluded. `examples/native_lyric_timing.rs` exercises continuous
2x sampling, grouped cues, retargeting an unfinished move and an exact paused seek.
The native timing fixture passed, including eight continuous checks across the
grouped cues. Actual wheel/resume and tall wrapped-word follow passed with zero
canonical source reads during ordinary playback. All 14 native renderer phases
and seven reference-style effect frames passed; the weaker glow and grouped
layout screenshots were visually inspected.
All four backing-fill pixel cases still passed. WASM compilation and formatting
passed. The optimized Windows build completed in 5m48s; the packaged 0.1.18 EXE
passed the isolated `--screenshots` fixture with exit code 0 and matches the
release build byte-for-byte. Release frames are in
`target/release-0.1.18-ui-check/docs/screenshots/`.

EXE SHA-256: `606042dc107fa020824922735e477044a5a1d715d60545bf9fb635e28b8ef937`.
Local ZIP: `dist/Vey-0.1.18-windows-x64.zip`, SHA-256
`3e9e81d560912fe951b7b99307b4c21d95fafd9bc9d8b63c002efd395350507b`.
No new tag or cross-platform publication was performed for this local Windows build.

## 0.1.19 long text and flowing fill, 2026-10-26

The reported long hyphenated chant no longer overflows horizontally. Wrapping
prefers spaces/hyphens, keeps separately timed syllables of a fitting word on
the same row, and splits an oversized token only at complete Unicode graphemes.
Joined emoji and combining marks remain intact. Fragments retain canonical text,
source indices and cue times, plus measured fill offset/span. Native visual
follow now enters each fragment as the original cue's fill reaches it, instead
of jumping straight to its last row at the shared cue start.

The default glow is 6px at alpha 80/255 (about 31%), with the same two-radius
cache and filled-only clipping. A 0.06em sweep wave animates the single shaped
gradient without per-letter surfaces. Scroll uses ease-out instead of the
previous slow-start curve. Reduced motion disables wave/animated scrolling;
untimed lyrics still do not acquire invented word timestamps.

Six copied-layout tests and three native-follow tests passed. The seven-phase
`native_lyric_overflow` fixture passed desktop/mobile/stage chant bounds, timed
fill continuation and following a long unbroken token. Screenshots under
`target/native-lyrics-check/overflow-*.png` were visually inspected. All four
backing-fill pixel cases passed, with no prematurely lit later-word/row cores.
The continuous timing fixture now keeps synchronous GPU readback outside its
clock assertions, since a readback stall can legitimately cause a late cue to
join the next committed group.
Continuous 2x timing, all 14 native phases, seven effect frames and actual
wheel/resume/wrapped-word checks passed. The full Rust suites passed with the
known account-label localization test excluded and environment-dependent tests
ignored. WASM compilation and formatting passed.
The optimized Windows build completed in 5m59s. The packaged 0.1.19 executable
passed the isolated native screenshot fixture with exit code 0; the mobile
release frame was visually inspected and the packaged EXE matches the build.
Release screenshots are in `target/release-0.1.19-ui-check/docs/screenshots/`.

EXE SHA-256: `ff8d128f62ebbc5b28e90f4765ebb506fbcb8df2e0f091a90b9d9d86724fe6ed`.
Local ZIP: `dist/Vey-0.1.19-windows-x64.zip`, SHA-256
`130362c0b314f711de4dd09b9af96fd07768e6208498617a517f11f3665cdee1`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.20 cue boundaries and complete fill, 2026-10-26

Normal native and widget timing now defaults to zero camera advance, lookahead
and ending threshold. Native sung focus and stage selection use actual cue
starts; real overlapping vocals remain active. Explicit anticipatory camera
settings remain available independently of sung focus.

The previous gradient could leave its final portion unfinished until after the
word deadline. Vey enables the copied Slint renderer's optional
`finish-fill-at-cue-end` property to complete each cue at its supplied end.
Wrapped fragments use local progress: the first fragment completes at its
measured boundary before the continuation fills. Later fragments cannot reveal
ink or glow early. Canonical text and timestamps remain unchanged; untimed
lyrics do not acquire estimated word timing. The existing 6px filled-only glow,
single shaped sweep and cached rendering remain in place.

The new nine-phase `native_lyric_boundary` fixture passed both whole-run and
letter rendering: 596 and 2071 changed pixels respectively completed by the cue
deadline, with zero late pixels. Scroll/stage checks kept the first cue focused
before the next actual start. The four-phase `native_lyric_wrapped_boundary`
fixture verified 991 completed first-row cores and 1213 continuation cores with
zero late or prematurely filled pixels. Boundary screenshots were inspected.

All 14 native renderer phases, four backing-fill cases, overflow layouts,
wheel/resume/wrapped follow and continuous 2x timing checks passed. Browser
widget, cue-boundary and custom-theme checks passed. Rust library/integration
suites passed with the known account-label localization test excluded and
environment-dependent tests ignored. WASM compilation, formatting and whitespace
checks passed. The optimized Windows build completed in 5m35s.

The packaged executable passed isolated `--screenshots` with exit code 0 and
matches the release build byte-for-byte. Desktop and mobile release screenshots
under `target/release-0.1.20-ui-check/docs/screenshots/` were inspected.

EXE SHA-256: `2e01647ec4d94f1190d286dd0b3a4c78c433d238beb5adc2a7103a0681e8c3fe`.
Local ZIP: `dist/Vey-0.1.20-windows-x64.zip`, SHA-256
`5b60339997d207a4d8811571c955963d3bf80b21b855e27ec71fdd7c5131cbbc`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.21 smooth line handoff, 2026-10-26

Native scrolling uses `cubic-bezier(0.3, 0, 0.2, 1)` for a soft start and stop,
retaining the configured 750ms duration and immediate retargeting. Actual cue
starts still control the camera in normal mode. No anticipatory timing was
reintroduced.

Focus progression is retained in Rust outside Slint model delegates. The nearby
14-row band sharpens over 180ms and blurs over 280ms with smoothstep interpolation.
Retargeting samples the existing blend before changing direction; seeks and
reduced motion snap immediately. Model updates cannot reset the progression.
Only changed blend values update the native model. No glyph rasterization or
canonical source reads are added to this frame work.

`external-focus-mix` is an optional copied-renderer property consuming the new
row `focus-mix` field. The host aligns sharp ink and cached blur under one row
transform and derives scale from the same blend. Complementary opacity briefly
keeps both bodies during the handoff; settled rows construct one body, and lazy
viewport culling remains intact. The normal single-run fill and cached glow are
unchanged.

Optional `smooth-highlight-fade` uses a smoothstep envelope with flat endpoints
for word highlight/glow and plain line text. The 500ms default fade follows the
supplied line end. Fill progression itself retains the exact cue/fragment
deadline and does not use this fade as a delayed clock.

Fresh focused Rust checks passed 19 native tests and all 10 copied Slint tests.
The 13-phase `native_lyric_handoff` fixture verified intermediate outgoing and
incoming pixels, zero additive bright duplicate pixels, exact restoration,
immediate reduced motion and the fade curve at 125/250/375ms. The real native
camera fixture recorded 54 continuous samples with no early movement, reversal
or overshoot, intermediate camera positions, rapid retargeting and exact settling.
Both deadline rendering paths still reported zero late pixels; the wrapped
fixture retained 991 completed first-row cores and 1213 continuation cores with
zero late/premature fill.

All four backing-fill cases, actual wheel/resume, seven effect frames, imported
font/custom-theme checks, all 14 renderer phases and desktop/mobile/stage
overflow checks passed. Autoscroll still recorded zero canonical source reads
over its two-second playback interval. Fresh desktop/mobile effect and handoff
frames were inspected.

The same synthetic 300-line / 2400-word development workload recorded 80 samples:
timer median 19.74ms / p95 94.50ms, synchronous snapshot median 75.76ms / p95
86.36ms, 2533 word-model reads. Raw output is
`target/native-lyrics-check/performance-smooth-0.1.21.json`. Timer intervals
include synchronous snapshot readback; these values are workload measurements,
not a playback FPS or device-performance guarantee.

WASM compilation and formatting/whitespace checks passed. The optimized Windows
build completed in 6m07s. Its packaged executable passed the isolated native
`--screenshots` fixture with exit code 0 and matches `target/release/vey.exe`
byte-for-byte. Desktop/mobile packaged screenshots under
`target/release-0.1.21-ui-check/docs/screenshots/` were visually inspected.

EXE SHA-256: `fcbc8aa0befb3f3bf4d6bb7d98504acd539b1ad3b393ff00d34bbe73f9b0b16f`.
Local ZIP: `dist/Vey-0.1.21-windows-x64.zip`, SHA-256
`16f696c24f1afe950addf39bd5e7d02ef31706dae4365bad243e099b64894f9d`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.22 whole-word wobble and settled letter motion, 2026-10-26

The previous working state was checkpointed as `1ab1037` before refinement.
Normal native lyrics now enable a restrained word wobble (0.05em translation,
1.025 horizontal peak scale). Layout-only motion intervals/origins group timed
syllables under one transform, preserving canonical text, timestamps and the
independent fill clock. Wrapped fragments start moving at their measured fill
entry. Untimed parts do not acquire motion timing.

The host opts into `settled-word-motion`: smooth attack/return, peak at 22% and
rest by 85% of a period bounded by the actual cue/fragment. Cues shorter than
240ms have proportionally smaller motion. CSS can disable wobble with
`--blyrics-animate-word-wobble:0`, set `--blyrics-wobble-duration`, or tune native
`--vey-word-wobble-strength` from 0 to 2. Whole-word wobble uses the shaped text
run and does not request glyph plans or per-frame rasterization.

Letter waves are independent of word wobble. Their presentation clock fits the
wave plan inside the real fragment interval; fill still uses the original cue
clock. Waves return continuously to the baseline and instantiate glyph bodies
only inside their motion window, using one shaped run before/afterward. Reduced
motion disables both effects. In the backing-fill fixture, moving letter bases
are checked against the measured dim-color ceiling; static cases retain exact
pixel checks. This distinguishes actual early highlight from moving gray ink.

Focused checks passed 20 native tests, five theme tests and all 10 copied Slint
tests. The new seven-phase native wobble fixture recorded 3328 changed motion
pixels, a shared global syllable origin, zero early movement in the following
word, exact resting/reduced-motion pixels and independent letter waves.
Whole-run and letter deadline cases recorded 596 / 4167 changed pixels and zero
late pixels. Wrapped boundary checks retained 991 completed first-row cores and
1213 continuation cores, with no late/premature fill. All four backing cases
passed, including moving letters. Focus/fade pixel checks, 60 continuous camera
samples, 2x grouped/retarget playback, reference-style effect frames and imported
font/custom-theme checks also passed.

The 14-phase renderer, desktop/mobile/stage overflow and actual wheel/resume
fixtures passed; canonical source reads remained zero in the playback check.
Fresh wobble/letter-wave and mobile backing frames were visually inspected.

The same 300-line / 2400-word synthetic development workload was measured with
word wobble off/on, 80 samples each:

| Measurement | Off | On |
| --- | ---: | ---: |
| Timer median / p95, ms | 18.37 / 85.50 | 18.16 / 99.72 |
| Snapshot median / p95, ms | 70.91 / 76.98 | 77.10 / 86.37 |
| Word-model reads | 1624 | 2572 |

Raw reports: `target/native-lyrics-check/performance-wobble-{off,on}-0.1.22.json`.
Timer samples include synchronous GPU snapshot readback and synthetic media
sampling; worker readiness/cue exposure can vary between runs. These are bounded
workload measurements, not playback FPS or a claim that motion has zero cost.

WASM compilation, formatting and whitespace checks passed. The optimized
Windows build completed in 5m20s.

The final backing fixture also passed its dim-base-ceiling assertion. The
packaged EXE passed isolated `--screenshots` with exit code 0 and matches the
release build byte-for-byte. Desktop/mobile packaged frames under
`target/release-0.1.22-ui-check/docs/screenshots/` were inspected.

EXE SHA-256: `5f4b0b05278592eb832a079bb10d2ee81c21e22549e97ab15f6886739c697566`.
Local ZIP: `dist/Vey-0.1.22-windows-x64.zip`, SHA-256
`68a3d83f6019c13df896ebb2eeb1b7a0c14ce8ae933c056614f7a5ef9696d367`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.23 fill consistency and cue-bounded glow, 2026-10-27

The reported repeated-word/backing pattern exposed a real halo lifetime bug:
short cues kept their shadow until the parent line's fade. A new regression
reproduced 2410 residual changed pixels after a 568ms backing cue had ended.
The host now enables optional `cue-bounded-glow`, with smooth attack/release
inside the actual cue/fragment. The same regression now records zero residual
pixels, while active-cue glow still changes 2656 pixels. Source times stay intact.

Whole-run ink, optional letter ink and glow now share one spatial ramp. Native
letter masks project that ramp through measured glyph offsets/widths, including
the one-pixel feather, rather than mixing those widths with grapheme-count mask
windows. Glow uses a matching alpha feather instead of a hard bright edge.
The shadow atlas keeps a constant solid tint. One sung prefix and eight
disjoint source-texel feather bands per sampled radius move the mask without
recoloring the whole atlas on each playback tick. A tint gradient would instead
span the entire atlas, causing different coverage in its separate radius tiles.
An identical-radius isolation fixture now records maximum intensity difference
2/255 and mean 0.21/255 when radius weights change; transparent-margin violations
remain zero.
Completed/empty ink uses an explicit solid brush, avoiding coincident gradient
stops and unnecessary gradient painting at rest. The Vey host uses a straight
fill edge; word/letter motion remains independent. Hyphens remain valid wrap
opportunities but no longer split a word-motion group such as a timed chant.

`native_fill_continuity` sampled 41 static frames from the repeated backing cue:
8057 advancing pixel samples, zero backward pixels and visible advance in 37
of 40 intervals. The 15-phase `native_fill_effects` verifies active/expired glow,
atlas-radius invariance, return from letter motion and variable-width glyph
fill. Its stationary-glyph isolation
recorded mean fill error 0.37 over 1199 eroded glyph cores against the shaped
run, using an intentionally transparent sentinel only inside the fixture.

Focused tests passed 21 native, five theme and all 10 copied Slint tests. Both
deadline paths passed (638 / 4176 changed pixels, zero late pixels); wrapped
boundaries retained 991 completed first-row cores and 1213 unlit continuation
cores without early/late fill. All four backing cases and the word-wobble
fixture passed, including still-unsung following words and exact resting ink.

Focus/fade crossfade pixels, 56 continuous camera samples, imported font/theme
checks, desktop/mobile/stage overflow, actual wheel/resume and all 14 renderer
phases passed. Ordinary playback still recorded zero canonical source reads.
Active/expired glow and mixed-width fill screenshots were visually inspected.
WASM compilation, formatting and whitespace checks passed.

The final optimized Windows build and packaged EXE passed isolated
`--screenshots` with exit code 0. The package matches `target/release/vey.exe`
byte-for-byte. Desktop/mobile frames under
`target/release-0.1.23-final-ui-check/docs/screenshots/` were inspected.

EXE SHA-256: `d262556aef2391008e0ade63efa166dde6822d5aaee694bf11e85445dd7b4e32`.
Local ZIP: `dist/Vey-0.1.23-windows-x64.zip`, SHA-256
`0ef256d2c739d7d4300ef16e99efa47104f1cc8a713a4f082ba4c72cd2670e2f`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.24 renderer-aware spatial fill, 2026-10-27

The native host left `native-text-gradients` enabled for every backend. Slint's
software text renderer uses a single color from a gradient brush, causing all
letters to brighten together. Explicit `winit-software` reproduced this with
381 prematurely bright future-side glyph cores at 25% of a single `H` cue.
GPU-only monotonicity checks did not cover this failure.

Native installation now selects the path from the renderer actually created,
using rendering-notifier support once. The bundled FemtoVG renderer supports
it; software returns `Unsupported`. GPU retains its native text gradient;
software uses the existing fixed-column spatial mask. Both paths use one solid
text run at empty/completed coverage, so future and finished words do not
instantiate the software mask columns.

Reduced motion previously disabled `highlight-swipe`, turning supplied word
cues into instant whole-word highlights. It now keeps timed fill while still
disabling wobble, letter waves, glow and animated focus/scrolling. Explicit
theme swipe disable and genuine untimed/line-sync content retain their existing
behavior; no cue times are inferred or changed.

`native_fill_spatial` checks single and repeated wide Latin/Cyrillic glyphs plus
parenthesized backing-style text. It compares actual eroded glyph interiors at
25%, 50% and 75% with before/full frames, accounting for font bearings/advance.
The matrix includes ordinary/reduced motion on GPU and software.
`--motion` adds a preceding word and 400ms cues with normal word wobble. This
exposed a second GPU defect: a partially scaled word lost its sung-side ink
(374/374 checked cores on the single-H midpoint). Whole-run ink now resolves
into a padded local surface before the parent transform, matching the existing
letter-wave composition strategy. The surface is limited to transformed,
partially filled ink; resting/completed words keep the direct text path.
The fixture waits for the native clock to sample each paused seek before
capturing. On Windows it restores its own minimized window without activation,
since normal native playback deliberately suspends work while minimized.

All 48 partial-fill comparisons passed across the two renderers (80 snapshots
including before/full references), with no incorrectly lit future-side or dim
sung-side cores. The software continuity fixture recorded 7968 advancing pixel
samples, zero backward pixels and visible advance in 37/40 intervals. All four
software backing cases passed, including mobile wrapping and optional letters:
premature-tail counts stayed zero (124/759 checked cores). The 15-phase GPU
effect fixture and imported font/custom theme checks also passed; cue glow
residual remained zero and variable-width fill error remained 0.37/255.
The same 15-phase effect and imported-font fixtures passed on software too:
active glow changed 2028 pixels, expired glow residual was zero, mixed-width
fill error was 0.00/255 and radius-tile mean difference was 0.26/255. WASM
compilation, formatting and diff whitespace checks passed.

The added 400ms/prefix/wobble matrix passed another 48 partial-fill comparisons
on GPU/software with reduced motion on/off. No future-side core lit early.
Static cases retain exact sung-side assertions. Moving cases require a bright
sung-side majority at midpoint, allowing translated glyph edges to leave their
resting reference pixels. The previous GPU midpoint failure improved from
374/374 missing sung cores to 50/374 at the moving single-H stem; repeated
Latin/Cyrillic words had zero missing midpoint cores on GPU.

The final optimized Windows build completed successfully. The packaged EXE
passed isolated GPU/software `--screenshots` runs, each with exit code 0 and
18 frames. GPU desktop/mobile and software desktop frames under
`target/release-0.1.24-final-{gpu,software}-ui-check/docs/screenshots/` were
inspected. The package matches `target/release/vey.exe` byte-for-byte.

EXE SHA-256: `f84c24abcd6361c768a499ab12de9bec51d03a9a5f8a5b827425afe074416a1b`.
Local ZIP: `dist/Vey-0.1.24-windows-x64.zip`, SHA-256
`10a51279dff6b105884dc4efc8a32c330969176f122b61a39d328ff8ab534402`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.25 retained word deadlines and shared glow, 2026-10-27

`FollowIndex` retains sorted cue/start/end and paint-release deadlines when
layout is built. Playback updates only the crossed event rows; seeks revisit
the active, previous and resident bands. Preparation eligibility uses retained
integer windows before reading word models. Continuous fill/wobble and camera
clocks still tick at 16ms; source cues and reduced motion are unchanged. The
strict paint-release boundary is retained and covered by a focused unit test.

The word worker precolors atlas RGB once, including transparent texels, without
changing mask alpha. `glow-precolored` lets all feather/radius Images share it
without per-item GPU colorize textures. Theme alpha is applied as normalized
layer opacity; existing uncolored upstream atlases retain the legacy tint path.
The custom-font fixture compares the two paths with translucent cyan glow to
cover both RGB and alpha. GPU maximum channel difference is 2/255 and software
1/255; unsung-side glow remains zero.

Release benchmark: 300 source lines / 2400 words, word wobble, 80 samples/run,
three runs per renderer and version, same Windows host. `--no-snapshots` excludes
synchronous readback from the event-loop measurement. Reports are
`target/native-lyrics-check/performance-flow-{before,after}-{gpu,software}-N.json`.
Medians across the three runs:

| Renderer | Word reads before / after | Tick p50 before / after | Tick p95 before / after |
| --- | --- | --- | --- |
| GPU | 788 / 40 | 16.257 / 16.354 ms | 17.187 / 17.144 ms |
| Software | 806 / 32 | 16.269 / 16.266 ms | 17.107 / 17.123 ms |

Word-model reads fell 94.9% / 96.0%. Event-loop cadence remains timer-limited
near 16ms; these measurements do not establish a global FPS or GPU-time gain.
The texture change removes repeated tint intermediates by construction; texture
memory savings were not independently instrumented.
The 22 native lyric unit tests and all 10 copied Slint tests passed. GPU and
software each passed static and 400ms/prefix/wobble spatial matrices (96
partial-fill comparisons total, including reduced motion), the 15-phase effect
fixture, 2x/grouped/retarget/paused-seek timing, exact word deadlines and stage
handoff, and imported font/translucent theme checks. The tint comparison covers
the current word's complete glow footprint; neighboring inactive blur masks
may finish asynchronously and are excluded from that comparison.

Software continuity recorded 7968 advancing pixel samples, zero backward
pixels and advance in 37/40 intervals. All four backing-vocal cases passed;
premature tails remained zero across 124/759 checked cores, including wrapping
and optional letter waves. GPU/software expired glow residuals remained zero.

WASM compilation, Rust formatting and whitespace checks passed. The optimized
Windows EXE and packaged copy match byte-for-byte. Isolated GPU/software
`--screenshots` runs each exited 0 with 18 frames. GPU desktop/mobile and
software desktop frames in `target/release-0.1.25-{gpu,software}-ui-check/`
were inspected. Browser and physical-device runtime acceptance were not run.

EXE SHA-256: `990d24fa34e8177bd2a92c1d92403d9762e0e853da8b5d088a89fceca4da39d1`.
Local ZIP: `dist/Vey-0.1.25-windows-x64.zip`, SHA-256
`6700c8d4ae7e7179fa24ca19b58e8406a5c5870a44121b2c40a589159fdd4ecc`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.26 player-entry and lyric handoff stalls, 2026-10-27

The 0.1.25 read-count benchmark covered warm playback, not cold player entry
or dense wrapped-line handoffs. `native_lyric_stutter` now opens the player,
plays 300 source lines / 2400 explicit word cues (one source line per second),
leaves and reopens it. It records 16ms timer intervals without snapshot
readback, while normal production navigation captures remain enabled. The
baseline release recorded an 802.68ms maximum interval on first entry and
182.03ms on reopening. These are local event-loop measurements, not FPS.

The host retains exact full-document geometry but instantiates only an
overscanned viewport band. `FilterModel` adds/removes boundary rows using the
presentation-only `rendered` flag, keeping overlapping delegates and their word
models alive. Omitted heights become top/bottom spacers, preserving actual
wheel scrolling, source-index seeks, wrapping and the final scroll extent.
Nearby sharp bodies stay mounted through focus crossfades. Snaps select their
destination band before the first navigation capture, avoiding an empty entry
frame. The stutter fixture asserts that the snapped cue is already painted.

Focus updates use a separate float model, avoiding full word-bearing row
invalidations on every blend frame. The legacy row `focus-mix` API remains
available to standalone callers. Font family/width measurements have bounded
thread-local caches keyed by text, family, size and weight, with
font-revision invalidation. Measurement math is unchanged and compared against
the uncached implementation for Latin, Cyrillic and Japanese text.

The live player no longer rerenders a permanent full-page cache on every lyric
tick. Native navigation performs its readbacks once, then a single worker
crops/downsamples/blurs the raw pixels while the camera continues. Busy workers
prevent queued retarget captures; obsolete/late/minimized results are discarded.
Ready blur fades in over 80ms. A late result uses live content instead of holding
the camera. WASM retains the original synchronous snapshot fallback and never
spawns the native worker.

Final Windows GPU release comparison (same dense fixture, 1440x900): the
baseline is one recorded run; final values are medians across three fresh
process runs. Raw reports are `stutter-release-before.json` and
`stutter-final-release-{1,2,3}.json` under `target/native-lyrics-check/`.

| Phase | Baseline p95 / max | Final median p95 / max |
| --- | --- | --- |
| First player entry + playback | 60.94 / 802.68 ms | 31.22 / 70.71 ms |
| Return Home | 47.95 / 150.62 ms | 21.56 / 101.08 ms |
| Reopen player + playback | 59.88 / 182.03 ms | 32.30 / 98.95 ms |

First-entry maxima across the final runs were 63.69 / 70.71 / 80.70ms;
reopening maxima were 117.37 / 74.23 / 98.95ms. Only 21 row delegates were
needed for the 900 wrapped rows. Every run verified the bounded transparent
768x485 blur texture. This removes the large layout pause and substantially
reduces handoff work; it is not a zero-spike or constant-FPS guarantee.
Synchronous GPU readback remains for the optional camera blur. A separate
`final-no-blur` run measured first/reopen maxima 56.27 / 63.92ms, with p95
31.90 / 32.99ms. Timed fill, word wobble and lyric glow stay enabled there.

All 25 native lyric unit tests and 10 copied Slint tests passed. GPU/software
fixtures passed cue deadlines, wrapped continuations, focus/unhighlight pixels,
long-text/mobile/stage bounds, real wheel/resume and spatial fill. GPU short-cue
motion coverage also passed. The snap fixture verifies the destination band on
entry. Final imported-font/translucent-theme checks passed on both renderers,
with legacy/precolored glow differences at most 2/255 GPU and 1/255 software.
Their unchanged-glyph comparison allows the observed one-level GPU composition
rounding; unsung glow above that tolerance remained zero. The debug navigation
fixture verified live fallback, retargeting, the blur switch, reduced motion and
clipping; its worker was too late to present blur, which the release fixture
verified separately. WASM compilation, formatting and whitespace checks passed.

The final optimized Windows package passed isolated GPU/software
`--screenshots` runs, each exiting 0 with 18 frames. GPU desktop/mobile and
software desktop frames under `target/release-0.1.26-{gpu,software}-ui-check/`
were inspected. The packaged EXE matches `target/release/vey.exe` byte-for-byte.
Browser runtime, physical mobile devices and real audio playback were not part
of these fixture checks.

EXE SHA-256: `50f0f986c57aec8f3a1f7a2c8c323057037ef491d1e00b0dc5af72285d58d16b`.
Local ZIP: `dist/Vey-0.1.26-windows-x64.zip`, SHA-256
`2c62b378648371a5b5fa9ea37b9b4e9f570ec732cc4fa69d02f7182e13f3e164`.
This is a local Windows build; no tag or cross-platform publication was performed.

## 0.1.27: prompt line launch and retained velocity

The old native `content-y` animation restarted its easing at rest whenever a
short or wrapped cue changed the scroll target. This introduced a visible
braking pause even when the event loop continued producing frames. The
`native_line_flow` fixture samples actual content position around a 400ms cue,
then retargets an unfinished 750ms move. Its `--legacy` flag exercises the old
path in the same executable; reports are `line-flow-LABEL.json`.

The host now drives scroll position with `src/native_lyric_camera.rs`. Each
retarget samples the existing segment's position and velocity, then starts a
monotone cubic Hermite segment from those values. Zero terminal velocity and a
bounded initial slope prevent overshoot. A very close forward target shortens
the remaining duration to preserve incoming speed. Reverse targets discard
opposite velocity rather than moving away from the target.

A camera at rest uses a front-loaded start slope instead of a zero-slope
ease-in. Previously the first third of a settled line transition travelled
almost no distance, which read as a freeze even though frames kept arriving.
The launch now covers 20px in 65.6ms in the release fixture (66.6ms on the
software renderer), versus 121.4ms for the legacy path; a moving retarget still
keeps its exact incoming velocity. The fixture also asserts that the displayed
`content-y` tracks the settled camera and that the target settles exactly.

Slint's own scroll easing is disabled only while this native driver owns the
position. The render window follows the displayed camera, including exact seek
snaps. Manual scroll, passive and stage paths keep their existing ownership;
returning to follow rebases at the displayed position. Source cue times, fill,
focus, glow, fonts, themes and word wobble are unchanged. The camera evaluates one
retained segment per tick; it does not read word
models or allocate per frame. Hidden/minimized lyric work remains suspended.

Five unit regressions cover position/velocity continuity, rapid repeated cues,
close targets, front-loaded launch from rest, reverse motion, exact snaps and
manual rebasing. Existing native fixtures cover actual wheel/resume, cue
deadlines, spatial fill and reduced motion. Release line-flow, timing, boundary,
autoscroll, focus handoff and the 300-line stutter fixture passed after the
change. This fixes the easing restart and slow launch; GPU
rendering/readback costs documented for 0.1.26 remain a separate source of
possible long frames.

The optimized Windows executable was packaged locally. The packaged EXE
matches `target/release/vey.exe` byte-for-byte.

EXE SHA-256: `79c236b6cfbba8f933994a34c9ddc6c1d1a7813e88b196109f2076ab56c25c35`.
Local ZIP: `dist/Vey-0.1.27-windows-x64.zip`, SHA-256
`6d0685a68afb28d0ac6171169c7de8b0f4df205565465e436fe1c1db75edc706`.
This is a local Windows build; no tag or cross-platform publication was performed.
Browser runtime and physical-device acceptance were not repeated for 0.1.27.

### 2026-10-28: incoming scroll after a delayed frame

Compared the preserved original Braccato `packages/core/src/engine.ts`
(`startPreparedLineScroll`, explicit per-line animation, 750 ms fallback) with
native scroll ownership. Native `snap_until` classified every media-clock delta
above 500 ms as a seek, including ordinary playback while a UI frame was late.
That bypassed the camera animation at the next cue. `playback_jump` now compares
media travel with elapsed wall time and playback rate. Backward/forward seeks,
paused seeks, rebuilds and reduced motion retain immediate positioning.

Regression: `cargo run --locked --example native_lyric_gap -- label` uses an
8-second explicit gap and a 650 ms delayed frame spanning the incoming cue.
Before the change it failed: zero intermediate camera samples, first incoming
position already equal to target and snap=true. Outputs use the supplied label
under `target/native-lyrics-check/long-gap-*.json`. This reproduction establishes
a delayed-frame snapping path, not the cause of every reported long-gap event.

Fresh Windows checks: the delayed-frame fixture now passes with 19 intermediate
camera samples on GPU and 12 on software, versus zero before. The playback-jump
regression and all five retained-camera unit tests pass. Source cue times and
highlight fading are unchanged; no original-song payload was supplied.

## 0.1.28 — prepared and stable line transitions (2026-10-28)

The previous delayed-frame correction did not resolve the user's observed
transition. This pass also addresses actual wrapped focus, rest motion and
per-frame rendering work:

- `FollowIndex` excludes synthetic line-synced tokens from word-follow cues.
  Every fragment starts together, so selecting the final equal-start cue was
  incorrectly focusing the last wrapped row. The first row now stays selected;
  real word/syllable follow remains timestamp-driven.
- `Timeline::scroll_groups` and `FollowIndex::prepare_targets` prepare camera
  destinations for cue, overlap and policy changes after layout. The table is
  keyed by group, source and visual row and invalidated by geometry/policy
  changes. Work/storage have explicit budgets and an exact live fallback for
  exceptional groups, seeks and resumes.
- Settled cameras use zero initial velocity and eased acceleration/deceleration,
  replacing the maximum initial slope of 3. Moving retargets still retain
  velocity; close targets remain monotone and settle exactly.
- A bounded pair of upcoming row bodies is selected up to one second before
  its cue. Current row bodies remain available while the camera enters them.
  Keeping every overscan row fully prepared was measured and rejected because
  it increased rendering cost.
- Trailing spaces stay in their word's shaping run. Text, whitespace, line cue
  times and whole-word motion are preserved without separate empty-space
  delegates. This reduces work for synchronized line-only lyrics.

Fresh checks on Windows:

| Check | Result |
| --- | --- |
| Native lyric unit tests | 34 passed |
| Clock/scroll-policy unit tests | 9 passed |
| WASM compile | `cargo check --locked --lib --target wasm32-unknown-unknown` passed |
| Release line flow, GPU | Retarget speed 356 -> 422 -> 473 px/s; 20px launch 167.7ms; exact settlement |
| Release line flow, software | Retarget speed 368 -> 432 -> 475 px/s; 20px launch 177.8ms; exact settlement |
| Long wrapped source, eight-second gap and delayed UI frame | GPU desktop 21 and mobile 20 intermediate camera samples; software mobile 22; first-row and prewarming assertions passed |
| Actual wheel / resume / wrapped real word cues / optional early camera | Passed on GPU and software with a wheel event over visible lyric text |
| Debug stress, line-only, before -> final | First-entry playback p95 91.04 -> 78.42ms; repeated-entry playback p95 91.82 -> 83.97ms |
| Release stress, line-only, GPU | Playback phase p95 19.19ms and 20.56ms; maxima 56.27ms and 83.73ms; render CPU p95 9.97ms |
| Release stress, line-only, software | Playback phase p95 21.30ms and 18.87ms; maxima 38.63ms and 25.68ms |

Stress command: `native_lyric_stutter -- label --line-only --no-blur` uses 300
lines, cold/repeated player entry and continuous cue handoffs without snapshot
readback. These are fixture event-loop intervals, not FPS guarantees; debug
before/after and optimized results are separate. The optimized GPU run still
had two intervals above 50ms. Reports are under `target/native-lyrics-check/`
with `stability-*` labels. The full-buffer rendering experiment is retained
only as diagnostic output, not implementation.

`native_lyric_gap -- label --wrapped --mobile --capture` captures the actual
native window before/after a deliberately tall source. Images were inspected;
the incoming source starts at its first visual row. Existing screenshots in
`docs/screenshots/` were not regenerated. The original wheel fixture's fixed
500px Y coordinate failed on GPU in a padding band; its visible-text coordinate
now passes. This is not an exhaustive acceptance test of all wheel hit regions.

The default local profile had reduced motion disabled and no custom lyric CSS;
no user settings were changed. The original-song payload was not supplied, so
acceptance of that exact incident remains unverified. Android/Linux runtime and
browser runtime were not exercised. Package version is now 0.1.28 to distinguish
this build from earlier 0.1.27 artifacts.

## 0.1.29 — audio polling cancelled the reported song transition

The user identified Smooth Criminal, line 2 to line 3. The default local profile
contained a saved Binimum RichSync selection. Only that song's first six cached
lines were copied to an ignored diagnostic directory; no user settings/library
or authentication data were modified. No copyrighted song text was added to the
repository. The checked-in polling fixture uses neutral text with equivalent
cue boundaries.

Actual source cues: second line [32728,34627), third line [35324,36684) ms.
Replaying them with ideal fresh media positions did not fail. Replaying the
native transport cadence did: `audio_thread` publishes about every 100ms, while
the UI polls on its frame timer at >=100ms (typically 112ms). A cached audio
sample can therefore be almost 100ms old when the UI receives it. The previous
clock treated negative errors >=60ms as discontinuities, rewound presentation
time, and `playback_jump` treated every backwards clock step as a seek.

The reproduced 2→3 transition began at camera=385.30px with target=575.91px.
A late status then changed the lyric clock from 35364ms to 35329ms, enabled
`snap_until`, and jumped the camera by 190.61px in one sample. There was no
layout rebuild or camera retarget discontinuity. The earlier easing/focus
changes did not address this audio handoff.

Changes:

- `src/native.rs`: private shared status records when its position was sampled;
  reads project that age only during playback, honoring rate/duration and a
  250ms stale-sample limit. Confirmed successful loads/seeks update a serial
  atomically with their position. The serial is omitted from serialized status.
- `src/app.rs` / `ui/main.slint`: confirmed serial changes advance the native
  seek revision together with the new media position.
- `src/braccato/engine.rs`: late but advancing actual samples cannot rewind the
  rendered clock. Corrections adjust future speed, bounded to a 10% slowdown.
  Real backwards samples and explicit clock resets remain exact.
- `src/native_lyrics.rs`: camera/focus reset follows confirmed transport seeks,
  page entry, paused seeks and explicit presentation changes. Ordinary playing
  clock corrections, including late catch-up, do not cancel camera motion.

Fresh optimized Windows results (7.5-second replay, no snapshot readback):

| Case | Clock reversals | Snap samples | Largest camera step |
| --- | --- | --- | --- |
| 0.1.28, sampled worker/UI cadence | 8 | 95 unintended | 190.61px |
| 0.1.29, same cached cues/cadence, GPU | 0 | 0 | 17.73px |
| 0.1.29, same cached cues/cadence, software | 0 | 0 | 13.41px |
| 0.1.29, real native worker + muted silent WAV, GPU | 0 | 0 | 18.43px |
| Real native worker, deliberately confirmed small seek | 1 expected seek rewind | 12 expected seek-window samples | 107.65px expected seek reposition |

The explicit-seek fixture verifies the backend seek serial reaches the UI and
that the camera snaps exactly to its target. This is a controlled silent-file
native-audio test; it does not establish original streaming playback or output
switching acceptance. The neutral fixture also passes the sampled-audio
assertions. WASM compilation passed.

Targeted clock/transport regressions passed. The complete library suite reported
187 passed, 7 ignored and one failure in the unchanged locale catalog test
(`i18n::tests::catalogs_have_all_languages_and_unique_keys`, a one-field row
instead of eight). That existing unrelated catalog change was preserved.

Reproduction:

```
cargo run --release --locked --example native_lyric_song -- tests/fixtures/native_audio_polling.json target/native-lyrics-check/polling-new --sampled-audio --assert-smooth
```

Use another output directory for each run. `--confirm-seek` checks a deliberate
25ms seek during the camera move; `--narrow` changes window width. A canonical
`LyricsAlternative` JSON can replace the neutral fixture to test a supplied
provider document. Optional `--audio=ABSOLUTE_WAV_PATH` uses the native audio
worker (muted inside this test) instead of synthetic media samples. Private
song data and local traces remain under ignored `target/` only.

## 0.1.30 — native letter and fill quality

The quality fixture isolates short (400ms) and long (4s) word cues with camera,
focus scaling, glow and whole-word wobble disabled. It compares AVATAR, Toffee,
office and a combining-accent word with letter waves on/off at cue start, return
and cue end. Extra samples near both endpoints check ink mass, catching both
dim pulses and duplicate heavy edges. Orange active ink over translucent blue
inactive ink checks alpha and feather interpolation. Outputs require a new
directory, and the window fits the physical display at fractional scale.

Reproduced causes and changes:

- Negative planned letter delays made future letters start already lifted in
  short cues. Settled motion now maps the full neutral-to-neutral plan into the
  existing cue/fragment interval, including its negative start; fill clocks are
  independent and unchanged.
- Nominal per-character advances omitted pair kerning and split ligatures or
  combining marks. The host now caches HarfRust-shaped runs at the selected
  weight, retaining cluster advances in em units and merging their existing
  animation windows. Contextual/joining/reordered runs retain whole-word text
  when independent cluster ink would differ. Font revisions invalidate caches
  and prevent stale computations from being published into a newer generation.
- Femtovg rounds each Text node's canvas origin, while positions within a run
  retain fractions (`i-slint-renderer-femtovg` 1.18.1, `align_canvas_during`).
  Letters now share a Text origin and place the cluster inside right-aligned
  text using its actual isolated shaped advance. A 0.001 physical-pixel bias
  keeps exact half-pixel ties from rounding differently after float translation
  cancellation. Glyph paint width is separate from the kerned layout advance.
  Solid letters retain direct Text rendering; only transformed gradients use a
  local bitmap surface, avoiding a cached-raster handoff at neutral endpoints.
- Software fill formerly divided the whole word into at most 256 logical-pixel
  strips. It now paints the solid prefix/suffix once and samples only the
  feather at physical-pixel resolution (512-band cap for broad custom feathers).
  Text origins remain fixed under the clipping bands.
- Slint Color.mix uses Sass alpha weighting; Femtovg and Skia also interpolate
  gradient alpha differently. Software uses premultiplied samples, and native
  gradients receive eight matching feather intervals. The host normalizes byte
  alpha before applying its inactive-text cap, preserving faint custom colors.

Reproduction (use different output directories per invocation):

```
cargo run --locked --example native_lyric_quality -- target/quality-gpu-new
cargo run --locked --example native_lyric_origin -- target/origin-new
```

Repeat the quality fixture with `SLINT_BACKEND=winit-software` and with
`SLINT_SCALE_FACTOR=1.25`. `native_fill_spatial -- --motion` checks short-cue fill
during whole-word motion. Its prefix now has a finite already-completed cue;
the earlier untimed prefix was still moving in the off reference and could
pollute the timed word's core mask, producing a false early-fill failure.

These checks cover bundled fonts on Windows GPU/software, including fractional
display scale. They do not establish Android/Linux runtime behavior or every
imported font's optional size-dependent shaping. Source timestamps, real-timing
classification, the audio clock and camera policy are preserved.

Fresh Windows verification:

- All 32 word/duration/renderer/scale combinations pass the quality assertions
  at 1x and 1.25x. Start/return differences fell from hundreds or thousands of
  pixels to at most two pixels above a 3-level RGB tolerance. Software matches
  exactly; cue-end comparisons are zero on both renderers. The 192 near-endpoint
  samples retain 0.9945–1.0076 of plain ink mass, with no duplicate heavy edge.
- The isolated common-origin proof includes a physical half-pixel tie at 1.25x.
  Both direct shared placement and cancelling a positioned parent's offset pass.
- Translucent color comparisons over 1302 common glyph-core pixels at 1x and
  2298 at 1.25x differ by at most 4/255 per RGB channel, compared with 28–42
  before the shared feather samples. Mean error is 0.24–0.56/255. Requested
  inactive alpha 64 and 128 produce distinct off-state colors as expected.
- GPU/software spatial fill passes all 96 partial-fill comparisons, including
  400ms cues, a finished prefix, whole-word wobble and reduced motion. Effect
  checks report zero expired glow residual and zero letter-return fill deficit.
  Wide/narrow fill error is 0.16/255 on GPU and 0.00 on software. Continuity
  checks have no backward fill pixels; independent letter waves, exact word
  rest and reduced motion pass on both renderers.
- The 38 targeted native tests pass, including shaped cluster/worker geometry
  and stale-font revision rejection. The library test executable reports 192
  passed, 7 ignored and the pre-existing locale catalog failure (one field where
  eight are required). That unrelated catalog edit is preserved. WASM checking
  and targeted formatting/diff checks pass.
- `cargo build --release --locked --bin vey` succeeds. The optimized library
  also passes the quality fixture on GPU at 1.25x, including the half-pixel
  endpoint case. The 0.1.30 portable package is built from that executable.

Relevant source entry points: `src/native_lyric_fonts.rs:417` (safe shaped
clusters), `src/native_lyric_glyphs.rs:77` (timing/model projection),
`src/native_lyric_paint.rs:95` (worker transfer), `ui/native-lyrics.slint:72`
(inactive alpha), and `third-party/braccato-rust/crates/slint/ui/lyrics.slint:149`
(ink), `:408` (neutral cue-bounded wave) and `:640` (gradient-only cache).
