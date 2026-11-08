# Architecture and integration contracts

Source inspection snapshot: 2026-11-01 (`Cargo.toml` 0.1.40). Start with
`project.md` for the file map. Contracts below describe intent in the current
tree; they are not proof that a target was built or run today.

## Home discovery

`src/discovery.rs` composes shared native/WASM Home shelves from local history,
playlists, the private recommendation snapshot, Blueberry candidates and a
bounded YouTube Music catalog (`Store.home_catalog`, local cache only). A
connected desktop YouTube Music session contributes at most one additional
shelf per media kind, with five cards per shelf; its normalized feed remains in memory and is never sent
to the account catalog. Home, search and collection browse stay view-only; played
and explicitly saved tracks follow the normal library sync path.
Blueberry's library sync excludes unplayed recommendation snapshots from shared
catalog submissions. `ui/home-feed.slint` retains album, playlist and artist
browse targets and real video IDs with 16:9 cards.
Actions carry both shelf and item identity so repeated cards select the correct
queue and video mode. `/api/home` uses the native catalog service for Web;
failed refreshes preserve the cache and artwork uses existing bounded workers.
See `docs/HOME-DISCOVERY.md` for limits, navigation and verification commands.

## Native runtime

Native audio uses Rodio/CPAL's target-native host (WASAPI on Windows, ALSA on
Linux, CoreAudio on macOS and AAudio on Android) and follows the current OS
default output. WASM uses the browser's HTMLAudioElement, whose default route is
managed by the browser/OS. Its dedicated worker checks
the route/configuration every second and retries missing/failed outputs; Windows
also compares WASAPI endpoint identities. Reconnection restores the source at its
held position with volume, speed and requested play/pause state. Only configurations
of the system-selected device are tried. Missing output returns a load error without
terminating the worker. Hardware switching still requires runtime acceptance.

System media controls are a separate metadata/transport layer. Desktop
`src/media_session.rs` publishes title, artist, artwork URL, duration, position
and play state through SMTC on Windows, MPRIS on Linux and Now Playing on macOS;
its event channel returns OS transport commands to `src/app.rs`. Android uses
`MediaSession` in `android/java/app/vey/VeyBridge.java`, polled through
`src/android.rs`. Browser playback publishes `navigator.mediaSession` metadata
and actions in `web/bridge.js`; HTMLAudioElement remains the audio output path.

Page track models use `src/track_model.rs`: path-indexed cover/like updates,
deduplicated mounted aliases and retained delegates for unchanged rows/small
structural edits. `TrackModel::as_any` exposes the concrete model through
Slint's `ModelRc` so mounted rows can receive asynchronous updates. Bulk catalog merges and playlist resolution use the indexed
Store methods in `src/services.rs`. Browser saves coalesce for 200ms and flush
on pagehide/hidden visibility; serialization remains on the browser thread.
Native/Web refresh library collection cards only when the collection category
is visible. Native projects collections on a bounded/coalesced worker, rejects
stale generations, and applies rows and textures on the UI thread. Web still
projects collections on the browser thread. Navigating back rebuilds them from
the current store.
Native cover-completion events patch matching mounted rows and existing sidebar,
playlist-destination and queue models in place. They must not rebuild the full
playlist/sidebar or queue model per image. Download progress also updates the
changed track path only. Native page rows are built when a route is opened;
hidden pages refresh from the current store on entry. System media position is
published at most once per second, with playback-state changes and seeks sent
immediately.
Library projection uses scanned local-track membership instead of filesystem
checks on the UI thread. Missing download mappings are pruned by the scan worker;
until that scan finishes, a removed local file can still appear in the library.
Playback and download selection check local paths on a blocking worker and
discard stale download progress/completion by generation.
Measurement scope and reproduction: `docs/PERFORMANCE.md`.

`src/main.rs` dispatches helper/server/auth modes or calls `playervf::run()`.
`src/lib.rs` re-exports the independently compiled `crates/vey-ui` Slint types
and gates native, WASM and Android modules. `crates/vey-ui/build.rs` compiles
`ui/main.slint`; root `build.rs` builds the Android bridge. Slint's generated
embedded font resources are shared with native lyric shaping, with a build-time
format check. Changes to shared UI properties
must be reflected in native callbacks, WASM callbacks and screenshot fixtures.
The `braccato` library path in `build.rs` points at
`third-party/braccato-rust/crates/slint/ui`, so a UI file added there is part of
the native compile graph.

