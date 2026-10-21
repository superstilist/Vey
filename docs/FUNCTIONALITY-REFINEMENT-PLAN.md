# Vey functionality refinement proposal

Drafted 2026-10-21 from current source and repository reports. This document
covers user-facing capabilities and their integration paths, rather than every
private Rust helper. It is a proposed backlog; no application fixes, live checks
or device acceptance are claimed by this documentation task. The companion
[UI proposal](UI-REFINEMENT-PLAN.md) owns presentation and interaction details.

## Baseline and priorities

Use the [project map](../.agent/project.md),
[architecture](../.agent/architecture.md), [issues](../.agent/issues.md) and
[parity inventory](PARITY.md) as the starting inventory, then resolve conflicts
against source. Inspection in this task traced shared callbacks into native/web
controllers and sampled storage, browser import, Android dialogs, public service
interfaces and video wiring. It was not a runtime audit of every branch.

**Source** below means inspected current behavior. **Reported** means existing
documentation evidence that needs reproduction. **Unverified** means acceptance
is missing. **Proposal** means a deliberate improvement, not an existing defect.
P1 covers reliability, recovery and completing core actions; P2 covers feature
completion and routine usability; P3 covers maintenance after those pass.
No critical data-loss finding was established in this pass.

Preserve the standalone Vey binary/playervf library boundary, version-1 store
compatibility and camelCase track/history fields. Keep old saved speed values
normalized to 1x: playback speed is no longer a user control. Braccato is the
selected lyric theme; retained RICS/theme APIs do not imply selectable-theme UI.
Google session/cookie capture, account library APIs and voice remain excluded.

## Capability and callback inventory

All entries are planned or acceptance-only work. The named callbacks refer to
[the shared UI](../ui/main.slint); handlers are in
[native](../src/app.rs) and [web](../src/web.rs), with services below them.
This inventory includes internal presentation integration where it affects users.

| Capability / shared callbacks | Inspected behavior and next work |
| --- | --- |
| Navigation, search, refresh-explore | Native search_generation and web generation reject stale browse/search results; route caches exist. Review state/cancellation and public detail gaps (F-07/F-08). |
| select-track, play-toggle, previous-track, next-track, seek-to | Native audio/browser audio with portable Queue. Retain end-of-track and failed-load behavior; verify stale selection handling (F-01/F-02). |
| volume-changed, toggle-shuffle, toggle-repeat | Shared persisted settings; repeat cycles off/all/one. Preserve existing volume pixel fix and keyboard endpoints (F-02/F-15). |
| add-path, scan, add-source, remove-source | Desktop file/folder scan; Android MediaStore; browser file selection and unsupported manual-path message (F-03/F-04/F-19). |
| toggle-like, create/open/add-to/remove-from/delete-playlist | Local Store-backed collections; picker create-and-add implemented (F-05). |
| sync-tags, apply-tags, restore-tags | Native reviewed tag writes/backups; web sync/restore explains native-only support and has no apply-tags binding (F-06). |
| download, radio | Native download returns final status; web opens audio endpoint; radio returns discovery tracks (F-07/F-09). |
| migrate, backup, restore, clear-history | Native dialogs and browser JSON flows; Android dialog stubs; browser write errors ignored (F-03/F-10/F-14). |
| request/refresh/import/copy-lyrics, choose-provider, provider-changed, adjust-offset | Shared parsing, provider alternatives and offsets. Clipboard handling differs by target (F-11/F-12). |
| lyric-mode-changed, lyric-user-scroll, lyric-resume, lyric-geometry | View/follow/host integration, with external Windows/web renderer and native fallback (F-11/F-13). |
| open-login, open-challenge, submit-auth | External YouTube Music launch is separate from lyric-provider token acquisition (F-12). |
| change-art-mode | Cover/video selection persisted; video follows song clock and is muted (F-13). |
| preferences-changed, appearance-changed, artwork-theme | Shared language/background preferences and native/web tonal palette integration (F-15). |
| Startup, shutdown, event drain, artwork workers, persistence | Entry/controller/storage integration; bounded work and shutdown flushing must be retained (F-14/F-17). |
| Keyboard, accessibility, history and responsive shell | Roles exist on custom controls; browser canvas screen-reader limitation remains (F-16; UI U-07). |

