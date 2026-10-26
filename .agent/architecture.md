# Architecture and integration contracts

Source inspection snapshot: 2026-10-20. Start with `project.md` for the file map.

## Native runtime

Native audio follows the current OS default output. Its dedicated worker checks
the route/configuration every second and retries missing/failed outputs; Windows
also compares WASAPI endpoint identities. Reconnection restores the source at its
held position with volume, speed and requested play/pause state. Only configurations
of the system-selected device are tried. Missing output returns a load error without
terminating the worker. Hardware switching still requires runtime acceptance.

`src/main.rs` dispatches helper/server/auth modes or calls `playervf::run()`.
`src/lib.rs` includes compiled Slint modules and gates native, WASM and Android
modules. `build.rs` compiles `ui/main.slint`; changes to shared UI properties
must be reflected in native callbacks, WASM callbacks and screenshot fixtures.

`src/app.rs` connects the Slint window to the portable store/queue and native
services. Background work returns typed events; track/request identity prevents
stale results replacing newer selections, including a generation check for load
completion. Selecting a different track aborts the previous load future and its
ranged HTTP requests; repeated selection of the same loading track is ignored.
Windows WebView children wait for an available
Win32 parent handle before construction (`src/windows_webview.rs`). Playback
polling and result processing are separate. Documented budgets: roughly 100 ms
playback polling, 16 ms result drain, up to eight events and a 3 ms yield budget
between events. These are
scheduling choices, not a guarantee that one large event cannot block.

Local flow: configured sources/system Music -> native scan -> tracks/artwork
requests -> bounded decoding/cache -> UI model rows. Playback selection -> queue
and current track -> native audio or public YTM stream resolution -> playback
status -> controls and synchronized lyric host. Keep blocking decoding, tag I/O,
network and disk writes away from the UI thread.

## Browser runtime

`scripts/build-web.ps1` compiles the `playervf` WASM library and generates
`web/pkg/` bindings. `src/web.rs` runs the shared UI/controller; `web/bridge.js`
handles browser files, exports, challenge and lyric-host DOM integration.
`web/player-motion.js` shares the camera adapter with the widget host.

Selected files get `browser://` identities derived from name, size and last
modified time, mapped to session object URLs. Current import uses a filename
title, unknown artist, empty album/artwork and initial zero duration. Persistent
likes/playlists do not make the session object URLs survive a reload.

`src/server.rs` serves the application and same-origin public APIs. Its handlers
delegate search/browse/radio and cached ranged audio to `src/native.rs`, lyrics
and token exchange to `src/lyrics_service.rs`. Fixture server on port 4181 is
static only; application backend uses port 4173. Do not confuse those hosts.

## YouTube Music catalog decoder

`third-party/yt-rust/` is a relative native-only Cargo dependency copied from
the supplied local yt-rust project. `src/native.rs::parse_ytm_songs` merges its
normalized search metadata with the host's renderer ordering, availability,
cover and clip hints; suggestions retain host highlighting/history fields.
Native and same-origin browser search share this path. Public transport,
browse/radio and audio/video resolution remain Vey services. No session provider
or account-data integration is enabled. Provenance/local patches and checks:
`third-party/YT-RUST-UPSTREAM.md`, `docs/YT-RUST-INTEGRATION.md`.

Audio resolution prefers a public visionOS context with matching visitor/device
and User-Agent data, retaining WEB_REMIX/Android fallbacks. Range downloads validate
byte boundaries and lengths, cancel outstanding tasks on failure, and atomically
publish complete cache files. Live reproduction and recovery: `docs/PLAYBACK-RECOVERY.md`.

## Permanent files and artwork

