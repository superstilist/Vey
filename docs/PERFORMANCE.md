# Player interaction performance

## Build reuse and Windows executable size (2026-11-04)

The generated Slint UI is compiled as `crates/vey-ui` and re-exported by
`playervf`. Editing player Rust logic can reuse the UI crate; changing an
imported `.slint` file or font rebuilds it. Root `build.rs` handles the Android
bridge. The UI build script re-exports the five embedded font resources for
native lyric shaping, avoiding extra copies. It validates the Slint-generated
resource declarations and fails explicitly if their format changes; review
this integration when upgrading Slint or changing font imports.
For the optional `SLINT_EMBED_RESOURCES=false` development mode, Slint keeps
its path-based resources and the lyric shaper gets its own bundled font bytes.
That mode remains dependent on the source paths and is not a standalone release.

Windows x64 release, Rust 1.95.0, Thin LTO, same player features and fonts:
the executable decreased from 105,901,568 to 82,005,504 bytes (101.00 to
78.21 MiB, 22.56%). Each font appeared three times before and once afterwards.
The first build with the new crate took 603.77 s versus 631.41 s for the
previous build; those runs also differed in which dependencies were rebuilt,
so they are not a controlled comparison. With a warm cache, a controller-only
`cargo check --locked --lib` took 1.58 s and reused the UI crate. These results
are machine-specific and do not measure a cold build on another machine.
After the build-script compatibility fix, a fresh release rebuild took 650.05 s.
A subsequent release build after changing only the controller file's timestamp
took 306.75 s and did not rebuild `vey-ui`; an unchanged release build took
0.78 s. No equivalent controller-only release measurement was taken before the
split, so these timings do not establish a before/after speedup ratio.
The final Windows executable completed the isolated 18-screen smoke run; the
five exported fonts matched their source files in a standalone test, and six
native lyric font tests passed. Cargo's integration-test command could not
replace the already-running `target/debug/vey.exe`; its compiled font test was
run directly instead. WASM and Android ARM64 passed `cargo check`; neither a
browser runtime nor an Android device was validated in this build pass. The
`SLINT_EMBED_RESOURCES=false` development mode also passed `cargo check`.

## Navigation controller work (2026-11-04)

Native and Web refresh library collection cards only while their Library
category is visible; returning to that category rebuilds the cards from the
current store. Other routes no longer rebuild those collections. Collection
history is evaluated only for the Recently Played sort, and song sorting
calculates each track's history key once rather than once per comparison.
Native playlist refresh resolves the library once instead of twice; Web no
longer refreshes playlist rows twice in one refresh. Empty selections skip the
library identity scan. These changes reduce synchronous controller work; they
are not a measurement of end-to-end animation or Android device performance.

Native collection projection now runs on Tokio's blocking worker pool. At most
one collection build runs at a time; later refreshes coalesce into one pending
build, and generation checks discard results from an older view or store state.
Only a subset of Store data is copied for the worker. Slint model changes and
texture uploads stay on the UI thread. Artwork completion updates matching card
images without rebuilding all collections. The Web controller still builds its
collections on the browser thread; this change does not add a Web Worker.
The native loading presentation can be captured with `cargo run --locked
--example library_loading -- target/NEW_OUTPUT.png`; use a new path and add
`--mobile` for the narrow layout.

On 2026-11-04 the collection job coalescing/stale-result and background
projection tests passed. Windows native, WASM and Android ARM64 compilation
checks succeeded; isolated 1100px and 390px native loading screenshots were
reviewed. No Android device was connected, and these checks do not measure
real-library navigation latency or establish multithreaded Slint painting.

Fresh Windows debug checks on 2026-11-04: `navigation_latency` completed nine
routes with 300 synthetic rows and the default backend. First movement on the
cold first route was 1284.76 ms; later routes ranged from 54.35 to 127.48 ms,
and the sampled event-loop p95 was 127.48 ms. This fixture sets Slint routes
directly and does not exercise the application refresh callback, so it cannot
measure the controller changes above or establish an improvement. A separate
`navigation_transition` run failed its static corner pixel equality before the
first transition: the sampled RGB values differed by one in two channels.
The default backend was selected; the actual renderer was not identified.
Direction/retarget and corner conformance were therefore not verified by that
run. Android runtime was not checked.