## Reliability and recovery

### F-01 — P1: establish current public playback behavior (I-01)

**Evidence:** Reported: upstream ranged audio returned 403 for two tracks.
Source: [server audio handler](../src/server.rs) delegates to native cached
audio; [native services](../src/native.rs) implement stream resolution/cache.
Search results alone do not establish successful decoding or audible playback.

**Work:** Reproduce search -> resolve -> range fetch -> decoder -> audio output
using representative public tracks, including an unavailable item. Classify the
actual failing stage before changing transport. Keep errors actionable and retry
bounded; retain the selected track and avoid any account/session workaround.
**Interface/dependency:** Preserve existing audio route/Track identity; add a
stage-specific error/result only where controllers currently lose that context.
**Acceptance:** Record the current live outcome and environment. Fixture expiry,
403, network interruption and unsupported audio each recover or show a useful
error; retry succeeds when a valid source is supplied. Selecting B while A
resolves never starts A. A service block remains a documented external limit
unless fresh evidence establishes a fix.

### F-02 — P1: verify local playback and queue invariants

**Evidence:** Source: [audio](../src/audio.rs) and
[Queue](../src/services.rs) provide transport, selection, shuffle and repeat;
native Previous restarts a song after three seconds. Existing audio smoke is
historical and device dependent.

**Work:** Preserve current transport semantics and close only reproduced gaps.
Track selection must use source identity, keep queue order stable across view
refreshes and recover from missing/invalid files without wedging controls.
**Interface/dependency:** Keep portable Queue/controller behavior aligned;
UI U-05 owns row identity and U-07 owns input access.
**Acceptance:** Exercise empty/single/multiple-track queues, automatic advance,
manual next/previous, repeat off/all/one, shuffle, seek endpoints, mute/full
volume and pause/resume. Repeat-one must not trap manual Next. Invalid source
followed by valid selection recovers; rapid A/B changes retain B. Run actual
native output smoke separately from browser audio-object fixtures.

### F-03 — P1: finish Android document operations and device acceptance (I-03/I-10)

**Evidence:** Source: [dialogs](../src/dialogs.rs) returns None for Android
pick/save operations. [Android bridge](../src/android.rs) and
[Java host](../android/java/app/vey/VeyBridge.java) implement MediaStore access;
native add-source triggers scanning. Unverified: real device behavior.

**Work:** Implement system document selection/export using Android content URIs
and grant lifetimes; route JSON import/export and migration through actual
read/write results. Keep MediaStore discovery and document selection distinct.
Show grant/deny/revoke outcomes and preserve the catalog after provider failures.
**Interface/dependency:** Add a target document abstraction carrying readable/
writable content access rather than forcing content URIs into desktop PathBuf
semantics. Use it at relevant callbacks; existing desktop paths stay compatible.
**Acceptance:** On an identified device/emulator test grant/deny/regrant/revoke,
SD/device audio, cancelled pickers, JSON export/import round trip, unavailable
provider, lifecycle/rotation and settings persistence. Install/launch/playback,
seek/volume and external-browser intent results must be recorded. Packaging or
inset fixtures alone cannot close this task; coordinate U-03/U-07.

### F-04 — P1: browser metadata and source reselection (I-05/I-06)

**Evidence:** Source: [pickFiles in bridge](../web/bridge.js) creates browser://
keys from name/size/lastModified, session object URLs and filename-only metadata.
The browser add-source handler ignores file/folder kind and imports those rows.

**Work:** Read supported tags, duration and embedded artwork from selected bytes
with bounded resource use. Retain filename fallback for missing/malformed tags.
Expose unavailable-session sources after reload and let users relink by the
existing stable key without losing likes/playlist references. Revoke replaced
or discarded object URLs while retaining those used by active audio.
**Interface/dependency:** Extend the existing picker Track payload and add an
ephemeral availability lookup/reselection action; no mandatory persistent file
permission or store-key migration. UI U-03/U-05 owns labels and recovery.
**Acceptance:** Tagged/untagged/malformed/large files import without blocking
navigation; duplicate imports update one identity. Reload preserves collections,
marks missing session files and relinks a matching selection. Cancelling or
selecting a different file leaves unmatched entries unavailable and intact.

