# Player interaction performance

2026-10-29, source version 0.1.33. This pass covers catalog/list/controller
work on Windows x64 and the shared WASM controller. It does not establish
all-song, audio-device, Android or Linux runtime performance.

## Player loading and page work (2026-11-01)

Native artwork loads use a bounded queue: two background requests and one
reserved slot for the current track. A view refresh replaces queued offscreen
work; in-flight results still update retained models. Local artwork is decoded
once, with corrupt candidates skipped. Browser artwork uses the same limits,
prioritizes the selected song, bounds response bytes and aborts stalled fetches.
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
