# Playlist menu, volume and normal playback — Vey 0.1.6

Changed 20 October 2026.

- Add-to-playlist uses full-width 64-pixel rows with an eight-pixel gap,
  centered icons, clipped long names and track counts. The list scrolls inside
  the modal while create/cancel remain outside its viewport. Safe-area and
  keyboard-aware modal bounds are retained.
- Native Liked/history navigation entries are excluded from destinations.
  Empty state offers creation; creating from this picker adds the current track.
  Regular playlist creation elsewhere still creates an empty list.
- Volume has an 18-pixel circular thumb, eight-pixel rail, a 44-pixel preferred
  input height and aligned hit/visual geometry. Pointer/touch updates value
  directly; the standard slider retains keyboard and accessibility operations.
  Settings shows volume percentage.
- Playback-speed slider, reset action and callbacks are removed. Old saved
  speed values load as 1x and are omitted when saving settings/backups; internal
  rate-aware audio/lyric APIs remain for clock compatibility.

## Checks

- `cargo test --locked --lib --tests`: 155 passed, seven ignored. Includes
  loading older settings without retaining an inaccessible playback speed.
- `cargo run --locked --example android_layout`: native fixture cases cover
  320-pixel playlist menu, desktop picker, audio settings and Android insets;
  all eleven cases passed. Picker/audio screenshots were visually inspected.
- `npm run test:playlists`: saves a track to the first and final item of a
  20-playlist list, verifies create-and-add, and selects in a 320-pixel picker.
  Screenshots: `docs/screenshots/playlist-picker-*.png`; report `playlist-check.json`.
- `npm run test:phone`: volume tap/drag and keyboard Home/End reach the actual
  audio element; legacy 1.75x state plays at 1x. Existing phone navigation,
  import, overlay and reduced-motion checks also run.
- Rust/WASM build and desktop `npm run test:material` passed, including real
  browser volume changes and keyboard increments. Android release APK passed
  signature, 16 KiB alignment and manifest checks:
  `dist/Vey-0.1.6-android-arm64.apk`, SHA-256
  `d2d1293529f2e0d00d0c24e6e73dc666b2ab41ac5e7d93eee822b2b19e3a1ac4`.

Android installation/runtime remains unverified on a physical device. Native
inset screenshots are Windows Slint fixtures; browser checks use headless Edge.

## 0.1.7 volume fill correction

The colored rail child had no explicit position. Partial values could look
centered rather than growing from the minimum end. Its x/y are now explicitly
zero, retaining the existing value/drag mapping and thumb geometry.

`cargo run --locked --example volume_fill` renders the actual TonalSlider at
0%, 100%, 25%, 50% and 75%. Four samples across the rail compare partial values
with rendered empty/full baselines, asserting that only the left portion is
filled. Screenshots are `docs/screenshots/volume-fill-*.png`. This catches the
visual defect that input-value checks did not detect.

The five native rendered cases passed on 2026-10-20; 25%, 50% and 75%
screenshots were visually inspected. No physical Android runtime pass is claimed.

## 0.1.8 compact volume without a frame

Removed the rounded rectangle drawn around the entire focused volume control,
as shown in the user's desktop toolbar screenshot. Keyboard focus forwards to
the standard slider, but does not paint an enclosing box. The rail still fills
from its left edge. `volume_fill` now renders 312-, 84- and 64-pixel widths at
five levels; the 84-pixel control is focused, and top/bottom pixel probes assert
that no focus frame is drawn. Samples under the thumb are excluded from rail
comparisons on compact controls.
