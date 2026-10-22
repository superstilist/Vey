# Automatic lyrics and responsive player — 2026-10-22

Native Windows/Android lyric lookup starts when a track is selected, in parallel
with stream resolution and decoding. Public provider results are displayed as
they arrive; later results can improve the selected quality. A saved per-song
choice still takes precedence. Provider quality is derived from actual timing;
line-only sources never acquire synthetic word timestamps.

Only one native lyric task is retained. Track changes, explicit refresh and
successful manual imports cancel the previous task; generations reject queued
stale results. Restoring the previous track after audio failure also restores its
identity and lyric lookup. Disk-cache write failures do not hide fetched lyrics.

The phone player places lyrics above the track identity, seek bar and transport.
Elapsed/duration labels remain visible at 320 px and secondary actions have
44 px touch targets and accessible names. Phone lyric sizes are 28–32 px;
desktop sizes are 40–56 px, matching the native fallback and web widget. Existing
safe-area insets, cover palette, reduced motion and lyric timing remain in use.
The baseline-ui skill supplied by the user informed this polish pass:
https://github.com/ibelick/ui-skills/blob/main/skills/baseline-ui/SKILL.md

## Validation

- Rust suite: 191 passed, 7 environment-dependent tests ignored. Includes a
  gated-provider regression proving public lyrics appear before auth completes
  and a later timed result upgrades the selected quality.
- Widget bundle rebuilt; `npm run test:sync` passed.
- Native `android_layout` fixture passed 15 cases, including 320×640,
  390×844 and 1440×900 lyric views. Final 320 px screenshot inspected.
- WASM check and debug web build passed.
- Android ARM64 `cargo check --locked --lib --target aarch64-linux-android`
  passed with NDK CC/AR configured (existing unused platform-helper warnings).
- Windows `--widget-smoke` exited successfully with WebView2 initialized;
  widget diagnostics reported 7 rich-sync lines, a 256 px ambient frame and
  final mobile host bounds 342×496 at (24,150). Slint snapshots do not capture
  the separate embedded WebView surface.
- `npm run test:phone` passed 320/360/390/430 px layouts, input, persistence,
  navigation and overlay/scroll retention. Updated stale test coordinates for
  the current Appearance/Data settings and player dismissal control.
- Formatting and diff whitespace checks passed.

These checks do not establish live lyric availability or Android device
behavior. No adb device/emulator was connected.