### F-05 — P1: collection lifecycle and stable selection

**Evidence:** Source: [Store](../src/services.rs) and controllers implement
likes/playlists/history. Delete removes the selected playlist immediately;
UI exposes it only for nonempty playlists. Native clear-history clears storage
and status without invoking refresh, unlike the browser handler.

**Work:** Make deletion available for empty playlists with explicit confirmation;
refresh visible history immediately after clearing. Retain real destination IDs,
create-and-add, deduplication and source identities after scanning/reselection.
**Interface/dependency:** Preserve serialized Playlist/History shapes; add UI
confirmation state rather than a new storage format. Coordinate U-05/F-04.
**Acceptance:** Create blank/nonblank names, add twice, remove last item, cancel/
confirm delete and clear history while Home is retained. No deleted row remains
visible; surviving collections persist after restart. Missing source references
remain recoverable. Confirm play/shuffle/repeat behavior after collection edits.

### F-06 — P2: make metadata review and restore complete

**Evidence:** Source: native sync-tags finds a candidate, apply-tags writes with
backup via [local_sync](../src/local_sync.rs), and restore-tags restores it.
Web sync/restore reports native-only support; apply-tags has no web binding.
This is a capability difference, not authorization to add browser disk writes.

**Work:** Tie review, write and restore results to the source path; update that
track's visible metadata/artwork after a successful write/restore. Keep review
explicit and hide/disable unsupported write actions with an explanation.
**Interface/dependency:** Reuse TagSnapshot/backup APIs, with track-keyed result
events where necessary. UI U-03/U-05 owns presentation.
**Acceptance:** Correct and ambiguous matches, cancel, read-only/unsupported file,
backup failure and restoring after restart all have explicit outcomes. A failed
backup must not lead to an unprotected write. Switching songs while review is
pending cannot write to the newer selection. Re-read tags to verify changes.

## Feature completion

### F-07 — P2: honest Home/discovery/search behavior (I-07)

**Evidence:** Source: chips pass translated labels to Search; All routes Home.
Native Search first filters local title/artist/album, then requests public
results; generation counters already prevent stale responses. Recommendations
and suggestions exist in native services; exhaustive UI integration is unverified.

**Work:** Make the existing chip behavior explicit as search shortcuts; keep
Explore's All action on Explore. Preserve local results if remote search fails,
and distinguish local/public results and radio context. Keep refresh deliberate
and revisits cached. Record which suggestion controls actually reach the API
before adding any new one.
**Interface/dependency:** Carry explicit query/context instead of using display
labels as an implied filter contract. UI U-01/U-02.
**Acceptance:** Empty query, local-only result, public-only result, combined
result, remote failure, cached revisit, refresh and radio from a selected song
have meaningful outcomes. Slow query A cannot replace B or a newer route.
Locale changes do not turn a claimed local filter into unrelated network work.

### F-08 — P2: public details and continuation (I-07)

**Evidence:** Source: native YtmBrowseItem distinguishes public result kinds;
ytm_browse takes browse_id/limit but returns a flat vector. Server search/browse
exposes Track vectors. Reported: album/artist/playlist detail and continuation
UX is incomplete. Local open-playlist handles writable local playlists only.

**Work:** Retain browse identity and kind through controllers and implement
read-only public album/artist/playlist detail destinations, track selection and
incremental loading. Extract and forward real upstream continuation tokens.
**Interface/dependency:** Add a public browse result/page contract with items,
detail identity and optional continuation, separate from writable local Playlist.
Keep existing song search/audio routes compatible; add detail/page API capability
for web only after native service support exists. UI U-02/U-05.
**Acceptance:** Each kind opens the right detail; append pages without duplicate
items, show true exhaustion, retry failed next page and discard results for an
abandoned detail. Back restores parent results/scroll. Missing IDs and expired
continuations are recoverable; public cards never become local destinations.

### F-09 — P2: truthful downloads with cancellation (I-07)

**Evidence:** Source: native on_download launches ytm_download_track and receives
a final message; the service writes into system Music/YouTube Music. Web opens
/api/audio/{id} in a new tab. These paths do not expose a unified job lifecycle.

