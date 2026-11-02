# Vey 0.1.45

## Home, recommendations and artwork

The Home page has a clearer layout, more compact artwork shelves and improved
loading and empty states. Recommendations use listening history, likes and
playlists; recent results are cached and refreshed approximately hourly while
the app is running. Artwork uses bounded caches and a public thumbnail fallback.

The newer Blueberry Melody worker is required for cross-device recommendation
snapshots and automatic catalog imports. It has not been deployed as part of
this release. Existing account library sync remains compatible with older workers.

Platform packages are built separately. Compilation and packaging do not establish
audio playback or UI acceptance on every target. Android packages use the existing
development signing key unless a release key is configured; macOS packages are
ad hoc signed and not notarized.

# Vey 0.1.42

## Playlist layout polish, 2026-11-01

Playlist pages now pair a multi-cover artwork panel and playback actions with
the track list in a responsive two-column layout. Sidebar playlists show their
cover and offer quick play. Covers refresh when asynchronous artwork loads.

This is a local Windows x64 package. Other platform runtime behavior is not
claimed by this build.

# Vey 0.1.41

## Playlists and profile sharing, 2026-11-01

Playlist rows now show artwork, author and last-modified time. Playlist pages
include a direct Play action and an opt-in control to publish the playlist to
the linked Blueberry Melody profile. The profile can list public playlists and
add one to the user's Vey cloud library; listening history appears only on the
owner's profile. The matching Blueberry Melody worker and site update must be
deployed separately before profile sharing is available online.

This is a local Windows x64 package. Other platform runtime behavior is not
claimed by this build.

# Vey 0.1.40

## Cleaner lyric view, 2026-11-01

Hides songwriter credits and the manual lyric time-offset badge while lyric
tools are closed. Opening track tools reveals those details; the offset remains
editable there.

This is a local Windows x64 package. Other platform runtime behavior is not
claimed by this build.

# Vey 0.1.39

## Configurable lyric spacing, 2026-11-01

Adds a saved setting for the space between a lyric and its romanization or
translation. The default adds 65% of the lyric font size; users can adjust it
from Settings → Lyrics, from zero to 150%. Native and browser lyric renderers
use the same setting and update their layout and scroll geometry together.

This is a local Windows x64 package. Other platform runtime behavior is not
claimed by this build.

# Vey 0.1.38

## System media controls, 2026-11-01

Publishes current track metadata and playback position to Windows System Media
Transport Controls, Linux MPRIS, macOS Now Playing, Android MediaSession and the
browser Media Session API. System play/pause, previous/next and seek actions are
forwarded to Vey's existing player. Desktop metadata includes the track artwork
URL where the platform supports it.

Native romanization and translation rows now leave more space below the lead
lyrics while preserving aligned scroll offsets.

This is a local Windows x64 package. Android device behavior and Linux/macOS
runtime behavior require platform runtime acceptance; compilation is not device
acceptance.

# Vey 0.1.37

## Windows build, 2026-11-01

Adds breathing room between native lead lyrics and romanization/translation,
with matching scroll geometry. The Collection Home layout no longer shows an
empty duplicate Listen again heading. When public recommendations are empty,
the page explains the state and links to Explore.

Native playback continues through Rodio/CPAL to the operating-system default
output; browser playback uses HTML Audio. Windows, WASM and Android cross-target
compilation passed. Device playback and Linux/macOS runtime acceptance were not
checked. This local Windows x64 package uses the current source tree; it is not
a published tag or a release for other platforms.

# Vey 0.1.36

## Windows build, 2026-11-01

Improves repeat audio loading, bounds and prioritizes artwork work, and reduces
native ambient updates during navigation. First-time online audio still waits
for a complete download. Details: PERFORMANCE.md in the source tree.

Redesigns the desktop player around larger artwork and synchronized lyrics.
The expanded view hides navigation while keeping the playback dock, and focus
mode places playback controls beneath the cover. The mobile player retains its
sheet layout.

Includes blended Home recommendations from the current song, recent listening and
liked tracks. Vey compares up to three public YouTube Music radio batches,
prioritizes songs shared between them, filters the 12 most recent plays and
liked songs, and limits repeated artists on the first screen. Local songs are
matched through title and artist; generic public picks are the fallback.

