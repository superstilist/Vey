# Android APK builds

Build an optimized ARM64 (`arm64-v8a`) APK with:

```powershell
cargo install cargo-apk --version 0.10.0 --locked
./scripts/build-android.ps1
```

Set `ANDROID_HOME` to an installed Android SDK and `JAVA_HOME` to JDK 17 or
newer. Install SDK platform 35, build-tools and an NDK. The script uses
`ANDROID_NDK_HOME`, or the newest installed NDK under the SDK. This build used
NDK 28.2.13676358 and build-tools 37.0.0. The Rust target is
`aarch64-linux-android` (`rustup target add aarch64-linux-android`).

The package keeps `com.playervf.rust` for data compatibility and displays Vey
with the supplied transparent plate logo. Minimum API is 26 and target API 35.
The native application library remains `playervf`, matching the activity entry.
Permissions include INTERNET, READ_MEDIA_AUDIO (requested at runtime on API 33+)
and READ_EXTERNAL_STORAGE capped at API 32 for older devices.

Outputs are `dist/Vey-VERSION-android-arm64.apk` and its SHA-256 file. The
script checks the APK signature, alignment and manifest before copying it.
`-SkipBuild` rechecks and packages an existing signed APK.

## Signing

For local installs, the script can use an existing
`$env:USERPROFILE/.android/debug.keystore` with the standard development
password. Compilation still uses the optimized release profile, and the APK
manifest has `debuggable=false`. The published 0.1.4 APK uses this development
certificate; it is not signed with a private production/store certificate.

To use another existing key, set `CARGO_APK_RELEASE_KEYSTORE` and
`CARGO_APK_RELEASE_KEYSTORE_PASSWORD` before running the script. Signing keys
and passwords are not bundled into the APK or committed to the repository.
Keep the same signing key for compatible in-place updates.

## Runtime scope

No Android device was connected during this build, so package checks do not
establish successful startup, audio playback or online-service behavior on a
phone. Android uses the Rust/Slint lyric fallback, rather than the Windows
WebView2/Braccato host. Native Android file/folder and save/open dialogs are
currently stubs. Since 0.1.5, adding music starts permission-aware device audio
discovery through MediaStore; manual path entry remains available. Import/export
document picking is incomplete. See ANDROID-FIXES.md and PARITY.md for checks
and the remaining platform differences.
