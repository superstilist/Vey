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
travels to the destination coordinates over 180–460ms according to the
remaining distance (320ms for a neighboring cell); every visible layer uses
the same world-position-minus-camera transform. Diagonals move on both axes,
and opposite destinations span two cells. Retargeting starts from the current
camera position rather than restarting a canned entrance animation.
Only content moves. The background, navigation, and playback bar stay fixed.
The complete lyric viewport moves as one object with the player. Its text,
scrollbar and lyric controls keep their local layout throughout camera travel.
Reduced motion moves the camera to its target immediately.

Eight route surfaces and the player surface stay mounted at viewport size.
While idle, one page at a time is painted through a clipped edge strip; the
currently visible page is already warm. A destination gets a brief preparation
frame before travel. Slint's rendering-cache hint keeps these surfaces ready
for compositing, trading viewport-scaled render-cache memory for fast returns.
Normal model and size dependencies refresh the relevant cache when content or
the viewport changes. Warming pauses during navigation. The player shell
remains mounted between visits.
The lyric host promotes its moving content to a compositor layer and skips
unchanged presentation styles and visibility attributes on clock updates.
The shared ease-in-out curve is `cubic-bezier(0.42, 0, 0.58, 1)`. The bridge
supplies the camera epoch, start/target coordinates and elapsed time and
duration. The browser host joins that same curve using Web Animations, so it moves
between IPC samples even when playback is paused. Duplicate samples preserve
the animation; reduced motion, editor mode and hiding cancel it. The camera
adapter is shared by the browser host and standalone widget. Grouped hosts
disable inner lyric translation to avoid moving the lyrics twice. A rounded
stage clips the moving viewport in both browser and Windows hosts. Windows
moves the child without resizing its document and skips hidden geometry work.
Native placement samples use a 16ms cadence during player camera travel,
returning to the usual 32ms bridge cadence when the camera settles.
The Slint fallback lyric list is instantiated only without the external host.
Decorative background readback drops to roughly half its normal cadence while
content moves, without resetting the warp clock, then returns to normal.
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

On mobile, the player uses a separate overlay sheet rather than the world-grid
camera. Its retained page keeps the normal viewport size and window position,
including settings-section scroll offsets. The sheet fades and moves 48 pixels
over 220 ms; close and browser Back reveal the same page. Desktop keeps the
player's northeast route. Reduced motion removes the sheet animation too.