Also includes playlist import from public YouTube Music links, independent Vey /
YouTube Music / Spotify connections, six Home layouts with previews/random
selection, and debounced/cached search.

YouTube Music Connect opens Google's normal v3 account chooser in installed
Chrome or Edge, without a Google Client ID/secret. Its dedicated browser tab
must remain open for automatic read-only playlist pulls. Spotify imports use
configured OAuth and match metadata to Vey's playable catalog. Vey cloud login
is independent. Setup and platform limits: [MUSIC-ACCOUNTS.md](MUSIC-ACCOUNTS.md)
beside the executable. Settings also includes account profile editing, password
changes and avatar upload; account details are in [ACCOUNTS.md](ACCOUNTS.md).
Live private-account synchronization has not been accepted on a real signed-in
account.

This local Windows x64 package is built from the current working tree, retaining
the source version 0.1.36. It is not a new published tag or a release for other
platforms. WebView2 and Visual C++ x64 runtimes are required by the Windows app;
Chrome/Edge is needed only for the YouTube browser connection.

## Whole-player themes (2026-10-29)

Themes now cover surfaces, transport, navigation, queue, artwork corners, font
fallback and background strength together with lyrics. The editor keeps a
separate draft and offers lyrics-only styling. Custom CSS/RICS can set bounded
--vey-* appearance roles.

Adds Graphite, Solar Dusk, Cobalt and Paper Night: 20 offline presets in total.
Material 3 uses colors evaluated from its pinned original CSS. Apple Music uses
its source accent and artwork shape. These are native appearance ports;
YouTube DOM layouts, remote fonts and custom extension shaders do not run in
Slint.

Browser camera motion uses bundled GSAP 3.15.0/CustomEase, retaining the host's
clock/path and reduced-motion behavior. Native motion remains Rust/Slint.
Native CSS colors now include HSL/HSLA, Display-P3 and OKLCH.

Windows x64 portable build, 2026-10-29. Scope and checks: docs/LYRIC-THEMES.md.

## Appearance panel (0.1.32)

Reworks Lyrics appearance into a scrollable settings panel with a fixed action
bar, font/size/weight/spacing controls, inactive blur and individual effects.
Its independent Braccato sample previews timed fill, prepared glow and letter
motion without changing the current song. Cancel discards the draft; Apply
validates and saves it; Reset restores Braccato and clears the font override.
Desktop supports Escape to cancel. Narrow layouts keep the action bar visible.

Fixes inactive CSS selectors changing active ink, resolves CSS variable aliases
and fallbacks, accepts `!important` and short alpha hex colors, and prevents
conditional/secondary browser rules from overriding native main typography.
Unsupported native rules are reported. Invalid native RICS cannot replace the
saved theme; browser-only RICS directives retain their upstream compiler path.

Windows x64 portable build, 2026-10-29. Detailed scope and checks:
docs/LYRIC-THEMES.md.

## Offline lyric presets (0.1.31)

Adds 13 offline lyric presets, with 16 styles available in Lyrics appearance.
Original Vey themes: Aurora, Ember, Ocean, Rose Quartz, Neon Pulse and Nocturne.
Community adaptations: Minimal, Opacity Karaoke, Apple Music, Lucid, Spotlight,
Sustain and Material 3, referenced from https://github.com/better-lyrics/themes.
These are Vey lyric adaptations; external player skins, shaders and fonts are
not installed.

Choose a preset, inspect its sample/creator credit and press Apply. Your choice
is saved across restarts. A font override remains available, and Custom CSS /
RICS opens the existing editor. Source lyric times and scroll policy are
preserved. Reduced motion overrides preset letter waves and glow.

Windows x64 portable build, 2026-10-28. Native desktop/mobile and browser
fixtures cover all presets, saved/custom CSS, font/color output and reduced
motion. Detailed scope: docs/LYRIC-THEMES.md.

## Letter and fill quality (0.1.30)