`src/app.rs` connects the Slint window to the portable store/queue and native
services. Background work returns typed events; track/request identity prevents
stale results replacing newer selections, including a generation check for load
completion. Selecting a different track aborts the previous load future and its
ranged HTTP requests; repeated selection of the same loading track is ignored.
Native screens are Slint surfaces, with decoded video frames in the art slot. Playback
polling and result processing are separate. Documented budgets: roughly 100 ms
playback polling, 16 ms result drain, up to eight events and a 3 ms yield budget
between events. These are
scheduling choices, not a guarantee that one large event cannot block.

Local flow: configured sources/system Music -> native scan -> tracks/artwork
requests -> bounded decoding/cache -> UI model rows. Playback selection -> queue
and current track -> native audio or public YTM stream resolution -> playback
status -> controls and synchronized lyric host. Keep blocking decoding, tag I/O,
network and disk writes away from the UI thread.
Artwork uses four background slots plus one reserved current-track slot. A
refresh retains queued covers belonging to the active page and drops stale
offscreen work; transient failures retry once. Native cover cache writes happen
after display; repaired cache files remain separate from user-selected covers.
When a local file has no embedded/folder cover or video ID, the artwork worker
can resolve a YTM cover with exact normalized title/artist matching, an 8-second
lookup cap, and duration-based version selection; successful images update
mounted rows and cache by local identity.

## Library and playback management

Native and WASM controllers share session-only library query, filter, sort and
selection state. Store v1 gains optional `libraryExcluded` with a serde default;
do not bump the format version for it. Bulk actions and relinking use canonical
track identities; relinking rewrites saved likes, history, playlist references,
lyrics state and local download paths without deleting audio files. Source scans
use cancellation tokens and generation checks so a removed source cannot publish
an obsolete scan result.

`LibraryViewState` also holds the session-only Songs/Albums/Artists/Playlists/
Downloads category. Album and artist cards are projections of `Store.library_tracks()`
using normalized non-empty metadata; incomplete tags stay in Songs, and playlist
cards include empty saved playlists. Recently played sorting reads `Store.history`.
Local collection details reuse Explore, while the Now Playing queue tab gets its
own model from `Queue` so it stays aligned even when another page is underneath.

Browser file URLs remain page-memory-only, so local files need to be reselected
after reload. The bridge reads available tags/covers from MP3 ID3, FLAC Vorbis
comments/pictures, Ogg/Opus comments and MP4/M4A metadata; filename remains the
fallback. Browser downloads stream to a temporary Blob and use AbortController;
native downloads use ranged audio progress and an abort handle. Neither changes
the Store v1 download contract for browser-picked source files.
Browser artwork uses a bounded, seven-day CacheStorage cache and asynchronous
`createImageBitmap` decoding downscaled to 768px before Slint upload. Initial
offscreen page warming waits three seconds after startup. Validated audio cache
metadata allows repeat opens without stream resolution; ranges fill one bounded
buffer as they finish. Browser artwork fetches have a timeout and byte limit.
During native page and sheet movement, ambient presentation is limited to
100 ms intervals while its clock continues; `VEY_LEGACY_AMBIENT_TRAVEL=1`
restores the previous cadence for diagnostics.

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

The Worker admits standard YouTube Music songs only when the account's saved
library or listening history contains their video ID. A private search or
recommendation result cannot publish catalog data. The background native match
for local files requires the same title, artist and duration within five seconds
and a resolvable audio song; ambiguous files stay local. The automatic shared
import uses the YouTube Music identity and does not query MusicBrainz.

Client sync limits each pass to 20 changed songs and sends the best cached
provider per real sync type. The Worker keeps higher-priority lyric documents,
skips unchanged writes and processes only queue rows with checked membership.
Video clips remain additional links on the main song.

`third-party/yt-rust/` is a relative native-only Cargo dependency copied from
the supplied local yt-rust project. `src/native.rs::parse_ytm_songs` merges its
normalized search metadata with the host's renderer ordering, availability,
cover and clip hints; matched clip IDs stay attached to their audio-song row,
the paired video shelf row is hidden, and catalog sync sends it only as the
song's additional `clipVideoId` link. Suggestions retain host
highlighting/history fields.
Native and same-origin browser search share this path. Public transport,
browse/radio and audio/video resolution remain Vey services. No session provider
or account-data integration is enabled in that parser. YouTube account access is
a separate read-only
`src/youtube_browser.rs` service using an owned Chrome/Edge profile and normal
Google v3 account chooser. Cookies stay in that browser; only normalized metadata
crosses to `src/music_accounts.rs`. Provenance/local
patches and checks:
`third-party/YT-RUST-UPSTREAM.md`, `docs/YT-RUST-INTEGRATION.md`.