`Store.downloads` maps public track keys to permanent local paths. Native download
completion registers a file source and local track; selections prefer that file
before stream resolution. Streaming cache files do not count as downloads.
Download marks and artwork update all mounted page models. Missing file mappings
are pruned on startup/scan. `src/media_library.rs` stages audio/tag/cover writes
before publication; cover replacement backs up removed front images. Native
track tools expose cover import/export; Android dialog stubs stay hidden.
`SongRow` shares row layout across page lists/home rows/player queue. Cover badges
cover every scanned/imported local track as well as Vey downloads. Native scans
populate `App.local_paths`; browser rows require a live session file URL. The
existing `downloaded` UI flag represents this broader local availability.
Badges use the shared `CoverBadges` global; `src/preferences.rs` applies/captures persisted
visibility, triangle/icon style and corner position for native and browser.
Only list/card templates opt into badges. Player media surfaces, the embedded
player queue and mini-player keep them disabled regardless of badge preferences.
See `docs/DOWNLOADS-ARTWORK.md` for supported formats and validation limits.

## Lyrics pipeline

`Store.lyrics_choices` preserves explicit per-song provider/variant selections
using source fingerprints and provider/format/sync fallback matching. Native and
browser controllers restore choices after cache/fetch; unavailable choices remain
saved. Known permanent YTM copies share their public track's choice key.
`Settings.lyrics_romanization`/`lyrics_translation` filter a display clone via
`braccato::visible_layers`, leaving cached/exported source and timings intact.
The compact lyric settings menu lives inside the player track-tools overlay,
not global Settings. lyrics_translation.rs enriches missing layers via Unison
with Google fallback, on a display clone; Store.lyrics_enrichment bounds the
persisted cache to 128 requests. Target language is independent or follows the
interface. Browser enrichment uses /api/lyrics/enrich; native uses Tokio.
Response keys guard the current selection. Romanization fallback infers the source
independently of Translation, groups missing lines by language and accepts only
Latin results without synthesizing timing; romanization keys use v2.
See docs/LYRICS-PREFERENCES.md.

Paste/provider/migration payload -> `src/lyrics.rs::parse_payload` -> canonical
alternatives and synchronization classification -> `src/braccato.rs` conversion
-> browser widget document or native Rust Braccato display rows. Preserve backing vocals,
translations, word boundaries, offsets and instrumental gaps across conversions.
Line-synced input has no reliable per-word timing to animate.
Word/syllable mode requires explicit part starts and matching source text.
Syllables must have ordered, distinct starts and preserve actual word boundaries;
lead, backing and romanization parts are checked independently. Verified syllable
alternatives take priority and use the compatible RichSync representation.
Timing completion fills only open line ends; it never distributes
line duration across words or syllables. Display adapters suppress untimed part/romanization
effects while preserving the parsed source. Widget documents carry `options.syncType`
so zero-origin lines and start-only word cues retain their detected mode. The
widget build adapts upstream view mode selection without editing vendored files;
`npm run test:sync` checks the resulting DOM and cue timing;
`npm run test:syllables` generates Rust documents and checks syllable grouping,
background timing and seek/reduced-motion behavior. See `docs/SYLLABLE-LYRICS.md`.
`parse_payload` unwraps complete backtick/tilde fences and JSON strings/envelopes
with a depth limit; format labels do not override content detection. Provider
responses normalize fences/quoted JSON before resolving named alternatives.
`recover_ttml` extracts complete root documents from surrounding chat/fence
debris, including bounded HTML-escaped transport recovery; skip JSON/SSE
envelopes until their contents are parsed. Do not decode already-valid XML
globally. TTML text normalizes numeric-space artifacts, preserving intentional
spaces between spans even across indentation newlines.
Keep explicit malformed XML rejection and Composer metadata extraction;
regressions live in `tests/lyrics_fixtures.rs` (`docs/LYRIC-INPUT.md`).

`src/braccato/` contains Rust parsing, TTML models, engine/settings, highlighting,
providers and fallback theme support. `tests/braccato_upstream.rs` plus fixture
JSON compare parser output against pinned upstream oracle data. Rust deliberately
preserves untimed QRC words where the oracle loses text; do not erase this fix to
make an upstream comparison identical. See `src/braccato/README.md` for others.