Refines native lyric letter animation and fill. Font shaping now preserves
kerning, ligatures and combining accents in animated clusters. Scripts that
cannot be split safely retain their whole shaped text. Short-cue letter waves
begin from rest and settle within the existing cue.

Kerned letters retain their paint bounds. Software fill samples the feather at
physical-pixel resolution, while GPU gradients use the same translucent color
samples. Faint custom inactive text keeps its requested alpha. Source lyric
timestamps and word/syllable classification are preserved.

Windows x64 portable build, 2026-10-28. Verification details and platform limits
are recorded in docs/NATIVE-BRACCATO.md.

## Audio polling and lyric motion (0.1.29)

Fixes native lyric motion being cancelled by delayed audio-status samples.
Audio positions now account for the age of their worker-thread sample. The
presentation clock corrects late, forward-progressing samples without moving
backwards. Only confirmed transport seeks, page entry and explicit presentation
changes reset camera motion; ordinary status corrections keep the animation.

Reproduced using cached Smooth Criminal word-timed cues around 34.6–35.4 seconds
and the native worker/UI polling cadence. The earlier clock could rewind by
35ms during the second-to-third line transition, cancelling its 750ms move and
jumping about 191px to the destination. See docs/NATIVE-BRACCATO.md for measured
regression results and remaining acceptance limits.

Windows x64 portable build, 2026-10-28.

## Prepared transitions (0.1.28)

Native lyric scroll destinations are now calculated after layout and reused at
cue boundaries. Upcoming rows begin preparation one second before their cue,
with only two upcoming rows selected to bound rendering work. Settled camera
movements accelerate and decelerate smoothly; interrupted movements preserve
their velocity. Actual seeks and reduced motion retain exact positioning.

Wrapped line-only lyrics start at their first visual row instead of jumping
to the last fragment. Trailing spaces share their word's shaping run, reducing
empty rendering delegates without inventing word timing or changing text.
Source timestamps, translations, backing vocals and timed word fill remain
unchanged. See docs/NATIVE-BRACCATO.md for verification scope.

Windows x64 portable build, 2026-10-28.

## Earlier camera work (0.1.27)

This build fixes the visible pause when a lyric transition begins or when
another cue arrives during an unfinished scroll. A settled native camera starts
moving on the next frame instead of spending the first part of its ease-in at
rest. A moving camera retains its current position and velocity when it receives
a new target, then settles without reversing or overshooting. Very close targets
shorten the settling time instead of abruptly losing speed. Seeks and reduced
motion snap exactly; manual scrolling rebases the camera. Source cue timestamps,
word fill, glow and wobble are preserved.

Windows x64 portable build. See docs/NATIVE-BRACCATO.md for the measured
line-transition regression and platform verification limits.

## Earlier rendering work (0.1.26)

This build targets pauses when opening the player and handing off lyric lines.
Native lyrics retain only a viewport band of row delegates with exact omitted
scroll height, keep nearby text ready through focus changes and cache repeated
font measurements. Focus animation updates separate float cells instead of
rewriting rows containing word/font models. The live player drops its permanent
full-page render cache outside navigation. Native navigation blur preparation runs on a bounded worker
while camera travel proceeds, with stale-result rejection on retargets.

The earlier 0.1.25 optimization below reduced steady word-model reads; that
measurement did not cover cold entry or dense line transitions.

On the local Windows release stress fixture, median p95 event-loop intervals
fell from about 61ms to 31ms. First-entry maximum intervals were 64–81ms across
three final runs, compared with 803ms before. These are workload-specific
measurements; source cue timing and lyric effects remain unchanged.

Native lyric updates now revisit word models only at cue and paint-release
deadlines, with retained preparation windows instead of per-frame word reads.
Glow color is prepared once in the worker and shared by its feather/radius
layers, avoiding separate GPU tint textures. Source timing, spatial fill,
word wobble and reduced-motion behavior are preserved.

On the same 300-line/2400-word release fixture, median word-model reads fell
from 788 to 40 on GPU and 806 to 32 on software across three runs each.
The event-loop interval remained near 16ms; this is reduced model/render work,
not a measured global FPS increase. Windows GPU/software checks are separate
from browser and physical-device acceptance.

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