Audio resolution prefers a public visionOS context with matching visitor/device
and User-Agent data, retaining WEB_REMIX/Android fallbacks. Range downloads validate
byte boundaries and lengths, cancel outstanding tasks on failure, and atomically
publish complete cache files. Live reproduction and recovery: `docs/PLAYBACK-RECOVERY.md`.

Home recommendations use a separate retained track model. They build a bounded
profile from the current song, recent history, liked tracks and playlists; local tracks
first resolve an unambiguous song match by title, artist and duration. Native and
browser rank verified Blueberry catalog candidates first alongside several
public YouTube Music radio batches, rank shared results
higher, avoid the 12 most recent plays and liked tracks, and cap repeated artists
on the first screen. With no usable personalized results, they fall back to the
YouTube Music home browse. Native uses `ytm_recommendations`; browser uses the
same-origin `/api/match` and `/api/browse?radio=1` endpoints. Selecting a row
plays it and keeps the returned radio batch as the queue.
Native and Web rank candidates through the same local Rust profile, using play
frequency/recency, completions, early skips, likes, dislikes and playlist adds.
The versioned profile and recommendation snapshot persist locally and privately
in the revisioned Vey account document; a stale snapshot refreshes hourly.
Offline profile changes remain in the local Store until account sync resumes.

The desktop player offers standard, expanded and focus layouts. Expanded hides
navigation while retaining the dock; focus also moves transport beneath the
cover. Mobile retains its sheet. `ui/main.slint` preserves video and lyric-host
geometry callbacks across these layouts.

## Permanent files and artwork

`Store.downloads` maps public track keys to permanent local paths. Native download
completion registers a file source and local track; selections prefer that file
before stream resolution. Streaming cache files do not count as downloads.
The playback dock and Now Playing download buttons use the same native worker
and file registration as the track tools. Android stores downloads in Vey's
persistent app directory.
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
Automatic provider results prioritize Unison; distinct provider documents are
retained, and cached tracks refresh providers in the background once per session.
`Settings.lyrics_romanization`/`lyrics_translation` filter a display clone via
`braccato::visible_layers`, leaving cached/exported source and timings intact.
Native secondary lyric rows reserve 1.15× lyric-size plus the saved, adjustable
0–150% secondary-gap setting; the browser renderer uses the same preference.
The lyric settings panel is available from global Settings and player track tools.
lyrics_translation.rs enriches missing layers via Unison
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
Whole-song native wrapping/shaping uses a single bounded worker in
`src/native_lyric_layout.rs`. Requests retain only the latest pending layout;
generation checks reject stale resize/song results. Only owned strings, numbers
and cue data cross threads; Slint models are reconstructed on their owning thread.
Native ambient blur/color preparation and frame composition also use one bounded
worker; the UI uploads completed pixels and never waits for a frame job.
The adapter suppresses upstream demo line-word staggering: line cues stay line
cues. Timed romanization, backing vocals and syllables retain source timestamps.
See docs/NATIVE-BRACCATO.md and third-party/BRACCATO-RUST-UPSTREAM.md.

`tools/braccato-widget/build.mjs` still bundles the preserved JavaScript snapshot
for web/braccato/. Browser-only src/lyrics_widget.rs bridges the iframe. Windows
lyrics render with Slint; native video remains separate, and lyric authentication
uses an embedded child over the lyrics viewport with a system-browser fallback.
JWTs never belong in rendering documents. Braccato remains the rendering engine. Native
themes map supported CSS/RICS declarations to typed controls in src/themes.rs;
arbitrary CSS selectors/DOM behavior remain browser-only.

`src/theme_presets.rs` supplies 20 offline appearance presets to native/WASM
through `themes::apply_settings`; CSS, font overrides and custom edits keep the
existing Settings schema. `ui/lyric-theme-picker.slint` previews colors/type
and credits inside the player dialog. Six originals and seven authored native
community adaptations live under `assets/lyric-themes/`, with pinned references
in PROVENANCE.md. Presets preserve timing/follow policy; browser reduced-motion
overrides also disable preset letter waves, glow and highlight fade. See
`docs/LYRIC-THEMES.md` for scope and reproduction.

