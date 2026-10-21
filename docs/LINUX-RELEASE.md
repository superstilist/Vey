# Vey 0.1.14 — Linux

The x64 release is built on Linux Mint 22.3 (Ubuntu 24.04 base), with glibc 2.39.
Use Mint 22.x / Ubuntu 24.04 or a compatible newer x86-64 desktop system. This
binary is not a package for older glibc distributions, ARM or macOS.

## Install the Debian package

From the download directory:

```sh
sudo apt install ./Vey-0.1.14-linux-x64.deb
```

Launch Vey from the desktop menu or run `vey`. Apt resolves the desktop, font,
audio and graphics dependencies. FFmpeg is optional for video artwork; without
it, Vey retains cover artwork. The desktop portal supplies file dialogs.

## Portable archive

Extract the tar.gz and run `./vey`. The native libraries listed in the Debian
package must be installed on the host. This is a native portable binary, not a
fully bundled AppImage. The UI, fonts and artwork assets are embedded.

Settings use `$XDG_DATA_HOME/playervf-rust`, or `~/.local/share/playervf-rust`.
Replacing the program does not replace the library or preferences.

Linux uses the native Slint lyric renderer, including original line/word cues,
translations and romanization. It does not provide the Windows WebView2 renderer.
The optional FFmpeg video adapter and online services require their respective
runtime dependencies. Build and dependency inspection do not establish successful
desktop startup, audio hardware playback or live-service behavior.

## Web backend

Extract the separate web release and run `vey --web-server` from the directory
containing its `web/` folder. Open http://127.0.0.1:4173. For internet hosting, use
a reverse proxy to this loopback endpoint and configure HTTPS yourself.

## Rebuild

Use Rust and a native C/C++ toolchain, plus ALSA, fontconfig, FreeType, xkbcommon
and D-Bus development packages. Run `bash scripts/build-linux.sh`. It produces
the architecture-specific tar.gz, .deb and SHA-256 files. Build on the oldest
supported distribution; compiling on a newer glibc does not support older hosts.
