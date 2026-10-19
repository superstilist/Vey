# Verification — 18 October 2026

Artwork freeze fix (18 October): reproduced Windows reporting the old app as not responding during library artwork loading. The controller previously rebuilt the complete track model and reloaded every earlier PNG for each artwork result. Artwork now decodes on at most two background workers, is bounded to 768 pixels without upscaling small images, and reaches Slint as decoded pixels. Individual model rows update without rebuilding the list. Requests are deduplicated, all displayed tracks request art (the previous 40-track cutoff was removed), corrupt cache entries are retried, and WebP/GIF/BMP and folder-cover fallback are supported. A real library cover was visually confirmed in Now Playing while the playback clock and synced lyrics advanced; Windows reported the process responsive. Four regression tests cover WebP, resizing, malformed/oversized input, corrupt cache and folder artwork.

| Check | Observed result |
|---|---|
| `cargo test --lib --tests` | **98 passed**, 7 ignored by default: 90 unit tests + 6 parser fixtures + 2 service integration tests |
| Native audio smoke (explicit ignored test) | **1 passed**, silent WAV through the real output device: load, play, pause, seek, rate, volume, invalid-load recovery, stop |
| Live public YTM search | **1 passed**; songs, videos and paired clip IDs returned |
| Windows Slint screenshot run | 13 PNGs generated for desktop, tablet and mobile layouts using fixed fixture data; screenshot mode does not save state or play audio |
| Windows UI check | Library navigation exercised via the visible sidebar; custom button accessibility actions added after observing missing invocation support |
| WASM build | Built `wasm32-unknown-unknown`; generated bindings under `web/pkg/` |
| Browser runtime | Canvas rendered responsively; public query returned 15 tracks; selection updated the player; clock/URL/startup issues found during the test were corrected |
| Public streaming runtime | **Failed upstream**: HTTP 403 on ranged media requests for both tested tracks. Browser reports the playback error. No successful public streaming pass is claimed |
| Android final check | Passed `cargo check --target aarch64-linux-android --lib` with NDK 28.2.13676358 |
| Android library link | Passed; `target/aarch64-linux-android/debug/libplayervf.so` produced. No device connected; no on-device smoke or APK acceptance |
| Linux | Not verified: Ubuntu WSL failed to attach its missing `ext4.vhdx`; Linux CI supplied, not executed |
| Turnstile | Local HTTP fixtures verify missing-JWT and HTTP-error handling; no real challenge was solved by automation |

[Original/native screenshot comparison](screenshots/comparison.png) uses the original project's screenshots and the current Slint fixture renders. It does not assert exact pixel parity.

Raw test/build evidence is in `verification/`. Remaining functional differences are in [PARITY.md](PARITY.md).

## Minimal player design update

The supplied YouTube Music references guided the charcoal palette, larger home headings, album shelf spacing and focused Now Playing layout. The focused desktop player hides the navigation and bottom bar, places artwork and a seek/playback area on the left, and uses a large lyrics column on the right. The monitor button returns to the standard player. Provider selection, offsets, paste/copy and track actions remain available under the settings button. Custom lyric theme sizes remain respected. The home page and album shelf now scroll, and the bottom progress line is anchored to the top of the player bar.

Validation: `cargo test --lib --tests` passed (98 tests, 7 ignored); `cargo check --lib --target wasm32-unknown-unknown` passed. Fifteen Slint renders cover desktop, tablet and mobile screens, including focused, standard and expanded-tools player states. Optional `PLAYERVF_SCREENSHOT_ART_DIR` supplies local cover images for visual QA; fixture titles remain synthetic and do not describe those images. A live Windows observation also confirmed the redesigned player with real local artwork and lyrics during playback. No Android-device or Linux runtime claim is made for this design update.