`third-party/braccato-rust/` is the copied local Rust-port snapshot. Native
Vey imports its Slint LyricsView through build.rs; src/native_lyrics.rs adapts
canonical display rows, clocks, source-index seeks and bounded glyph preparation.
ui/native-lyrics.slint binds the native component to the player surface.
The adapter suppresses upstream demo line-word staggering: line cues stay line
cues. Timed romanization, backing vocals and syllables retain source timestamps.
See docs/NATIVE-BRACCATO.md and third-party/BRACCATO-RUST-UPSTREAM.md.

`tools/braccato-widget/build.mjs` still bundles the preserved JavaScript snapshot
for web/braccato/. Browser-only src/lyrics_widget.rs bridges the iframe. Windows
lyrics no longer instantiate WebView2; video and challenge hosts remain separate.
JWTs never belong in rendering documents. The app selects Braccato only. Native
themes map supported CSS/RICS declarations to typed controls in src/themes.rs;
arbitrary CSS selectors/DOM behavior remain browser-only.

## Rendering and motion

Native lyric focus uses `src/native_lyric_focus.rs` and `native_lyric_masks.rs`:
one bounded worker rasterizes inactive-line masks, returns raw RGBA and rejects
stale document/layout generations. Slint creates/uploads images on its UI thread.
Only 14 nearby rows retain masks; paused focus reuses paint. Satoshi/Noto font
selection and cached variable-font metrics live in `src/native_lyric_fonts.rs`.
Native display-only NBSP keeps supplied word spaces; canonical timings/text stay
unchanged. See docs/NATIVE-BRACCATO.md for reference-style checks.
Word glows and glyph plans use a separate single-job worker in
`src/native_lyric_paint.rs`; layout generations and seek windows reject stale
results. Focus preparation runs independently so dense word cues cannot starve it.
The visible lyric clock ticks at 16 ms and slews small late media samples.
Normal mode uses zero scroll offset, zero lookahead and zero ending threshold.
CSS can opt into anticipatory camera/grouping; sung focus always uses actual
cue time. Native stage selection waits for real starts and retains real overlaps.
Committed groups do not scroll again at each cue. Active-band geometry handles
overlaps, pins tall wrapped sources and retargets immediately; manual/resume,
paused seeks and reduced motion remain exact. Sung activation keeps its clock.
The default glow is 6px at 31% opacity. Atlases crossfade two radii, or four
for broader custom glow. Normal fill paints base/highlight in one shaped text
run; letter waves and wobble are opt-in. Only viewport-adjacent row bodies
  instantiate word views. Focus crossfades aligned sharp ink/cached blur for
  180ms in and 280ms out, sharing one row scale/origin. Settled rows retain one
  paint body; reduced motion switches immediately.
  `FocusPaint` retains blend clocks outside model delegates and updates nearby
  `focus-mix` values only while changing, so ordinary row updates cannot reset
  a handoff. Seeks snap the blend together with the camera.
Multiword timed cues remain one shaping run until wrapping; normalized cue
offset/span carries the same fill edge across wrapped fragments without retiming.
Native fill is bounded by the supplied cue end; each wrapped fragment completes
before its continuation begins. The copied renderer exposes this as an opt-in
`finish-fill-at-cue-end` property so standalone upstream defaults remain available.
Wrapping keeps syllables of a word together, permits hyphen breaks and falls
back to whole grapheme clusters for oversized tokens. Visual follow uses each
fragment's measured fill-entry phase; canonical timestamps stay unchanged.
  The single-run fill has a gentle wave; scroll uses a soft-start/stop cubic
  curve. Highlight fade uses smoothstep without delaying cue fill. Reduced
  motion disables wave, animated scrolling and focus crossfade.
