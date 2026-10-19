# Vey validation — 19 October 2026

Vey keeps the existing standalone Rust/Slint architecture and PlayerVF data
locations. The shipped binary is now `vey.exe`. The supplied, unmodified PNG
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
