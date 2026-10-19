# Vey validation — 19 October 2026

Vey keeps the existing standalone Rust/Slint architecture and PlayerVF data
locations. The shipped binary is now `vey.exe`. The supplied PNG (original in 0.1.0/0.1.1)
is embedded as the window/header logo and provided as the browser favicon.
The native and source design tokens use its lime accent.

## Responsiveness changes

- Keep the existing mounted page surfaces, interruptible camera path, prepared
  destination and compositor-driven lyric host.
- Drain native worker results every 16 ms, with at most eight events per batch
  and a yield after an event when the batch has exceeded 3 ms. This is a budget
  between events; an individual large library update can still exceed it.
- Poll playback status independently at roughly 100 ms.
- Coalesce native save requests for 200 ms before copying the current store.
  A dedicated disk writer serializes and writes outside the UI thread, retaining
  at most one pending snapshot. Normal shutdown flushes the latest snapshot.
- Wait for native decoder replies and the track-load serial gate on blocking
  workers, leaving async workers available for network requests.

## Checks

- `cargo test --locked --lib --tests`: 151 passed, seven ignored (live network
  and output-device checks). Includes pending-save shutdown and disk-error tests.
- `scripts/build-web.ps1 -Debug`: Rust/WASM compilation and bindings succeeded.
- Widget build and `npm test`: passed, including unchanged paused samples,
  hidden animations, reduced motion and no per-tick lyric rebuilds.
- `npm run test:app`: passed desktop/mobile startup, lyrics, clock/seek,
  ambient bridge, settings overlay, and close/reopen checks.
- `npm run test:transitions`: passed all eight directions, interrupted travel,
  stable lyric bounds, nine-cell gestures, and retained scroll positions.
- `npm run test:stability`: passed delayed samples, retained lyrics, pause,
  and zero scroller mutations across 100 steady updates.
- Optimized native `--screenshots`: completed desktop, mobile and tablet renders.
- Optimized native `--widget-smoke`: exited with code zero; WebView2 reported
  rendering, seven lyric lines, and a 256-pixel ambient frame. Test data was
  isolated under the ignored smoke-data directory. See `vey-native-smoke.json`.

The development WASM navigation sample recorded a median animation-frame gap
of 16.7 ms, p95 77.8 ms and maximum 150 ms during concurrent compilation.
This is a headless Edge sample, not an optimized native benchmark or a promise
of zero dropped frames. No before/after speedup percentage was measured.
See `navigation-performance.json` and the widget JSON reports for raw results.

Tests use fixture audio and mocked services. Live online-service parity and
Linux/Android runtime behavior were not revalidated in this change.


## Vey 0.1.1 tonal design

The style change is described in `MATERIAL-STYLE.md`. The Rust suite now passes
153 tests, with seven environment-dependent checks ignored. Two added tests
cover neutral/transparent artwork, distinct warm/cool hue selection and readable
primary/foreground contrast across 24 sampled hues. The native and WASM builds
share the same artwork-change callback and palette roles.

The dedicated browser material check verifies actual warm/cool surface pixels,
retaining the previous palette during a delayed cover download, palette retention
across navigation, and volume control through pointer and keyboard input.
See `material-check.json` and screenshots 41–44 for the current run.

The 0.1.1 optimized Windows executable also completed `--widget-smoke` with
exit code zero, seven rendered lyric lines, and a 256-pixel ambient frame.
The archive checksum was verified before publication.

## Vey 0.1.2 phone and settings update

See PHONE-VERIFICATION.md and phone-check.json for responsive layouts, real
settings actions, file import and Back/Forward checks. The app now embeds a
transparent edit of the supplied logo; LOGO.md records the asset and edit prompt.

The optimized 0.1.2 Windows build also passed --widget-smoke (exit 0, seven
lyric lines, 256-pixel ambient frame). Its release checksum was verified.

## Vey 0.1.3 visual cleanup and release

Player cover/metadata panels are transparent on desktop and mobile. Loaded
artwork no longer has a colored fallback or music-note icon underneath; missing
artwork retains its placeholder. Player covers use consistent rounded corners.
Secondary icon actions lose their idle fills, while primary and selected states
remain emphasized. Repeated sidebar/home captions and the normal lyric-provider
footer are removed; renderer errors and nonzero offsets remain available.

This release passed the Rust/WASM build, desktop/mobile lyric integration,
Material palette and audio controls, and phone settings/import/overlay checks.
The optimized Windows executable passed --widget-smoke with exit 0, seven lyric
lines and a 256-pixel ambient frame. The ZIP checksum was verified. Screenshots
35–44 and 45–51 show the refreshed browser UI; native screenshots were refreshed
by the isolated smoke run. These fixture checks do not cover physical phones
or live online-service parity. Rust logic is unchanged in this UI-only update.
