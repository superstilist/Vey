# Android library and safe-area changes — 20 October 2026

Vey 0.1.5 addresses two source-level causes of the Android problems: scanning
relied on desktop HOME/Music discovery without audio permission, and the shared
UI safe-area properties defaulted to zero on Android.

## Library

- Manifest declares READ_MEDIA_AUDIO and READ_EXTERNAL_STORAGE capped at API 32.
  `src/android.rs` checks the OS version/grant and requests the appropriate
  permission on Java's main thread. No all-files permission is requested.
- The UI stays responsive while the dialog is open. Grant resumes scanning;
  denial leaves Android-permissions instructions and allows later retry.
  Permission/provider errors do not replace the catalog.
- A worker queries MediaStore external audio, including indexed audio outside
  Music and on removable storage. Rust metadata/artwork/playback retain their
  filesystem paths; stale, unreadable and unsupported records are skipped.
  Discovery covers indexed audio, not arbitrary private files of other apps.
- Tracks merge with manual sources and deduplicate canonical paths before
  repeated metadata parsing. Raw AAC is accepted. Concurrent scans are suppressed.
- Android library/settings actions start device discovery. Counts/access messages
  are shown and mobile messages wrap; manual path entry remains available.

## Layout

- Shared inset properties bind to `Window.safe-area-insets`: Android supplies
  status/navigation/cutout dimensions; browser CSS overrides still work. Values
  update with configuration changes.
- Header, content, sidebar, bottom controls and player share safe bounds.
  Android phone landscape retains mobile navigation rather than using a
  desktop layout that requires a taller window.
- Modals center inside safe bounds above the virtual keyboard; constrained
  modal content scrolls. Short page viewports can scroll to track lists rather
  than collapsing the lists beneath headers/status messages.

## Reproducible checks

- Rust suite run on 2026-10-20: **154 passed, seven ignored, zero failures**.
  Native synthetic-inset layout run: **eight cases passed**; portrait settings,
  landscape library, player and tablet screenshots were visually inspected.
- Android release APK built successfully; signature, 16 KiB alignment and
  manifest verified. Package is `com.playervf.rust`, version 0.1.5; audio
  permissions and the legacy API-32 cap were confirmed in the packaged manifest.
- Rust/WASM build and `npm run test:phone` passed, including 320/360/390/430-pixel
  layouts, actual browser audio controls, file import, settings/history navigation,
  player overlay restoration and no browser errors. These remain browser fixtures.
- Output: `dist/Vey-0.1.5-android-arm64.apk`, SHA-256
  `ce6b27d1ac3544b7978e155d82ae834806a22f2e33fe727faa62487ac5a26ff6`.
- `cargo test --locked --lib --tests`: includes an indexed-library regression
  with Unicode names, duplicate/custom sources, deleted records, unsupported
  files and actual WAV metadata parsing.
- `cargo check --locked --lib --target aarch64-linux-android`: configure NDK
  environment as in the CI workflow.
- `cargo run --locked --example android_layout`: asserts content bounds and
  responsive modes; generates `docs/screenshots/android-insets-*.png`. Cases
  cover 320/390-pixel portrait, landscape left cutout, tablet, library/settings,
  player and source modal. Synthetic native Slint insets on Windows, not
  Android-device captures.
- `scripts/build-android.ps1`: release APK, signature, 16 KiB alignment and
  manifest checks, then APK/SHA-256 under `dist/`.

## Remaining device acceptance

No connected Android device/emulator was available. Verify first launch,
grant/deny/permanent denial, granting from system settings, revocation, rescanning
added/deleted audio, removable volumes, metadata/artwork and actual playback.
Check rotated cutouts, gesture/three-button navigation, keyboard visibility,
settings scrolling, background/resume and relaunch. Direct media-path access
needs coverage on older Android, especially Android 10 scoped storage.
Full SAF import/export remains pending.

API references: [Android shared media](https://developer.android.com/training/data-storage/shared/media),
[Android 13 permissions](https://developer.android.com/about/versions/13/behavior-changes-13),
[Slint safe areas](https://docs.slint.dev/latest/docs/slint/guide/platforms/mobile/general/).
