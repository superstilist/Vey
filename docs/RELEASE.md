# Vey 0.1.12 — Windows x64 and Android ARM64

## New in this release

- Public YouTube Music parsing uses the bundled yt-rust source. Playback handles
  refreshed audio URLs; the Windows lyric host waits for a valid WebView2 handle.
- Permanent downloads embed metadata and cover art and become local library
  tracks. Cover import/export is available in Windows track tools.
- Local-file marks appear only on list/card covers. Their visibility, style and
  corner can be changed; player artwork and the mini-player keep no badges.
- Lyrics remember the selected provider/variant for each song. Automatic
  translation and romanization use Unison with Google fallback, with a persisted
  cache, independent on/off controls and translation-language selection.
- Lyric controls live only in the player track-tools menu. Original source,
  backing vocals, real syllable/word timing and timed source romanization are
  preserved. Generated readings are line text without invented word timings.
- Existing UI, cover-derived colors, phone navigation and local-data paths remain
  compatible. See the feature documents for details and verification limits.

## Windows portable package

Extract the ZIP and run vey.exe. Keep this README and the third-party notices
beside it. UI, fonts, Braccato and Kawarp assets are embedded. The installed
Microsoft Edge WebView2 and Microsoft Visual C++ x64 runtimes are required;
their installers are not bundled. This Windows executable is unsigned.

## Android package

The ARM64 APK uses package com.playervf.rust and minimum API 26. It is built
with the optimized release profile and debuggable=false. It uses the existing
development signing certificate for compatibility with previous APKs; this is
not a production/store signing certificate. Android uses the native Slint lyric
renderer. Native document pickers remain incomplete.

## Data and online features

Settings and the library use the existing PlayerVF-Rust storage identity
(%APPDATA%/PlayerVF-Rust on Windows, internal app storage on Android). Replacing
the app package does not replace that data. Online playback and lyric enrichment
need internet access; translation/romanization sends lyric lines to the named
external services on a cache miss.

## Build and evidence

PowerShell: scripts/build-release.ps1 for Windows; scripts/build-android.ps1
for Android. Both create versioned artifacts and SHA-256 files under dist/.
After both packages exist, scripts/clean-artifacts.ps1 -RequireAndroid removes
old local packages, generated build/debug output and root build logs. It keeps
the current release and checks every deletion stays inside this checkout.

Native/WASM compilation and lyric-menu rendering were checked during the latest
implementation. Live enrichment requests, Android device playback and Linux
runtime remain unverified. Automated tests were not run for this release task.
APK signature/alignment/manifest checks are packaging checks, not device tests.