**Work:** Give native downloads observable progress and cancellation, clean
temporary output on failure, and report destination/completion only after a valid
file is committed. On web provide an explicit browser-managed save action and
accurate capability copy; if app-managed progress is added, it must reflect
actual streamed bytes and cancellation, not opening the audio tab.
**Interface/dependency:** Add track-keyed download job events with transferred
bytes, optional total and completion/failure/cancel outcomes; use a cancellation
handle in the native worker. Reuse F-01 transport; F-03 owns Android destinations.
**Acceptance:** Known/unknown length, repeated request, cancellation, expired
stream, disk-full/permission failure and retry leave no completed-looking partial
file. Progress never fabricates a percentage without a total. Browser save
completion is described within what the browser can actually report (U-01/U-03).

### F-10 — P1: validate backup, restore and migration outcomes

**Evidence:** Source: native/web restore accepts Store version 1; migration is
allowlisted. Browser reads/pickers have some ignored error paths. Native restore
replaces the store; its UI currently lacks a dedicated replacement confirmation.

**Work:** Preview/confirm replacement, validate before changing live state and
show explicit read/parse/version/cancel outcomes. Apply restored preferences and
rebuild visible collections consistently. Explain missing browser session media
after a valid restore. Keep lyric credentials outside all backups/migration.
**Interface/dependency:** Reuse Store v1/defaults and the current migration
allowlist; return typed operation errors through callbacks. F-03 document
transport, F-04 reselection and U-03/U-07 dialogs.
**Acceptance:** Native/web round trip of tracks, collections, offsets and
preferences; malformed JSON, unsupported version, omitted old fields, picker
cancel and unreadable files preserve existing state. Check serialized output
for excluded credentials. Restored old playback-rate values normalize to 1x.

### F-11 — P2: fixture-led lyric compatibility and editing (I-09)

**Evidence:** Source: [parser](../src/lyrics.rs),
[TTML conversion](../src/braccato/ttml.rs) and controllers handle provider,
paste and cached alternatives. Recent fence/escaped-space fixes are documented
in [lyric input](LYRIC-INPUT.md). Coverage of all provider dialects is incomplete.

**Work:** Add only reproduced missing dialects using minimal nonpersonal
fixtures; retain timing classification, background vocals, translations,
romanization, metadata, offsets and malformed-import retention. Keep copy/paste,
alternative choice, refresh and follow/resume behavior consistent by capability.
**Interface/dependency:** Preserve canonical parsed documents and syncType;
adapt providers/hosts without editing vendored upstream code. U-06.
**Acceptance:** Plain/LRC/enhanced/QRC/TTML/SRT/VTT/JSON and actual failing
provider fixtures reach native/web adapters without inventing word timing.
Malformed XML/failed refresh retains old lyrics. Late results cannot replace a
new track. Copy reports failure where unsupported; Android's native copy handler
currently compiles out the desktop clipboard operation, so add a supported
bridge or explain availability rather than silently succeeding.

### F-12 — P1: platform lyric authentication acceptance (I-04)

**Evidence:** Source: [challenge](../src/challenge.rs) has Windows/Android
hosts; [lyrics service](../src/lyrics_service.rs) coordinates token cache,
refresh and retry; browser uses challenge/exchange through the backend.
Unverified: Android acquisition. Linux has no integrated challenge host.

**Work:** Verify available hosts end to end and make Linux's unsupported
automatic challenge path explicit, with the existing manual token route where
available. Preserve separate lyric auth storage, concurrent refresh coordination,
expiry handling, cooldown and one retry after 401. External YTM launch remains
a separate capability. U-03/U-06 owns user-facing explanations.
**Interface/dependency:** Retain verify/lyrics API semantics and provider
Origin/Referer behavior; report acquisition/verification failures without tokens.
**Acceptance:** Fresh acquisition, cached auth, expiration, simultaneous lyric
requests, invalid token, 401 once, repeated 401 and network failure have bounded
outcomes and recover on retry. Record Android results on a device. No secret
appears in rendering documents, logs or exported backups.

### F-13 — P1: render-host/video acceptance across targets (I-02/I-08)