The 0.1.32 appearance panel (`ui/lyric-appearance.slint`) has an isolated draft,
fixed actions and an independent Braccato sample. `src/lyric_appearance.rs`
validates drafts and maintains one bounded CSS control block; native sample
glow/glyph/focus preparation reuses the existing workers. Sample clocks and
models never update the song. Apply commits; cancel/outside/Escape discard;
Reset clears the font override. Conditional/secondary CSS rules are reported
rather than flattened into main typography. The browser retains its upstream
RICS compiler for directives outside the native subset.

Since 0.1.33, Settings.theme_entire_player selects whole-player profiles in
src/player_themes.rs. Applied profiles own semantic palette roles, geometry,
interface font and ambient strength; disabled scope restores the saved color
mode/hue. Braccato's empty/default CSS retains cover-derived colors. Drafts
never write live Tokens before Apply. CSS/RICS --vey-* overrides stay in one
marked block and do not alter cue or scroll timing. Four new original Vey
styles join the original catalog. Native ambient preparation stops at zero
effective opacity, including minimal profiles.

tools/braccato-widget/player-motion-source.js builds generated
web/player-motion.js with bundled GSAP 3.15.0/CustomEase. Build the widget before
WASM bindings so scripts/build-web.ps1 copies the current ESM into snippets.
The bridge camera retains the host's path/elapsed time and restores owned styles
on cancel; native motion remains Rust/Slint. Native profiles do not execute
arbitrary DOM CSS, remote theme scripts/fonts or source fragment shaders.

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
Retained cue/release events update word activity and paint residency only at
deadlines; indexed warm windows skip model reads for ineligible preparation.
Seeks revisit active/previous/resident rows. Continuous fill still reads the
16ms clock. The worker precolors one shared glow atlas, preserving mask alpha;
layer opacity applies theme alpha without per-Image GPU tint textures.
`src/renderer.rs` is the view driver: only active/changed rows are written per
frame. `src/native_lyric_glyphs.rs` keeps immutable Slint glyph models shared
across cue updates, seeks and glow/color changes; `src/native_lyric_follow.rs`
holds the retained source-to-visual lookup so wrapped rows never acquire
inferred cues. `src/native_lyrics.rs` installs the adapter and owns the clock
and viewport windows.
The visible lyric clock ticks at 16 ms and slews small late media samples.
Normal mode uses zero scroll offset, zero lookahead and zero ending threshold.
CSS can opt into anticipatory camera/grouping; sung focus always uses actual
cue time. Native stage selection waits for real starts and retains real overlaps.
Committed groups do not scroll again at each cue. Active-band geometry handles
overlaps, pins tall wrapped sources and retargets immediately; manual/resume,
paused seeks and reduced motion remain exact. Sung activation keeps its clock.
`src/native_lyric_camera.rs` retains position and forward velocity when a new
cue retargets an unfinished native scroll. A camera at rest eases in and out;
monotone Hermite segments shorten settling for very close moving targets.
`FollowIndex` prepares bounded scroll destinations for cue/overlap groups after
layout and invalidates them with geometry/policy changes. Synthetic line-only
parts do not advance wrapped focus; their first row stays selected. Trailing
spaces share their word's shaping run. The next two rows warm up to one second
before their cue and remain retained during entry. The source cue clock is
unchanged. The Slint scroll animation is disabled while this driver owns the
position; manual/stage/passive paths retain their own ownership. Seeks/reduced
motion snap and manual scrolling rebases the driver. The diagnostic environment
switch `VEY_LEGACY_LYRIC_SCROLL=1` restores the previous ease-restart path for
A/B checks. `examples/native_line_flow.rs` samples the real window and asserts
retained velocity, prompt launch and exact settling.
The default glow is 6px at 31% opacity. Atlases crossfade two radii, or four
for broader custom glow. Normal fill preserves a shared shaping origin for
base/highlight; subtle whole-word wobble is enabled, while letter waves remain opt-in.
Layout-only motion intervals/origins keep syllables together and defer wrapped
continuations until their fill entry. Motion settles within the actual cue;
duration/strength CSS and reduced motion remain supported. Letter waves work
independently and instantiate glyph bodies only during their motion window.
Only viewport-adjacent row bodies instantiate word views. Focus crossfades
aligned sharp ink/cached blur for 180ms in and 280ms out, sharing one row
scale/origin. Settled rows retain one paint body; reduced motion switches immediately.
`FocusPaint` retains blend clocks outside model delegates and updates a separate
float model only while changing. Animated focus does not rewrite word-bearing
rows or invalidate their font/layout bindings. Seeks snap the blend together
with the camera; manual mode clears retained focus floats.
Multiword timed cues remain one shaping run until wrapping; normalized cue
offset/span carries the same fill edge across wrapped fragments without retiming.
Native fill is bounded by the supplied cue end; each wrapped fragment completes
before its continuation begins. The copied renderer exposes this as an opt-in
`finish-fill-at-cue-end` property so standalone upstream defaults remain available.
Wrapping keeps syllables of a word together, permits hyphen breaks and falls
back to whole grapheme clusters for oversized tokens. Visual follow uses each
fragment's measured fill-entry phase; canonical timestamps stay unchanged.
Whole-run/letter fill and glow use the same linear spatial ramp; empty/completed
coverage uses a solid brush. The Vey host opts into filled lead-prefix glow
through the active line; backing/secondary cues retain cue-bounded release.
Lead paint residency follows the line end and supports late seeks. Wrapped
fragments keep their actual fill edge. Hyphens permit wrapping but do not split
word-motion groups. Cached glow uses a solid tint and bounded source-texel
feather clips, all sampling the same full tile/origin on a physical-pixel grid.
Scroll uses a soft-start/stop cubic curve. Highlight fade uses
smoothstep without delaying cue fill. Reduced motion disables wave, animated
scrolling and focus crossfade.
Native installation checks renderer support once. Both backends paint shaped
solid text through fixed physical-pixel fill clips. GPU ink keeps one local
composition path at empty, partial and completed coverage before word/letter/
line transforms, preventing gradient glyph artifacts and clip seams. The old
gradient brush is available only in the isolated diagnostic probe. Reduced
motion retains timed fill. Native stable ink keeps the original conditional
neutral Text path and uses a fixed-size pool only during partial coverage, bounded to 64 cells.
Capacity comes from static word/glyph ramp width, not the playback clock;
the repeater does not resize within a cue. Retaining hidden pools on every
unsung/completed word regressed frame time, so solid words keep only neutral ink.
Keep neutral Text outside the feather's clips: moving it into a clip changes
fractional ligature rasterization at letter-wave handoffs.
Visible playing lyrics sample presentation time every 8ms, with full layout,
row residency, focus and paint preparation limited by a 16ms deadline. The
intermediate pass only projects the existing playback clock and updates fill
time. Entry and explicit seeks bypass that deadline. Hidden/paused polling
remains 16ms; minimized windows suspend work. `VEY_LEGACY_LYRIC_CADENCE=1`
restores the original 16ms cadence for matched diagnostics.
The native host windows row delegates around the actual viewport, retaining
exact omitted-row height as top/bottom spacers. A filtered notifying model
adds/removes boundary rows and preserves overlapping delegates. Nearby sharp bodies survive
focus changes. Font family/width measurements use revision-aware bounded
thread-local caches; imported fonts invalidate them. The player page caches its
full surface only during camera travel/warming, not every lyric playback tick.
Explicit backing phrase intervals survive display conversion as single cues;
untimed phrases retain line sync and never acquire estimated word timings.
Glow is clipped at the actual fill edge. Appearance CSS and selected font family
persist in Settings. Desktop TTF/OTF imports are validated, stored under the state
directory's lyric-fonts/ and registered with Slint fontique and both raster workers.
Font revisions invalidate layout/glyph caches; missing glyphs fall back to Noto.
Font files stay local; settings do not contain paths.
Native lyric changes no longer serialize an unused browser document. Desktop
video decodes through FFmpeg into Slint frames; Android uses a TextureView.
On Windows, lyrics verification uses an opaque Wry/WebView2 child over the lyrics
viewport and returns the one-use token through validated IPC. WebView startup or
loading failure can fall back to the existing ephemeral loopback/system-browser
flow. Other native targets keep that flow; silent automatic challenges are unavailable.