Explicit backing phrase intervals survive display conversion as single cues;
untimed phrases retain line sync and never acquire estimated word timings.
Glow is clipped at the actual fill edge. Appearance CSS and selected font family
persist in Settings. Desktop TTF/OTF imports are validated, stored under the state
directory's lyric-fonts/ and registered with Slint fontique and both raster workers.
Font revisions invalidate layout/glyph caches; missing glyphs fall back to Noto.
Font files stay local; settings do not contain paths.
Native lyric changes no longer serialize an unused browser document. Windows
video destroys its WebView on return to cover mode, suspends hidden IPC and
coalesces unchanged payloads/bounds. Authentication's challenge WebView remains
required; Android video uses a native TextureView, with WebView only for auth.

Slint directly composites native lyrics and the shared Rust ambient texture.
The browser retains bundled Kawarp through its iframe host.
`src/material.rs` derives tonal roles on artwork changes, not playback ticks.
Dominant hue families use circular averaging and artwork chroma; grayscale
covers keep dynamic themes neutral. Kawarp uses 0.28 opacity, warp 1, 15 blur
passes, 1.5 saturation, 0.02 dithering and no tint, with bright-cover dimming.
The browser texture and native fallback add slow clock-driven rotation with
overscan. Native bilinear sampling lives in `src/ambient_motion.rs`; pause and
reduced motion stop continuous work. Mobile `TouchStyle` gives shared actions
44 px targets and pressed feedback without extra persistent backgrounds.
Theme and texture transitions both use one-second smootherstep fades. The
native lyric component shares the Slint surface and artwork without a child HWND. See `docs/APPEARANCE.md`.
Desktop page surfaces stay mounted in a compass grid; one interruptible camera
moves content and lyric host together. Mobile Player is a retained-page overlay.
Reduced motion bypasses travel. Details: `docs/PAGE-MOTION.md`.
`src/navigation_fx.rs` captures two bounded transparent page textures per journey
for blur; `ui/navigation-streaks.slint` follows the same camera vector. The shared
stage radius drives Slint, DOM and Windows clipping. No snapshot/blur runs per frame.

Avoid duplicate lyric rebuilds, repeated theme compilation, repeated geometry
styles and hidden frame work. Clock interpolation must apply seeks, pause and
rate changes promptly while smoothing minor sampling corrections. Preserve
bounded ambient requests and retry after a lost response.

## Data and authentication

Appearance: `Settings.color_mode` stores `dynamic` (default), `static` or
`background`; `color_hue` stores the fixed hue (default 85). Native and WASM
callbacks persist both. `src/material.rs` caches the artwork hue and retargets
17 token roles from their displayed values over one second on a finite 32 ms
timer. Background mode changes only neutral surface roles; Static also fades
the ambient texture to zero. Reduced motion applies targets immediately.
`colors-changing` pauses hidden painting/warming without destroying pages.
Camera easing must match Slint, `web/player-motion.js`, and Rust's inverse in
`src/lyrics_widget.rs`: cubic-bezier(0.45, 0, 0.55, 1), 240–520 ms.
See `docs/APPEARANCE.md` for checks and settings UI.

`src/services.rs` defines `Track`, `Source`, `Playlist`, `History`, `Settings`,
`CachedLyrics`, `Store` and `Queue`. Store version is currently 1. Serialized
track/history fields use camelCase; do not change formats casually.
Store includes tracks/sources, likes, playlists, history, offsets, migrated legacy
preferences, lyric cache and settings. Missing fields have compatible defaults.
Since 0.1.6, Settings.rate is an internal normal-speed compatibility field;
serde ignores old saved rates and omits the preference from new backups.
`playlist-destinations` contains real writable playlists; native navigation's
Liked/history rows must not be used as add-to-playlist destinations.

`src/storage.rs`: `PLAYERVF_DATA_DIR` override; otherwise APPDATA/PlayerVF-Rust,
or XDG_DATA_HOME/playervf-rust (HOME/.local/share fallback). Android entry sets
an internal-data override. Unreadable/unsupported state is copied before recovery.
Saves write `state.pending.json`, then rename to `state.json`. One background
writer retains the latest pending snapshot; normal shutdown joins the writer
and flushes pending writes.
UI save requests are coalesced; preserve error reporting and shutdown behavior.