**Evidence:** Source: [video](../src/video.rs),
[Linux adapter](../src/video_linux.rs),
[browser surface](../web/video-surface.js) and lyric host implement target
adapters. Windows/web use upstream Braccato; Linux/Android use a smaller native
lyric renderer. Browser fixture reports are historical; live/device scope differs.

**Work:** Exercise muted video against the song clock, cover fallback and host
startup/retry/disposal. Describe fallback limits per target; retain upstream
effects and timing on supported hosts. Do not add a second audible clock.
**Interface/dependency:** Keep nullable VideoSource and song identity; retain
stale resolution cancellation, hidden-media pause and release semantics.
**Acceptance:** Play/pause/seek, clip shorter than song, missing/failed clip,
rapid switch, hide/show, resize and shutdown preserve audio and release resources.
Test Linux without FFmpeg and Windows host failure. Inspect actual native
composition/lyric crop; browser success cannot establish Android or Linux runtime.

## Shared behavior and acceptance

### F-14 — P1: persistence errors and refresh consistency

**Evidence:** Source: [storage](../src/storage.rs) saves through a pending file
and rename, with one coalescing worker, shutdown flush and unreadable-state preservation.
Web save ignores localStorage set_item errors and unavailable storage.

**Work:** Surface browser persistence failure once per relevant failure episode
with backup/retry guidance; preserve usable in-memory state. Retain native
coalescing and flush behavior, and ensure mutations update retained views.
**Interface/dependency:** Return persistence status to the web controller/UI
without storing diagnostics in the user data schema. U-01/F-05/F-10.
**Acceptance:** Browser quota/disabled storage and native read/write/rename
failure report outcomes; later success clears the failure. Shutdown saves the
latest queued mutation. Corrupt native state remains preserved before recovery;
likes/history/playlist changes refresh without reopening the application.

### F-15 — P2: preferences, localization and compatibility

**Evidence:** Source: [preferences](../src/preferences.rs),
[i18n](../src/i18n.rs) and [material](../src/material.rs) share language,
background controls and palette behavior across controllers. Eight languages,
three color modes and six fixed hues are implemented, rather than missing work.

**Work:** Audit supported settings from UI -> callback -> stored value -> applied
runtime -> restart/restore. Resolve only demonstrated native/web drift. Keep
Japanese font coverage and dynamic status localization; preserve source lyrics.
**Interface/dependency:** Keep compatible defaults and color/rate semantics;
do not reintroduce old preferences without an explicit product decision. U-09.
**Acceptance:** All settings, reset, reduced motion, rapid palette changes and
legacy backups work after restart/restore on native/web. Missing locale values
fall back predictably; presentation bounds/contrast pass U-09 scenarios.

### F-16 — P2: deliberate input/accessibility support (I-07/I-12)

**Evidence:** Source: custom controls declare some roles/actions and volume
forwards standard input. Reported gaps include original shortcuts/menus.
Documented browser Slint canvas shell has no screen-reader support.

**Work:** Inventory actual shortcuts and exposed actions before adding parity
features. Complete keyboard access for transport, navigation, collection and
dialog actions. Assess current Slint/platform accessibility support; record a
specific supported path before promising browser screen-reader parity. A DOM
shell alternative would be a separate architectural implementation decision.
**Interface/dependency:** U-07 owns focus/activation; callback capabilities own
action availability. No full web-shell rewrite is assumed in this proposal.
**Acceptance:** Core keyboard scenarios pass on Windows and web with no trap;
native assistive-tech checks record roles, labels and current values. Publish
the web screen-reader limitation and assessment outcome; lyric DOM accessibility
alone cannot close shell accessibility. Context actions invoke real callbacks.

### F-17 — P1: platform/runtime and performance evidence (I-10/I-11)

**Evidence:** Reports provide Windows/browser fixtures; Linux runtime and
Android device acceptance remain unverified. Source already bounds artwork/
background work and coalesces saves; this proposal does not claim a speedup.

