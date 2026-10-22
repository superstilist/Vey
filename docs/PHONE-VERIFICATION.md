# Phone layout and settings — Vey 0.1.2

The phone layout uses a combined playback/navigation dock, 44-pixel settings
actions, and a scrollable home. Settings now has an overview
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

## Compact search refinement — 2026-10-22

The Android layout fixture now includes Search at 320×640, 390×844, 430×932
and 844×390 with synthetic safe-area insets. Those four cases passed and the
320 px, 390 px and landscape screenshots were inspected. These are native
simulations, not physical-device screenshots. Mobile search and typing/browser
checks cover the same dimensions plus desktop; package verification is separate
from on-device keyboard and installation acceptance.

Final checks for this refinement: `android_layout` passed 19 cases;
`npm run test:search` passed 1440×900, 320×640, 390×844, 430×932 and 844×390,
including touch-button search submission and typing during audio playback.
`npm run test:phone` passed its navigation, overlay, input and persistence checks.
The WASM debug build passed. The Android ARM64 APK was rebuilt and passed
signature (v2/v3), alignment, ABI and manifest checks using the existing local
development key. Artifact: `dist/Vey-0.1.15-android-arm64.apk`;
SHA-256: `a59c08e9717ea56a7af67d6c83756483f108704d6bf404baa13de3bfb00a435b`.
Physical Android installation, keyboard/IME and playback are still unverified.

## Unified mobile dock — 2026-10-22

Playback controls, the seek rail and labeled page selection now live in one
rounded, clipped surface. Navigation keeps its own touch row below a divider.
The dock is 112 px tall normally and 104 px in short viewports, plus the shared
bottom safe area and outer gap. Normal phone content gains 32 px of height. The mobile playback row omits like
and expand icons; tapping the track opens the full player.
The same viewport calculation keeps the mounted underlay stable while the
full player opens. Desktop retains its existing 72 px playback bar.
The Android fixture checks continuous dock background pixels across the join.

Final dock verification: native `android_layout` passed all 19 cases, including
continuous joined-surface pixel assertions. Portrait 320 px and landscape
screenshots were visually reviewed. The WASM debug build and `test:phone` passed,
including track-tap opening and exact underlay scroll preservation.
`test:search` passed all five sizes (1440x900, 320x640, 390x844, 430x932,
844x390), covering typing and Unicode during playback. Android
ARM64 packaging passed signature v2/v3, alignment, manifest and ABI checks.
APK SHA-256: `f09ba6e1b34972d37fa1113defb5039eec24c4b8cf7887f3a664feea63c7cac2`.
Physical-device installation, IME and playback remain unverified.

## Open canvas cleanup - 2026-10-22

Removed decorative page, home-card and settings-group fills. Placeholder artwork
shares one neutral tone; mobile tabs and settings icons no longer have nested
filled backgrounds. Base and dynamic palettes use near-neutral dark surfaces
and text, keeping artwork-derived accents and existing appearance modes.

Validation: all 19 native layout cases passed; inspected narrow search, home,
settings and desktop/tablet screenshots. All four material unit tests passed,
including actual palette-role contrast checks across hues. WASM debug build,
`test:phone`, `test:material` and `test:appearance` passed. Appearance test menu
coordinates were corrected to target Appearance instead of Language; the material
test now opens the player at the current mobile dock position.
Android ARM64 APK passed signature v2/v3, alignment, ABI and manifest checks.
SHA-256: `14f30d2894f42209fffbba5835aadb8b20b7b16f55f8fd23f385a7134027f328`.
Physical Android testing was not performed.

## Rotating background and mobile targets - 2026-10-22

Kawarp output now rotates slowly in normalized coordinates with overscan.
Android/Linux native fallback adds rotation and bilinear sampling of its prepared
256x144 texture. Rotation follows the existing playback clock; paused targets
are cached and reduced motion freezes the texture. Mobile shared actions and
brand transport now use 44 px targets, with pressed feedback and matching filter
row height. Dock padding accommodates the targets without increasing its height.

Passed: 19 native layout cases, the native rotation/opacity/frozen-frame unit
test, WASM debug build, `test:ambient`, `test:phone` and `test:search` at five
sizes. Search regression now taps the outer edge of the enlarged mobile play
button while checking real audio pause/resume and subsequent input. Inspected
native narrow-player layout and browser rotation frames. Android ARM64 packaging
passed signature v2/v3, alignment, manifest and ABI checks.
APK SHA-256: `ddf0f0d3b5b135611b004dd8bc9caa5c79af087a7790af7f58d62240897c7df4`.
Physical-device startup, touch latency and playback are not verified.
