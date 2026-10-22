# Search keyboard focus — 2026-10-22

## Changes

- Windows reproduced a split focus: clicking the Slint search field after a
  WebView interaction moved Slint's caret but left Win32 keyboard focus in the
  child. The shared host now restores native focus on a pointer press delivered
  to the shell. WebView/editor presses keep their own keyboard focus.
- A focused child returns focus before being hidden. Video visibility changes
  only when its visible state changes, rather than issuing ShowWindow each tick.
- Silent Windows auth windows and their WebViews no longer request focus.
- Android silent auth WebViews block focus and descendant focus before attaching
  to the activity. Closing a focused challenge restores the previous attached
  view without overriding another field the user has since selected.
- The shared search field keeps its input and clear-button slot mounted at a
  stable width. Padding forwards focus, clearing returns focus to the input,
  and navigating back to Search requests focus once after the header is visible.

## Regression checks

Native Windows: generate the existing video fixture with `npm run test:video`
inside tools/braccato-widget (fixture server required), then run
`cargo run --locked --example search_input`. The test sends Win32 mouse and key
messages to its own window. It checks typing before playback, focus transfer
from a real WebView2 child, hiding that focused child, eight playback/track
cycles, Unicode, punctuation and exactly one Enter submission. The initial
version failed at native focus transfer before the fix.

Browser: `npm run test:search` checks desktop and phone layouts, real fixture
audio, repeated edits before/after playback, returning from iframe focus,
Unicode, punctuation, pause/resume and submission without requests on edit.
The report is written under target/, with no real library or account data.

Android compilation and native Linux/device runtime are separate checks;
compilation is not proof of keyboard/IME behavior on a physical phone.

## Verification on 2026-10-22

- Native Windows regression passed all eight cycles, Unicode/punctuation and
  Enter submission. The pre-fix test failed with keyboard focus still in the
  WebView after a shell click.
- Final rebuilt WASM passed `npm run test:search` at 1440×900 and 390×844,
  including select-all/backspace replacement, Cyrillic/Japanese, iframe focus,
  pause/resume and no search request before Enter.
- Rust tests: 191 passed, seven environment-dependent checks ignored.
- WASM debug build and Android ARM64 compile check passed. The Android Java
  bridge was recompiled by the Android target build; existing unused-helper
  warnings remain. NDK CC/AR were configured for that check.
- Physical Android IME and native Linux runtime were not tested here.
- Final optimized Windows build completed and was packaged with
  `scripts/build-release.ps1 -SkipBuild`. The packaged executable matched the
  build SHA-256 and completed `--widget-smoke` with exit code 0. WebView2
  diagnostics reported 20 samples, seven lyric lines and rendering status.
