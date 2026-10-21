# Vey 0.1.12 — Windows, Linux, Android and Web

## Downloads

| Platform | Architecture / package | Requirement |
| --- | --- | --- |
| Windows | x64 ZIP | Installed WebView2 and Visual C++ runtimes |
| Linux | x64 tar.gz and .deb | Mint 22 / Ubuntu 24.04 or compatible newer glibc 2.39 |
| Android | ARM64 and x86_64 APKs | API 26+, existing development signing certificate |
| Web | Optimized WASM ZIP | HTTP(S); full online features need the same-origin Vey backend |

Every download has a separate SHA-256 file. Existing Windows/ARM64 artifacts
are unchanged; Linux, Web and Android x86_64 assets extend this release.
The application source remains tag v0.1.12; platform packaging helpers are
maintained on main. Native macOS/iOS and ARMv7 are not published in this release.

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

The ARM64 and x86_64 APKs use package com.playervf.rust and minimum API 26. It is built
with the optimized release profile and debuggable=false. It uses the existing
development signing certificate for compatibility with previous APKs; this is
not a production/store signing certificate. Android uses the native Slint lyric
renderer. Native document pickers remain incomplete.

## Linux and Web packages

Linux packages are built natively on Linux Mint 22.3 x86_64 over SSH, with build
headers extracted into an isolated user cache (no sudo or system installation).
Use apt install with the .deb to resolve runtime dependencies, or extract the
tar.gz and run the binary with those dependencies installed.
See [Linux instructions](https://github.com/superstilist/Vey/blob/main/docs/LINUX-RELEASE.md).

The Web ZIP includes generated runtime assets, licenses and launch helpers.
Use the matching native Vey binary as the backend for YouTube Music and lyric
services; static hosting supports local browser files/manual lyric imports.
See [Web instructions](https://github.com/superstilist/Vey/blob/main/docs/WEB-RELEASE.md). No hosted website is deployed by this release.

## Data and online features

Settings and the library use the existing PlayerVF-Rust storage identity
(%APPDATA%/PlayerVF-Rust on Windows, internal app storage on Android). Replacing
the app package does not replace that data. Online playback and lyric enrichment
need internet access; translation/romanization sends lyric lines to the named
external services on a cache miss.

## Build and evidence

PowerShell: scripts/build-release.ps1 for Windows; scripts/build-android.ps1
for Android. Both create versioned artifacts and SHA-256 files under dist/.
Linux: bash scripts/build-linux.sh. Web: scripts/package-web.ps1.
Android x86_64: scripts/build-android.ps1 -Architecture x64.
After packages exist, scripts/clean-artifacts.ps1 -RequireAndroid removes
old local packages, generated build/debug output and root build logs. It keeps
all current-version platform artifacts and checks every deletion stays inside this checkout.

Native/WASM compilation and lyric-menu rendering were checked during the latest
implementation. Live enrichment requests, Android device playback and Linux
runtime remain unverified. Automated tests were not run for this release task.
APK signature/alignment/manifest checks are packaging checks, not device tests.