Migration uses the allowlist in `src/services.rs`; backup/import must not include
cookies, JWTs or challenge credentials. `src/local_sync.rs` stores separate tag
backups before reviewed metadata writes; restore must remain available.

Lyrics auth is separate from Google sign-in. `src/lyrics_service.rs` implements
token cache, exchange and provider requests; `src/challenge.rs` dispatches the
Windows helper or Android WebView bridge. Android shares JWT renewal, 401 retry
and response normalization; it does not persist tokens in Java. `/verify-turnstile` exchange accepts a JWT/token response. `/v2/lyrics`
form fields are videoId, song, artist, album, duration, alwaysFetchMetadata=false,
token. Preserve expected Origin/Referer and cookie-aware transport. Concurrent
refresh is shared; documented failure cooldown is 30 seconds. Never print tokens.

## Build/packaging boundaries

Package/binary: `vey`; Rust library/WASM artifact: `playervf`. Android package:
`com.playervf.rust`, API 26 minimum, target 35, ARM64. Native activity lives in
`src/lib.rs`. `src/android.rs` requests audio permission on the Java main thread
and queries MediaStore external audio on a worker. API 33+ uses READ_MEDIA_AUDIO;
older devices use READ_EXTERNAL_STORAGE. Grant resumes scanning; provider errors
retain the existing catalog. `src/dialogs.rs` still has file/save picker stubs;
MediaStore discovery does not implement SAF document import/export. Native safe
insets feed shared layout; dialogs also use keyboard bounds and scrolling.
Windows portable package needs installed WebView2 and Visual C++ x64 runtimes.
Release scripts generate `dist/` artifacts and checksums; inspect their current
parameters and signing docs before packaging. Never store signing secrets here.

## Player refinement interfaces

`src/preferences.rs` applies/captures language and Kawarp options for both
controllers. `locales/catalog.tsv` feeds Slint, controller messages and the web
widget; source text is never translated. `ui/motion.slint` owns semantic motion
durations. Japanese fonts are bundled under OFL.

`src/video.rs` resolves clips and sends muted visual state from the song clock
to browser HTML video, Windows WebView2, Android TextureView/MediaPlayer or
Linux FFmpeg frames. Empty/failing sources reveal the art slot. Android Java
source is compiled to embedded DEX by `build.rs` (JDK and Android SDK required).
See `docs/PLAYER-REFINEMENT.md` for contracts, dependencies and validation limits.

Native lyric requests start on track selection independently of audio loading.
`lyrics_service::fetch_with_progress` publishes normalized alternatives as each
provider completes, then caches the final quality order. Track generations and
one cancellable lyric task suppress stale/duplicate requests; explicit imports
cancel pending requests. Shared auth renewal outlives track cancellation. Cache-write failures must not hide fetched lyrics.

Windows WebView hosts install an idempotent shell pointer/focus bridge on the
Slint HWND. Pointer presses on the shell restore native keyboard focus; hiding
a focused child releases it. Do not move focus on playback timers. Silent auth
hosts must be non-focusable before attachment (Android) and never request focus
(Windows). Search input stays mounted with stable clear-action geometry.

## Vey account sync

`src/account.rs` installs a shared native/WASM controller. Its separate opaque
session is excluded from Store exports; Basic sync allowlists video/cloud references,
preferences and history. Revision conflicts pause automatic sync. Private audio
uploads require server-checked developer/tester Premium entitlements; downloads
are account-scoped native files or temporary browser Blobs. See `docs/ACCOUNTS.md`.
The Cloudflare D1/R2 implementation lives in the separate `blueberry_melody` repo.

Vey uses its own RFC 8628 OAuth device grant via the system browser. The default
public API is https://blueberry-melody.superstilist.workers.dev; manual server/email
entry remains available. Browser popup preparation is synchronous with the click;
consent is explicit, token polling obeys the server interval, and cancellation
invalidates the local pending flow. No client secret is shipped.
