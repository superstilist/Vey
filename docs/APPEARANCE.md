# Appearance and smooth transitions — Vey 0.1.9

Open Settings → Вигляд (Appearance).

- **Динамічні / Dynamic:** the full tonal palette follows the current cover.
- **Статичні / Static:** the selected fixed palette is used for all songs;
  decorative artwork background rendering stops and its texture fades away.
- **Лише фон / Background only:** neutral surfaces and the artwork texture
  follow the cover; accent/container/text roles retain the selected palette.
- Six fixed hues are available for Static and Background only. The choice and
  mode persist in native state, browser storage and JSON backups. Older settings
  default to Dynamic with the lime hue; unknown mode names fall back to Dynamic.
- **Менше руху / Reduced motion:** color changes and page travel apply directly.
  This uses the existing reduced-motion preference shared with lyrics.

## Implementation

Kawarp uses opacity 0.28, warp intensity 1.00, 15 blur passes, saturation
1.50 and dithering 0.020. Bright artwork is dimmed before filtering; no extra
color tint is applied. The native lyric backdrop uses the same opacity and
pixels; the phone player sheet also displays this texture. Artwork and theme
colors both fade over one second. Theme extraction
groups neighboring hues and favors dominant areas over small vivid details;
neutral covers produce neutral dynamic themes instead of lime-tinted surfaces.

`src/material.rs` samples artwork only when its image changes, retaining the
last hue during empty/loading artwork. Color targets interpolate for one second
using smootherstep, with zero velocity/acceleration at both ends. Interrupted
fades start from the displayed palette. A finite 32 ms timer updates colors and
stops at the endpoint; palette extraction never runs on this timer.

During a fade, off-screen page surfaces skip painting and speculative warming
pauses. Objects, page models and scroll positions stay mounted; the destination
preparation frame and intersecting camera pages remain visible. This limits
competing redraw work while backgrounds and controls change color.

Desktop page travel uses `cubic-bezier(0.45, 0, 0.55, 1)`, 240–520 ms according
to remaining distance (380 ms for an adjacent cell). Slint, browser camera
presentation and Rust's camera-time inverse share this curve. Settings sections
fade with the same easing over 170 ms per phase. Retargeting continues from the
current camera position. Mobile Player retains its separate page overlay.

The native lyric backdrop uses the current surface and the shared animated
texture opacity, so switching to Static also fades that background consistently.
Windows/browser use the same locally bundled widget and camera adapter.

## Checks and limits

Commands: `cargo test --locked --lib --tests`, `npm run test:appearance`,
`npm run test:material`, `npm run test:transitions`, `npm run test:phone`.
Widget/browser commands run in `tools/braccato-widget` with its fixture server.
Reports and screenshots record their actual run scope; fixtures do not establish
physical Android behavior or guarantee a frame rate on every machine.

2026-10-20 validation: 157 Rust tests passed, seven environment-dependent tests
ignored; debug WASM and the widget bundle built. Appearance, material, phone and
eight-direction transition browser fixtures passed. Twelve native synthetic
Android-inset layouts passed, including the 320-pixel-wide Appearance page.
`appearance-check.json`, `navigation-performance.json` and screenshots contain
the targeted browser evidence. No physical Android runtime check was available.

Release packages: `dist/Vey-0.1.9-windows-x64.zip` (CRC and SHA-256 checked)
and `dist/Vey-0.1.9-android-arm64.apk` (signature, 16 KiB alignment, manifest and
SHA-256 checked). Hashes and sizes are in `appearance-release-check.json`.