2026-10-29, source version 0.1.33. This pass covers catalog/list/controller
work on Windows x64 and the shared WASM controller. It does not establish
all-song, audio-device, Android or Linux runtime performance.

## Player loading and page work (2026-11-01)

Native artwork loads use a bounded queue: four background requests and one
reserved slot for the current track. A refresh retains queued covers that still
belong to the visible page and discards stale offscreen work, so routine list
updates do not restart the backlog. Transient failures retry once automatically;
cover requests time out after ten seconds. In-flight results update retained
models. Local artwork is decoded once, with corrupt candidates skipped.
For local tracks that still have no embedded/folder art or video ID, the same
background queue makes a song-only YouTube Music lookup (8-second cap), accepts
only normalized exact title/artist matches, and prefers the closest known
duration. A successful result is cached under the local track identity and
updates mounted playlist/library rows immediately; an unmatched song keeps its
placeholder instead of receiving a likely-wrong cover.
Browser artwork uses its existing limits, prioritizes the selected song, bounds
response bytes and aborts stalled fetches.
Browser covers are fetched into a bounded seven-day CacheStorage cache. The
browser decodes images asynchronously with `createImageBitmap`, scales them to
768px and returns bounded RGBA pixels to Slint; startup no longer runs the full
WASM image decoder synchronously for each Home cover. Offscreen page warming
starts after a three-second startup grace period and still yields during travel
and palette changes.

Complete, size-checked YouTube audio cache entries have a local metadata index,
so later opens avoid resolving a stream over the network. Missing or invalid
indexes fall back to the existing resolver. Range downloads refill a free slot
immediately and assemble validated chunks in one bounded buffer. First playback
still waits for a complete download; device and live-network latency were not
measured in this pass.

A candidate that hid nonvisible pages reduced cold first movement but worsened
the event-loop p95 and median settling in three local debug navigation runs, so
the existing page visibility policy was retained. At 1100x760 with 300 synthetic
tracks, medians across the runs were 1695→985 ms to the first movement,
489→502 ms to settle, and 338→449 ms event-loop p95. These are debug fixture
samples, not release FPS or a device-wide claim. The Windows
`navigation_transition` fixture passed desktop/mobile, retarget and reduced-motion
checks on the candidate. Isolated audio tests passed 22 cases, including cache
recovery and concurrent local HTTP ranges; artwork fallback passed five cases,
and the native queue passed two. Browser fetch size/timeout and queue priority
fixtures are `npm run test:artwork-fetch` and `npm run test:artwork-queue` under
`tools/braccato-widget`. These checks do not establish real-device FPS.

### Native ambient during navigation

The native ambient timer still advances its motion and transition clocks on
each tick, but emits a frame at most every 100 ms while pages or the mobile
sheet move. It resumes its normal cadence after travel; first frames, reduced
motion and paused settling are retained. `VEY_LEGACY_AMBIENT_TRAVEL=1` restores
the prior cadence for comparison.

The isolated Windows debug `ambient_navigation` fixture observed 23–25 ambient
updates during a 1.6-second series of 13 route changes with the legacy cadence,
and 12 updates with the new cadence. Both produced 24 updates in the 0.8-second
interval after navigation; paused and reduced-motion intervals produced none.
Two later runs sampled navigation event-loop p95 at 77–78 ms with the legacy
cadence and 73–76 ms with the new cadence; the small samples do not establish
an end-to-end speedup. These are debug observations, not global FPS or release timing. Reproduce
with `cargo run --locked --example ambient_navigation`; run the same executable
with the diagnostic environment variable for the legacy comparison.

## Faster page and panel motion

The follow-up motion pass uses 100–180ms travel (160ms per neighboring cell),
an ease-out curve shared by Slint/Rust/GSAP, and immediate route highlighting.
It removes the preparation-frame wait and the extra 20ms completion padding.
Retargets restart their completion deadline. Settings content commits on input;
there is no fade-out wait or input shield. Feedback is 70ms, secondary panels
100ms and the retained mobile player sheet 140ms. Lyric cue/fill/follow timing
and authored artwork palette durations are unchanged.

