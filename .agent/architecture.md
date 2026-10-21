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
-> upstream widget document or native fallback rows. Preserve backing vocals,
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

`tools/braccato-widget/build.mjs` bundles vendored upstream runtime exports and
the integration into `web/braccato/`. Generated assets are checked in and native
Windows builds embed them. Ordinary Rust builds need no Node dependency install.
Windows host: `src/lyrics_widget.rs`; browser host: `web/bridge.js`. JWTs never
belong in rendering documents. Preserve origin/message checks and host disposal.

The app selects Braccato only; legacy theme selectors/custom CSS UI were removed.
RICS and theme-processing code still exists in upstream/fallback APIs. Older
docs describing multiple selectable themes do not establish current UI support.
Windows/web show loading/retry on host failure; do not assume automatic fallback
merely because an older integration note describes that behavior.

## Rendering and motion

Slint draws controls/surfaces. Bundled Kawarp produces a small decorative texture
through the web host; native lyric background paints the matching surface crop.
`src/material.rs` derives tonal roles on artwork changes, not playback ticks.
Dominant hue families use circular averaging and artwork chroma; grayscale
covers keep dynamic themes neutral. Kawarp uses 0.28 opacity, warp 1, 15 blur
passes, 1.5 saturation, 0.02 dithering and no tint, with bright-cover dimming.
Theme and texture transitions both use one-second smootherstep fades. Keep
the native backdrop opacity aligned with Slint; see `docs/APPEARANCE.md`.
Desktop page surfaces stay mounted in a compass grid; one interruptible camera
moves content and lyric host together. Mobile Player is a retained-page overlay.
Reduced motion bypasses travel. Details: `docs/PAGE-MOTION.md`.

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
