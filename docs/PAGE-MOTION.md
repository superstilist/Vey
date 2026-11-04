# Page motion

Pages use a consistent compass around Home. Incoming content approaches from
the destination's position; outgoing content moves in the opposite direction.
Returning follows the reverse vector.

| Northwest | North | Northeast |
| --- | --- | --- |
| Search | Settings | Player |
| Queue (west) | Home | Explore (east) |
| Playlist | Liked | Library |
| Southwest | South | Southeast |

Pages occupy fixed cells in a viewport-sized world grid. A shared camera
travels to the destination coordinates over 100–180ms according to the
remaining distance (160ms for a neighboring cell); every visible layer uses
the same world-position-minus-camera transform. Diagonals move on both axes,
and opposite destinations span two cells. Retargeting starts from the current
camera position rather than restarting a canned entrance animation.
Only content moves. The background, navigation, and playback bar stay fixed.
Page travel has no animated path decorations; the renderer spends its frame
budget on the cached page surfaces and camera motion.
The complete lyric viewport moves as one object with the player. Its text,
scrollbar and lyric controls keep their local layout throughout camera travel.
Reduced motion moves the camera to its target immediately.

Eight route surfaces and the player surface stay mounted at viewport size.
After the first three seconds, one page at a time is painted through a clipped
edge strip; the currently visible page is already warm. This leaves startup and
the initial Home artwork pass free of offscreen page rasterization. Travel
starts on the route-change callback, without waiting for warming, screenshot
readback or a preparation timer. Slint's rendering-cache hint keeps these surfaces ready
for compositing, trading viewport-scaled render-cache memory for fast returns.
Normal model and size dependencies refresh the relevant cache when content or
the viewport changes. Warming pauses during navigation and palette fades;
off-screen painting pauses during palette fades while pages remain mounted.
The player shell
remains mounted between visits.
The lyric host promotes its moving content to a compositor layer and skips
unchanged presentation styles and visibility attributes on clock updates.
The shared ease-out curve is `cubic-bezier(0.2, 0, 0, 1)`. The bridge
supplies the camera epoch, start/target coordinates and elapsed time and
duration. The browser host joins that same curve using Web Animations, so it moves
between IPC samples even when playback is paused. Duplicate samples preserve
the animation. Normal native and WASM navigation composites live page surfaces;
it performs no full-window screenshot readbacks, CPU page blurs or texture
uploads for navigation. This replaces screenshot-based page blur to prioritize
input latency. Browser lyric-host blur remains on its compositor layer.

`VEY_LEGACY_NAVIGATION_CAPTURE=1` opts native runs into the previous two-snapshot
blur path in `src/navigation_fx.rs` for comparison. It retains the bounded blur
worker, 768x640 texture limit and stale-result rejection, but can delay input
while reading pixels. Clear the variable after diagnostics. This switch restores
capture cost, not the old animation durations. Normal navigation creates no blur
worker or result-polling timer. Each retarget restarts the completion deadline,
including when its duration is unchanged; the camera position stays continuous.
The live player surface uses a full-page render cache only during travel and
offscreen warming; ordinary lyric ticks draw without rebuilding that cache.
Unsupported snapshot renderers retain live content. Native Rust Braccato is
part of the captured Slint content. External browser lyrics
blur by 5.5px at mid-travel. Blur can
be disabled separately in Appearance settings. Reduced motion, editor mode and
hiding cancel it. The camera adapter is shared by the browser host and standalone
widget. Grouped hosts blur without translating the lyrics twice. A rounded
stage clips the moving viewport in both browser and native Slint. The shared
Slint stage supplies its radius (20px mobile, 28px desktop), and the mobile
player clips its children to its own rounded sheet. Native lyrics no longer
use child-window placement or lyric IPC. The copied Rust Braccato view shares
the page camera; browser host geometry updates remain separate.
Decorative background readback drops to roughly half its normal cadence while
content moves, without resetting the warp clock, then returns to normal.
The native ambient texture is updated at most once per 100 ms while a page or
mobile sheet is moving. Its phase and artwork transition clock continue, and
the normal cadence resumes after movement. Set `VEY_LEGACY_AMBIENT_TRAVEL=1` to
compare the previous native cadence.
Paused color morphs reuse a cached GPU target and blend against that target;
they do not redraw or read back identical warp frames. Settled duplicate
requests reuse the encoded frame too. Artwork changes invalidate the target,
and interrupted morphs still start from the currently displayed mixture.

From `tools/braccato-widget`, run the fixture server with `npm run serve` and
then `npm run test:transitions` to check all eight directions, interrupted
navigation, shared backgrounds, and stable player bounds in the WASM app.
The test also records headless-browser animation-frame timing samples in
`docs/navigation-performance.json`; these are environment-specific timings,
not a native-device frame-rate guarantee.

`node tools/braccato-widget/camera-presentation-test.mjs` checks browser blur,
interruption, duplicate samples and reduced-motion cancellation.
`cargo run --locked --example navigation_transition` exercises native captures,
diagonal travel, retargeting, the blur switch and reduced motion, and writes
visual fixtures under `docs/screenshots/navigation-*.png`.
With the WASM assets built and fixture server running,
`node tools/braccato-widget/navigation-viewport-test.mjs` checks the running web
shell's camera samples, sharp chrome and matching desktop/mobile host radii.

Settings sections commit immediately on input, including repeated changes and
Back; there is no fade-out timer or temporary input shield. Secondary panels
use 100ms motion and interaction feedback uses 70ms. These shell changes do not
change lyric cue, fill or follow timing. Reduced motion still snaps immediately.

On mobile, the player uses a separate overlay sheet rather than the world-grid
camera. Its retained page keeps the normal viewport size and window position,
including settings-section scroll offsets. The sheet fades and moves 48 pixels
over 140 ms; close and browser Back reveal the same page. Desktop keeps the
player's northeast route. Reduced motion removes the sheet animation too.