The 14 animated vector streak paths were removed from page travel. Follow-up
Windows GPU release profile, 2026-11-03, same 300-track / 1100×760 / 9-route
workload: p95 event-loop interval was 21.75ms before and 21.33ms after;
maximum was 69.18ms and 69.44ms. Route settling ranged 162–195ms. This fixture
does not show a measurable event-loop improvement from removing the paths, so
the change is a lower-complexity visual path rather than proof that the reported
freeze is resolved. Native artwork concurrency is also reduced to two
background decodes plus the reserved current track; the route fixture does not
load covers and cannot quantify that separate improvement.

Normal navigation uses live cached page surfaces instead of screenshot page
blur. This removes two full-window readbacks per journey, native blur-worker
startup/polling, CPU blurring and navigation texture uploads. Browser lyric-host
compositor blur remains. `VEY_LEGACY_NAVIGATION_CAPTURE=1` enables the old native
snapshot effect for diagnostics; it does not restore the former motion timing.

Windows GPU release fixture, one sweep of nine cold/repeated routes, 300 tracks,
1100x760 window, 8ms sampling timer, with no screenshots during timed movement:

| Measurement | Before, ms | After, ms |
| --- | ---: | ---: |
| Median time to first 5px of movement | 91.53 | 41.76 |
| Median request-to-settled time | 490.57 | 185.13 |
| p95 event-loop interval during travel | 51.78 | 41.75 |
| Maximum event-loop interval during travel | 132.98 | 50.14 |

These are local route/input scheduling samples, not all-page/device guarantees
or an FPS measurement. Run `cargo run --release --locked --example
navigation_latency -- target/NEW_OUTPUT_DIRECTORY` to reproduce the workload.
The final after-run used the freshly rebuilt release executable; final reports
live under `target/navigation-final/` and `target/navigation-before/` locally.

The GPU `navigation_transition` fixture passed direction changes, reduced
motion, desktop/mobile corners and immediate consecutive Settings changes.
The opt-in legacy capture fixture also completed using its live fallback when
blur was late. Native and WASM builds completed after the motion changes.
The optional software-renderer run failed the static corner-pixel assertion
before its first route change; software pixel conformance is not established.
Browser camera easing, viewport/clipping, navigation-pad gestures and the new
`test:navigation-latency` fixture passed. The browser fixture observed zero
full-window navigation readbacks. Its debug-WASM host observations include bridge
polling, Playwright input overhead and rendering; they are separate from native
timings above. The 2,000-track transport/likes/save fixture also passed after
the motion change. Rust: 254 passed, eight ignored, the known locale failure
excluded. See `docs/PAGE-MOTION.md` for motion contracts and fixture commands.

## Changes

- `Store::remember_all` indexes one batch instead of repeatedly scanning the
  growing catalog. Scans, public results, browser file imports and migrated
  history use it. Order and last-update-wins metadata remain compatible.
- `Store::playlist_tracks` indexes the catalog once per playlist resolution,
  preserving playlist order, repeats and missing-reference handling.
- `src/track_model.rs` retains page models and unchanged delegates. Cover and
  like updates use path indices, visiting matching rows once per distinct
  mounted model. Small insertions/removals retain unaffected delegates; large
  structural changes reset once rather than repeatedly shifting the vector.
- Native list construction builds the downloaded-file reverse lookup once.
  Likes outside the Liked page update matching rows and the current-track flag
  without rebuilding the song list. Liked still refreshes its membership.
- Browser file imports merge selected identities in one pass. Browser saves
  coalesce into a 200ms window, keeping the latest store; `pagehide` and hidden
  `visibilitychange` flush a pending save synchronously. JSON serialization and
  localStorage writes still run on the browser thread, once per flush.
- Browser rapid track switches reject obsolete/aborted play promises without
  showing a stale error over the newer selection.

Existing background native saves and shutdown flushing, bounded artwork decoding,
result-drain budgets, request generations, lyric timing and motion are retained.