Slint directly composites native lyrics and the shared Rust ambient texture.
`src/ambient_native.rs` is the bounded native backdrop for platforms without the
browser Kawarp renderer: 256x144 preparation on artwork/option changes only, and
hidden/static work stops. The browser retains bundled Kawarp through its iframe host.
`src/material.rs` derives tonal roles on artwork changes, not playback ticks.
Dominant hue families use circular averaging and artwork chroma; grayscale
covers keep dynamic themes neutral. Kawarp uses 0.28 opacity, warp 1, 15 blur
passes, 1.5 saturation, 0.02 dithering and no tint, with bright-cover dimming.
The browser texture and native fallback add slow clock-driven rotation with
overscan. Native bilinear sampling lives in `src/ambient_motion.rs`; pause and
reduced motion stop continuous work. Mobile `TouchStyle` gives shared actions
44 px targets and pressed feedback without extra persistent backgrounds.
Texture transitions use one-second smootherstep fades. Palette transitions use
one second by default or the applied profile's duration. The
native lyric component shares the Slint surface and artwork without a child HWND. See `docs/APPEARANCE.md`.
Desktop page surfaces stay mounted in a compass grid; one interruptible camera
moves content and lyric host together. Route surfaces cache only while moving or
warming; settled pages paint live so asynchronous artwork updates are immediate.
Mobile Player is a retained-page overlay. The persisted reduced-motion switch
bypasses travel and shared decorative animation. Details: `docs/PAGE-MOTION.md`.
`src/navigation_fx.rs` keeps the two-snapshot blur worker behind the native
`VEY_LEGACY_NAVIGATION_CAPTURE=1` diagnostic switch. Normal native/WASM travel
uses live page surfaces, cached during travel, without readbacks. `ui/navigation-streaks.slint` follows
the same camera vector. The shared
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
`src/lyrics_widget.rs`: cubic-bezier(0.2, 0, 0, 1), 100–180 ms. Settings sections
commit immediately. Normal navigation uses live page surfaces without screenshot
readbacks; `VEY_LEGACY_NAVIGATION_CAPTURE=1` retains native snapshot blur only
for diagnostics. Browser lyric-host compositor blur remains available.
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
token cache, exchange and provider requests; `src/challenge.rs` opens the system
browser for the provider's Turnstile challenge and receives a bounded loopback callback.
Selecting Dacubeking explicitly, refreshing automatic lyrics, or using the
player Lyrics access action can start this user-completed challenge when a
valid JWT is missing or rejected; the JWT is stored locally and that request is
retried once. Normal song startup and server requests never open a challenge.
Duplicate refreshes share one in-flight native challenge. Windows Vey embeds the
provider page in an opaque child over the lyrics viewport; other native targets
use the system-browser loopback flow and web embeds the provider frame.
`/verify-turnstile` exchange accepts a JWT/token response. `/v2/lyrics` form
fields are videoId, song, artist, album, duration, alwaysFetchMetadata=false,
token. Preserve expected Origin/Referer and cookie-aware transport. Never print
tokens.
The separate current Better Lyrics `getLyrics` API is cache-first and does not
use that JWT. Auto mode queries it alongside the legacy and public providers;
explicit `betterlyrics` requests bypass old cached lyrics and local sidecars.
Direct KuGou matching uses title, artist and duration before fetching LRC;
existing plain/line caches are shown immediately and can be supplemented once
per song per session without replacing user-supplied or richer timed lyrics.

