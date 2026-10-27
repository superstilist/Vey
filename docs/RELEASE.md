# Vey 0.1.24

Native lyrics now select the fill path from the renderer actually created.
The software renderer uses a spatial mask instead of treating a text gradient
as one color across the whole word. GPU rendering keeps its single-run gradient.
Reduced motion retains the timed fill while disabling decorative motion.
Empty/completed words use one solid text run on both renderers.
GPU word fill is composed locally before wobble/line scaling, so the sweep
stays attached to the moving word. The temporary surface is used only while
a partially filled word is transformed; glyph overhang remains padded.

Native fill now uses the same spatial sweep for whole text and optional letter
waves, including variable-width glyphs and wrapped fragments. Empty/completed
coverage uses a solid brush. Glow follows that sweep's feather and fades out
within each actual cue instead of lingering over finished words. Hyphenated
syllables share word motion while retaining their independent fill timings.

Normal native lyrics now include subtle whole-word wobble. Timed syllables
share a motion interval and transform origin, retaining their original fill
clocks. Motion settles before the cue/fragment ends; short cues move less.
Letter waves work independently and return to one shaped run at rest. Reduced
motion disables both. Custom CSS supports `--blyrics-wobble-duration` and
`--vey-word-wobble-strength` (0–2), plus the existing wobble on/off switch.

Native line transitions now accelerate and settle softly. Retained focus
crossfades use 180ms to sharpen and 280ms to blur, with aligned paint and one
shared scale. Highlight fades ease out over 500ms for both line and word sync.
Reduced motion and seeks remain immediate; cue starts and fill deadlines stay exact.

Local Windows x64 build, 2026-10-27. Native lyrics now use a single shaped
fill pass, instantiate word views only near the viewport and use a soft
6px glow at 31% opacity by default. Optional custom motion, fonts and themes remain available.
Long hyphenated chants now wrap at hyphens; oversized tokens wrap at complete
Unicode graphemes. Timed syllables stay together when their word fits. Fill and
scroll follow wrapped cue fragments continuously without changing source timings.
The fill has a straight, stable edge and scrolling uses a soft-start/stop curve.
Normal mode waits for supplied cue starts, with no default camera lead or
future-line grouping. Sung focus never uses a future scroll cue; native stage
also waits for actual line starts and preserves genuine vocal overlaps.
Fill completes by each word's supplied end. Wrapped continuations remain
unfilled until the previous fragment completes. Optional anticipatory camera
settings remain available without advancing sung focus.
Backing-vocal phrases use one continuous fill edge, including when a cue wraps
onto another line; whitespace no longer restarts the fill on each word.

Native lyrics and the lyric editor use Rust/Slint. WebView2 remains necessary
for lyric API challenges and optional Windows video; returning to cover mode
releases the video WebView. The ZIP includes a SHA-256 checksum beside it.

The platform notes below describe the earlier 0.1.16 release.

## Earlier 0.1.16 release notes

This release fixes search focus during playback, refines compact mobile navigation,
and improves automatic lyrics and return-to-current-line scrolling. The open dark
interface includes restrained colors, larger mobile controls and rotating artwork
backgrounds. Appearance includes Calm and Expressive background presets.

Android packages passed packaging checks; physical-device acceptance remains
unverified. Browser and native layout evidence is recorded in PHONE-VERIFICATION.md.


## Downloads

| Platform | Package | Notes |
| --- | --- | --- |
| Windows | x64 ZIP | Requires WebView2 and Visual C++ runtimes |
| Linux | x64 tar.gz and .deb | Mint 22 / Ubuntu 24.04 or compatible glibc 2.39+ |
| Android | ARM64 and x86_64 APKs | API 26+ |
| Web | WASM ZIP | Online services require the Vey backend |

Every artifact includes a SHA-256 checksum. This release fixes mobile lyric
autoscroll pausing on taps, adds a swipe-down handle to close the mobile player,
and combines now-playing information and controls in one compact mobile card.
Search input remains editable while typing; press Enter to submit a query.

Other included features: YouTube Music search and playback; permanent local
downloads with embedded artwork; local-file marks on list and card covers;
remembered lyric providers and variants; automatic translation and romanization;
and synchronized lyric display. Translation and reading controls remain in the
player tools menu. Untimed generated lyrics do not claim word-level timing.

Windows, Linux, Android and Web packages are built from tag v0.1.16. Linux
packages target Mint 22.3 / Ubuntu 24.04 x64. Android APKs use the existing
Vey development signing key to permit compatible updates. Web assets require
HTTP(S); full online playback and lyric services need a same-origin Vey backend.

See [Linux instructions](https://github.com/superstilist/Vey/blob/main/docs/LINUX-RELEASE.md),
[Web instructions](https://github.com/superstilist/Vey/blob/main/docs/WEB-RELEASE.md),
and [Android build notes](https://github.com/superstilist/Vey/blob/main/docs/ANDROID-BUILD.md).

The audio routing change passed `cargo check --locked --lib` on Windows.
Physical output switching and audible playback still need hands-on verification.
Package signature/checksum checks do not substitute for device/runtime acceptance.
