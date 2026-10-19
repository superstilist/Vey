# Phone layout and settings — Vey 0.1.2

The phone layout uses labeled bottom tabs, a separate mini-player above them,
44-pixel settings actions, and a scrollable home. Settings now has an overview
and focused Audio, Library, Lyrics, and Data/sign-in pages. The back button
stays visible while forms scroll. Repeated Settings taps return to its overview.
Dialogs fit narrow screens; the logo is smaller and transparent (see LOGO.md).

Page travel takes 180–460 ms according to distance, with a 16 ms preparation
frame. Settings sections fade out/in over 140 ms each and retarget to the latest
selection. Reduced motion switches directly. Lyric tools fade and slide without
changing the lyric viewport. Explore revisits use cached results; Refresh still
makes a new request. Browser Back/Forward restores pages and settings sections.
On mobile, the player is a 220 ms overlay sheet over the retained page. Opening
or dismissing it does not move the page camera. Closing returns to the same
page, settings section and scroll position. Desktop keeps its camera route.
CSS safe-area insets are applied to the header and bottom controls.

## Verification

- Rust: 153 tests passed, seven environment-dependent checks ignored.
- Rust/WASM build and widget bundle succeeded.
- Touch-emulated Edge: 320x640, 360x740, 390x844, 430x932. Tested playback
  rate/volume changes against the actual audio object, rate reset, local WAV
  import, reduced-motion persistence, settings navigation, browser Back/Forward,
  player overlay dismissal and preserved scroll, rapid tab changes, cached
  Explore and explicit refresh. No browser errors.
- Inspected screenshots 45–51, including the bottom of the narrow Data form.
- Desktop/mobile lyrics integration, Material palette/input and eight-direction
  transition checks passed.

Run the fixture server in tools/braccato-widget with npm run serve, then
npm run test:phone. Raw report: phone-check.json. These are browser emulation
checks; physical phone hardware, Android packaging, and real notch/keyboard
behavior were not tested. They do not establish a guarantee of zero freezes.

The optimized Windows 0.1.2 executable completed --widget-smoke with exit code
zero. WebView2 reported rendering, seven lyric lines and a 256-pixel ambient
frame; the final mobile lyric bounds were 342x484 at (24,298). Inspected native
mobile screenshots 21 and 27. Data was isolated under docs/smoke-data, outside
normal user settings. The release archive SHA-256 was verified.