## Repeatable controller benchmark

Run `cargo run --release --locked --example player_performance`.
The executable compares the prior algorithms with the new implementations in
the same process, using synthetic data only. Values below are the median of
five samples on this Windows host. Setup/drop costs are included in both catalog
merge paths; the artwork dispatch models are already constructed in both paths.

| Workload | Before, ms | After, ms |
| --- | ---: | ---: |
| Merge 10,000 tracks into an empty catalog | 175.3123 | 4.0007 |
| Resolve 2,000 playlist entries in a 10,000-track catalog | 36.0026 | 0.8439 |
| Dispatch 400 cover updates to a 10,000-row model | 191.2921 | 0.0705 |
| Apply 100 like changes to a 10,000-row model | 58.2788 | 0.0166 |

The last two rows measure dispatch/model work, excluding cover decoding, GPU
uploads, Slint layout/painting and persistence. They are not frame-time/FPS
measurements or end-to-end click latency. Small catalogs can have different
tradeoffs because building an index has a fixed cost.

## Regression checks

Focused Rust checks cover bulk-merge compatibility and unchanged serialized
state, ordered/repeated/missing playlist entries, and row-index updates after
removal/reorder/duplicate identities. The broader library run passed 205 tests,
with seven ignored and the existing blank-row locale-catalog test failing
(`i18n::tests::catalogs_have_all_languages_and_unique_keys`).
Re-running with that exact known failure excluded passed 254 library/integration
tests, with eight ignored and one excluded. Native/WASM compile checks and
formatting checks on the changed Rust files passed.

Browser fixture: build bindings with `./scripts/build-web.ps1 -Debug`, start
`npm run serve` under `tools/braccato-widget`, then run
`npm run test:player-performance -- ../../target/NEW_OUTPUT_DIRECTORY`.
It uses isolated browser storage and local mock audio/API routes, records
screenshots/report in a required new directory, and exercises transport, likes,
volume-save coalescing, lifecycle flushing and reload persistence. No personal
library or live-service requests are required.

The final 2,000-track browser fixture passed with 13 full-store writes for 60 slider
events; selection after removing a liked row kept the correct track identity.
Hidden visibility and pagehide each flushed a pending
change immediately, and reload retained the latest state. A separate responsive
search diagnostic passed 1440x900, 320x640, 390x844, 430x932 and 844x390, including
Unicode queries, playback toggles and lyric focus. The first concurrent run of
the existing search fixture timed out waiting for playback; its diagnostic
rerun passed all sizes without changing application code.

`cargo build --release --locked --bin vey` and
`./scripts/build-web.ps1 -Debug` completed. These are local outputs, not packaged
or published releases. Native audible/device playback and global frame-time
measurements were not part of this pass.

## Windows artwork-event burst follow-up (2026-11-05)

Source inspection found that every completed/aliased cover called the full
playlist refresh and rebuilt the queue model. The sidebar refresh cloned and
deduplicated the whole library, then recreated every playlist row; thus a wave
of cover completions repeated catalog work for each image. Download progress
also scanned all mounted page rows per event. The native controller now patches
the matching cover/sidebar/destination/queue rows, updates download state by
track path, indexes playlist-cover resolution once per full refresh, and creates
page models only when that route is opened. Desktop system-media position is
published once per second, with state changes and seeks immediate.

Fresh Windows validation: `cargo check --locked --bin vey` and
`cargo build --release --locked --bin vey` passed. The release executable is
82,426,368 bytes (78.61 MiB), SHA-256
`1F055449E2C7F5BDC6C7AEFAEBE8D699AB91724FDECB07EE3F01305C6450447B`; build
time was 5m24s. The app was launched with its populated local library. Seven
Home/Library/Liked/Playlist route switches completed in 1.94s, and the first
playlist open showed its collage and track covers without reopening it. No audio
was started. This is one interactive Windows smoke session, not a long playback
soak or an Android runtime check.

The separate release `native_lyric_stutter` fixture ran before the controller
changes. It used 300 synthetic lines with eight timed words and a 16ms sample
timer; it does not load audio, a real library or cover art. Phase values are
event-loop gaps (p95/max, ms):

