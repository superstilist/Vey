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