## Build/packaging boundaries

Package/binary: `vey`; Rust library/WASM artifact: `playervf`. Android package:
`com.playervf.rust`, API 26 minimum, target 35, ARM64. Native activity lives in
`src/lib.rs`. `src/android.rs` requests audio permission on the Java main thread
and queries MediaStore external audio on a worker. API 33+ uses READ_MEDIA_AUDIO;
older devices use READ_EXTERNAL_STORAGE. Grant resumes scanning; provider errors
retain the existing catalog. `src/dialogs.rs` still has file/save picker stubs;
MediaStore discovery does not implement SAF document import/export. Native safe
insets feed shared layout; dialogs also use keyboard bounds and scrolling.
Windows clip playback requires FFmpeg on PATH; the portable executable also needs the Visual C++ x64 runtime.
Release scripts generate `dist/` artifacts and checksums; inspect their current
parameters and signing docs before packaging. Never store signing secrets here.

`build.rs` also compiles `android/java/app/vey/VeyBridge.java` with `javac` and
`D8` (`--min-api 26`) using `ANDROID_HOME`/`ANDROID_SDK_ROOT` and `JAVA_HOME`.
A missing JDK or SDK fails the Android build before Rust links.

## Player refinement interfaces

`src/preferences.rs` applies/captures language and Kawarp options for both
controllers. `locales/catalog.tsv` feeds Slint, controller messages and the web
widget; source text is never translated. `ui/motion.slint` owns semantic motion
durations. Japanese fonts are bundled under OFL. Settings rows and switches live
in `ui/controls.slint`; native and WASM entry points fill `app-version` from
`CARGO_PKG_VERSION`. See `docs/MATERIAL-STYLE.md`.