**Work:** Create a dated acceptance matrix for Windows, browser, Android and
Linux with actual environment, build, scenario, outcome and unresolved limits.
Measure import/artwork, navigation interruption, lyric/clip switching and idle
hidden/paused work on the same fixture before/after relevant changes.
**Interface/dependency:** Retain event/request budgets and hidden-work suspension;
no timing or callback rewrite without evidence. U-10 supplies reproducible data.
**Acceptance:** Each target has recorded launch/playback/storage/lyrics results
or a concrete unavailable-environment reason. Record actual Linux CI result
separately from the presence of a workflow. An unavailable target stays open;
browser emulation/build completion does not substitute for device acceptance.

### F-18 — P3: reconcile maintenance and release documentation (I-13)

**Evidence:** Documentation still mentions playback rate and selectable themes
in places despite current UI/source changes. Build/runtime evidence is mixed
with older reports. Package identity is Vey; compatibility library/storage names
remain PlayerVF.

**Work:** Reconcile README/parity/integration guidance after each accepted task,
keeping historical reports dated. Update only affected .agent constraints and
commands. Packaging uses current version and affected generated assets.
**Interface/dependency:** No rename of storage/bridge/library identifiers;
production signing remains a separate release decision.
**Acceptance:** Current docs agree with source capabilities and platform limits;
linked commands/paths exist. A later release records actual package build,
checksums and target acceptance rather than copying previous results.

### F-19 — P1: library scanning, metadata and artwork resilience

**Evidence:** Source: native scan/add-path/add-source/remove-source callbacks
normalize configured sources and refresh the catalog through
[native scanning](../src/native.rs). [Artwork workers](../src/artwork.rs) provide
bounded decoding/cache work. These established paths need populated-library
acceptance, rather than replacement based on the empty Library capture.

**Work:** Verify file/folder sources, overlapping folders, rescans, missing paths,
malformed metadata and unreadable artwork. Show scan status and source-specific
failures while retaining unrelated usable tracks. Preserve collection references
when a source disappears and becomes available again; distinguish removing a
configured source from deleting a media file.
**Interface/dependency:** Keep Source/Track identities and existing normalized
paths. Carry source identity in scan errors where currently lost; F-03/F-04 own
Android/browser transport, U-01/U-03/U-05 own feedback and rows.
**Acceptance:** Repeated and overlapping imports do not duplicate identities;
supported tags/duration/covers appear, missing tags retain useful fallbacks and
corrupt art does not prevent playback. Remove/re-add a source without deleting
its files or losing recoverable likes/playlists. Large fixture scanning/artwork
work stays off the UI thread, late results cannot overwrite a newer catalog,
and F-17 records responsiveness on the same workload before/after changes.

## Sequence and validation

1. **Reliability baseline:** F-01/F-02/F-05/F-10/F-14/F-19 plus U-01/U-05/U-10.
   Reproduce failures before fixing transport or queue behavior.
2. **Platform recovery:** F-03/F-04/F-12/F-13 with F-17 evidence and U-03/U-07.
   Start platform-specific acceptance as soon as an environment is available;
   independent desktop/browser work can continue while a device is unavailable.
3. **Feature completion:** F-06 through F-09, F-11 and F-15/F-16; their owning
   UI entries specify presentation. Public details depend on service paging;
   downloads depend on known transport behavior.
4. **Maintenance:** F-18 accompanies completed work. None of these proposals
   marks a pre-existing issue resolved or authorizes a release.

For future changes use [the workflow](../.agent/workflow.md). Rust behavior uses
focused existing tests first, followed by required integration/build checks.
Existing suites include [native services](../tests/native_services.rs),
[lyric fixtures](../tests/lyrics_fixtures.rs),
[upstream comparisons](../tests/braccato_upstream.rs) and
[settings](../tests/theme_settings.rs). cargo test --locked --lib --tests is the
broader Rust command; native audio smoke is an explicit ignored/device check.
Check WASM with cargo check --locked --lib --target wasm32-unknown-unknown;
rebuild bindings when browser behavior changes. Relevant widget suites are
listed in [package scripts](../tools/braccato-widget/package.json); check server,
asset and FFmpeg fixture prerequisites before execution. Real network/hardware
checks are additional evidence for the corresponding task, not inferred results.

The documentation task validates links, callback/source anchors and cross-task
coverage only. Later task reports should state what ran, on which target, the
observable result and any unresolved external dependency. Keep credentials,
personal media and full logs out of .agent and these proposals.
