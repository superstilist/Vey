# Vey 0.1.14

## Downloads

| Platform | Package | Notes |
| --- | --- | --- |
| Windows | x64 ZIP | Requires WebView2 and Visual C++ runtimes |
| Linux | x64 tar.gz and .deb | Mint 22 / Ubuntu 24.04 or compatible glibc 2.39+ |
| Android | ARM64 and x86_64 APKs | API 26+ |
| Web | WASM ZIP | Online services require the Vey backend |

Every artifact includes a SHA-256 checksum. This release adds debounced
search-as-you-type across local tracks and YouTube Music, combines local and
remote web search results, and improves recovery when the system audio output
fails while a track is playing.

Other included features: YouTube Music search and playback; permanent local
downloads with embedded artwork; local-file marks on list and card covers;
remembered lyric providers and variants; automatic translation and romanization;
and synchronized lyric display. Translation and reading controls remain in the
player tools menu. Untimed generated lyrics do not claim word-level timing.

Windows, Linux, Android and Web packages are built from tag v0.1.14. Linux
packages target Mint 22.3 / Ubuntu 24.04 x64. Android APKs use the existing
Vey development signing key to permit compatible updates. Web assets require
HTTP(S); full online playback and lyric services need a same-origin Vey backend.

See [Linux instructions](https://github.com/superstilist/Vey/blob/main/docs/LINUX-RELEASE.md),
[Web instructions](https://github.com/superstilist/Vey/blob/main/docs/WEB-RELEASE.md),
and [Android build notes](https://github.com/superstilist/Vey/blob/main/docs/ANDROID-BUILD.md).

The audio routing change passed `cargo check --locked --lib` on Windows.
Physical output switching and audible playback still need hands-on verification.
Package signature/checksum checks do not substitute for device/runtime acceptance.