| Mode | Entry phase | Home phase | Return phase |
| --- | ---: | ---: | ---: |
| Default effects | 23.63 / 112.55 | 21.43 / 62.25 | 26.12 / 133.97 |
| No navigation blur | 26.15 / 147.63 | 17.49 / 79.95 | 29.70 / 87.09 |
| Reduced motion, glow, wobble and focus blur | 19.59 / 71.65 | 16.81 / 20.43 | 17.22 / 57.48 |

Reduced motion improved steady phase gaps in this fixture, while occasional
entry/return spikes remained. This result is separate from the artwork-event
fix and does not prove all rendering stalls are gone.

## Native lyric and ambient workers (2026-11-05)

The follow-up report concerns uneven word fill and line changes while transport
continues. Whole-song shaping/wrapping now runs on one bounded worker with one
replaceable pending request. Generation checks reject obsolete song/resize
results. Only owned cue data, strings and numbers cross threads; Slint models
stay local. Hidden documents can prepare before player entry, with animation
and camera updates dormant. Unavailable initial geometry can require another
layout when the player opens.

Ambient resize/blur/color preparation and frame composition also run on one
bounded worker. The UI uploads completed pixels, with at most one frame in flight.

Fresh checks: 41 native lyric tests and three ambient-related tests passed,
including a new latest-request/cue-preservation test. Windows release binary and
diagnostic example built successfully. Executable: 82,533,376 bytes, SHA-256
`E85DFAB9A5F1067766E1854E7CA0AA1BCC64C01638DAAC0AA105F3C670DF22E7`.

Diagnostic command:
`cargo run --release --locked --example native_lyric_stutter -- LABEL --audio --soak --no-blur`.
It generates a quiet WAV, runs the native audio service, paints 300 synthetic
timed lines, animates the backdrop and switches Home/player routes for 45 seconds.
Audio advanced to 143.004 seconds after its initial seek. Applying prepared layout
took 1.63 ms for 1,200 wrapped rows. The earlier synchronous sample took 16.11 ms;
ambient/audio workloads differ, so this is not a matched end-to-end comparison.

| Phase | Event-loop gap p95 / max, ms | Gaps over 50 ms |
| --- | ---: | ---: |
| First player entry | 24.21 / 117.80 | 1 |
| Home during playback | 17.95 / 49.29 | 0 |
| Returned player, 37 seconds | 27.90 / 63.80 | 11 |

Render elapsed time: p95 16.13 ms / max 105.32 ms. Snapshot capture happens after
measurement; JSON/PNG: `target/native-lyrics-check/stutter-final-audio-soak.*`.
Drawing spikes and uneven frame pacing remain; this is not a stutter-free
renderer or a 60 FPS guarantee.

A normal release-app check used isolated persistence, generated local audio,
cached Ukrainian timed lyrics and the user's appearance settings (Roboto,
weight 900, animations enabled). Play, lyric entry, Library navigation, return
and pause responded; visible transport advanced through 25 seconds. Layout
application reported 0.90 ms at the final viewport width, with no lyric update
log over 8 ms. Default Music-folder discovery still ran in this isolated profile;
no original library files or original persisted preferences were modified.
This short check does not measure underruns or establish long-term, Android or
browser stability.

## Stable native fill models (2026-11-06)

The feather repeater now uses a capacity derived from static ramp/fragment/glyph
geometry and layout width, bounded to 64 cells. The current playback position no
longer changes its integer model. Slint 1.18.1's repeater resets its instances
when a newly bound model replaces the previous one; changing the old count
therefore recreated text delegates during a cue. Solid ink keeps its original
conditional Text; only the active partial-fill body owns a pool.

Both broader retention experiments were rejected. Holding hidden pools for all
viewport words measured render p95 37.06 ms and returned-player event-loop p95
59.10 ms. Keeping a hidden neutral Text during partial coverage measured render
p95 19.38 ms (18.66 ms on repeat). The delivered variant retains neither.

