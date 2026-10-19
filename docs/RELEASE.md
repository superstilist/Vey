# Vey 0.1.0 — Windows x64

Extract this archive and run `vey.exe`. Keep the bundled notices with it.
The UI, fonts, Braccato and Kawarp assets are embedded in the executable.
The app uses the installed Microsoft Edge WebView2 and Microsoft Visual C++
x64 runtimes. This package does not bundle their installers.

This build includes eight-direction camera navigation, grouped player and
lyrics movement, destination pre-rendering and cached artwork morphs. Camera
duration follows travel distance, so shorter interrupted moves settle sooner.
The background and playback controls stay steady throughout navigation.
Reduced motion is available in player settings.

Vey uses the supplied plate logo and lime accent. Settings saves run on a
background writer and rapid changes are combined into one pending snapshot.
Background results yield between batches so they cannot monopolize the UI
loop. The final pending save is flushed when the app closes normally.

Local settings and library data live in `%APPDATA%\PlayerVF-Rust`. Extracting
or replacing this package does not replace that data. Online playback, sign-in
and lyric services require an internet connection.

This is a portable, unsigned build. Native smoke checks use fixture tracks;
they do not establish parity for every live online service or machine.

To rebuild from the source checkout, run `./scripts/build-release.ps1` in
PowerShell. The script builds the widget and optimized Rust executable, then
creates the archive and SHA-256 checksum under `dist/`.