`src/video.rs` resolves clips and sends muted visual state from the song clock
to browser HTML video, Android TextureView/MediaPlayer or desktop FFmpeg frames.
Empty/failing sources reveal the art slot. Android Java
source is compiled to embedded DEX by `build.rs` (JDK and Android SDK required).
See `docs/PLAYER-REFINEMENT.md` for contracts, dependencies and validation limits.

Native lyric requests start on track selection independently of audio loading.
`lyrics_service::fetch_with_progress` publishes normalized alternatives as each
provider completes, then caches the final quality order. Track generations and
one cancellable lyric task suppress stale/duplicate requests; explicit imports
cancel pending requests. Cache-write failures must not hide fetched lyrics.

Search input stays mounted with stable clear-action geometry. Windows lyric
verification places a Wry child over the lyrics viewport; other native targets
use the system-browser callback flow.

## Vey account sync

`src/account.rs` installs a shared native/WASM controller. Its separate opaque
session is excluded from Store exports; Basic sync allowlists video/cloud references,
preferences and history. Revision conflicts pause automatic sync. Private audio
uploads require server-checked developer/tester Premium entitlements; downloads
are account-scoped native files or temporary browser Blobs. See `docs/ACCOUNTS.md`.
The Settings account editor updates the profile and password through authenticated
Blueberry endpoints; raster avatars are capped at 2 MiB and stored in the backend's
MEDIA R2 binding. Profile data and the account session are separate from Store sync.
The Cloudflare D1/R2 implementation lives in the separate `blueberry_melody` repo.
Playlist metadata (`author`, `updatedAt`, `isPublic`, `sourcePlaylistId`) travels
inside the revisioned cloud library and preserves entry order/repeats. The
recommendation profile and listening history stay private to the account.
Authenticated catalog imports send metadata and cached lyric timings only for
eligible YouTube Music identities. The audio resolver uses the stable video ID.
Local paths, expiring stream URLs and listening history stay outside the shared
catalog. Native periodic preparation, cloud restore merging, playlist-import merging and account/session writes run on
workers. A store revision guard rejects a stale cloud restore and rebases a
stale playlist import. Web preparation still runs on the browser thread, and
native restore still applies UI models there.
Blueberry playlist profiles expose only explicitly public playlists.

Vey uses its own RFC 8628 OAuth device grant via the system browser. The default
public API is https://blueberry-melody.superstilist.workers.dev; manual server/email
entry remains available. Browser popup preparation is synchronous with the click;
consent is explicit, token polling obeys the server interval, and cancellation
invalidates the local pending flow. No client secret is shipped.

Native audio status retains a monotonic sample timestamp and projects its age
while playing (bounded to 250ms). Confirmed loads/seeks carry a serial that the
native controller forwards as `native-seek-revision` with the same position.
The lyric clock slews late advancing samples through future speed, never by
rewinding painted time. Camera cancellation follows confirmed seeks, page entry,
paused seeks and presentation changes, not clock deltas. Regressions:
`examples/native_lyric_song.rs`, `tests/fixtures/native_audio_polling.json` and
`examples/native_lyric_gap.rs`; details in `docs/NATIVE-BRACCATO.md`.

Native lyric measurements and optional letter plans use cached HarfRust shaping
with the selected font weight. Letter plans retain full-run kerning in em units,
keep ligatures/marks together, and fall back to one shaped run when isolated
clusters would alter the ink. Font revisions reject stale cache publications.
`LyricInk` separates logical advance from paint bounds and samples feathers
at physical-pixel resolution using premultiplied colors on both renderers,
without changing cue or fill clocks. `examples/native_lyric_quality.rs` writes to
a required new output directory; its captures support fractional display scales.