An intermediate build before the clock-cadence split below passed
`cargo build --release --locked --bin vey --example native_lyric_stutter`.
Executable: 82,545,152 bytes; SHA-256
`C7098324CB0BD970DC6974B9A5386E7E2D096003BC45702020B76CF369628476`.
The 45-second `cue-pool-final-1106 --audio --soak --no-blur` diagnostic advanced
native audio to 143.003 seconds after its initial seek:

| Phase | Event-loop gap p95 / max, ms | Gaps over 50 ms |
| --- | ---: | ---: |
| First player entry | 30.35 / 119.13 | 1 |
| Home during playback | 17.15 / 84.64 | 1 |
| Returned player, 37 seconds | 30.41 / 100.69 | 11 |

Render elapsed time was p95 16.55 ms / max 102.26 ms. JSON/PNG:
`target/native-lyrics-check/stutter-cue-pool-final-1106.*`.
This fixes clock-driven model replacement, but does not demonstrate a general
frame-pacing improvement over the earlier worker/capped samples. Drawing spikes
remain. Glyph correctness/continuity checks are recorded in
`docs/NATIVE-BRACCATO.md`; no Android/browser or real-song audible certification
is implied by this diagnostic.

## Native fill clock cadence (2026-11-06)

Running the entire native lyric handler every 8ms regressed the diagnostic's
returned-player event-loop p95 from 26.43 to 34.04ms. That design was rejected.
The delivered handler uses an intermediate pass that only samples/projects
`PlaybackClock` and publishes `lyrics-position`. Layout, row residency, focus,
camera and paint preparation keep a 16ms deadline. Entry and explicit seeks
bypass it; hidden/paused polling stays at 16ms and minimized work is suspended.

The example now records rendered start-to-start intervals and the lyric clock
observed by each rendering callback, separately from its 16ms heartbeat.
These are rendering callbacks, not measured monitor presentation or audible
latency. Snapshot readback remains outside the measured interval.

Four sequential 45-second native-audio runs used the same release executable,
`SLINT_SCALE_FACTOR=1`, an animated generated cover and `--audio --soak --no-blur`.
The order was 16ms, split 8ms, split 8ms, 16ms. Set
`VEY_LEGACY_LYRIC_CADENCE=1` for 16ms; unset it for the delivered split cadence.
Returned-player results (37 seconds per run):

| Run | Repeated lyric-clock frames | Clock step median / p95 / max, ms | Render interval p95 / max, ms | Intervals over 33ms |
| --- | ---: | ---: | ---: | ---: |
| 16ms first | 60.0% | 16 / 25 / 97 | 16.86 / 83.63 | 18 |
| Split 8ms first | 28.0% | 9 / 19 / 46 | 16.12 / 94.40 | 9 |
| Split 8ms repeat | 27.3% | 9 / 20 / 50 | 16.71 / 50.38 | 16 |
| 16ms repeat | 59.9% | 16 / 25 / 89 | 16.49 / 88.48 | 17 |

Render elapsed p95 was 11.16/11.14ms for 16ms and 10.52/10.59ms for split 8ms.
The heartbeat p95 did not improve: 24.06/24.68ms versus 29.74/29.25ms.
Audio remained playing and advanced to 143.011–143.054 seconds after its
100-second seek in every run. The split cadence improves the granularity of
painted fill time; it does not remove all drawing spikes or establish global
UI responsiveness. Android, browser and real-song audible checks were not run.

Reports: `target/native-lyrics-check/stutter-split{16,8}-{final,repeat}-1106.json`
and the corresponding `-frames.json` (`[phase, interval_ms, lyric_time_ms]`).
Windows release build passed. Executable: 82,565,120 bytes; SHA-256
`0831ED1B2C1235D64619419F6361FA05A357D7C695B9362532C0547ABB21D117`.
Final-source regressions passed: 41 tests selected by
`cargo test --locked --lib native_lyric`, `native_lyric_timing` (2x playback,
grouped cues, moving retarget and paused seek), and `native_fill_continuity`
(8,129 forward/zero backward pixel transitions, 37/40 advancing samples).
Focused Rust formatting and diff-whitespace checks passed.
