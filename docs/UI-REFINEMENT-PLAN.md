# Vey UI refinement proposal

Drafted 2026-10-21 from the current working tree and existing screenshots.
This is an editable implementation backlog. The documentation task does not
implement these changes or establish fresh application/device test results.
The companion [functionality proposal](FUNCTIONALITY-REFINEMENT-PLAN.md) owns
service behavior; shared work is cross-referenced by its F identifier.

## Direction and evidence

Keep the current cover-derived tonal palette, rounded controls, organic artwork,
open player/lyric surfaces and restrained secondary actions. Follow
[Material style](MATERIAL-STYLE.md), [appearance](APPEARANCE.md) and
[page motion](PAGE-MOTION.md). Retain mounted pages, interruptible navigation,
the mobile player sheet and reduced motion. Do not restore the removed volume
focus frame, playback-speed control or selectable lyric themes.

Evidence labels used below:

- **Source:** behavior or structure inspected in current source.
- **Capture:** visible in an existing fixture screenshot; capture age/content
  limits the conclusion, and appearance must be reproduced before changing it.
- **Review:** a proposed acceptance check, not a demonstrated defect.

Directly reviewed in this task: existing desktop Library/Search, phone
Player/Appearance, tablet Settings and landscape Data screenshots. Earlier
planning also inspected desktop/phone Home and landscape Settings. The images
show several empty routes and a Japanese interface. They do not establish
populated-list usability, current live streaming, keyboard operation or real
Android behavior. [The refinement script](../tools/braccato-widget/refinement-test.mjs)
captures nine routes at 1440x900, 320x640, 900x900 and 740x390, and locale
Settings samd cawrite the word vry-proof into a file called proof.txt and read it backrd neples at 320x740. Its report is historical fixture evidence.

Priorities: **P1** blocks a core action or understanding of its result;
**P2** improves routine usability; **P3** is polish after those checks pass.
All entries below remain proposed. Source anchors identify the inspected code
region by symbol/behavior so line-number drift does not invalidate the links.

## Coverage map

| Surface | Main refinement | Completion scenario |
| --- | --- | --- |
| Home | Clear first action, honest chips and adaptable hero (U-02, U-04) | Empty library, recent history, playing track, long title |
| Explore | Loading/error/empty distinction and real detail destinations (U-01, U-02) | Refresh, failure/retry, mixed result types, pagination |
| Search | Query focus and useful no-results recovery (U-01, U-02) | Initial route, no matches, local results during remote failure |
| Library | Platform-specific import and readable rows (U-03, U-05) | Import, denied access, populated list, missing source |
| Liked | Clear unlike result and useful empty guidance (U-01, U-05) | Remove last like; recover via search/library |
| Playlist | Actions available when empty; clear deletion (U-05) | Empty/populated playlist, create-and-add, delete/cancel |
| Queue | Current-track identity and retained context (U-05) | Duplicate titles, end of queue, rapid selection |
| Player / Lyrics | Metadata space, coherent host states, accessible tools (U-04, U-06) | Long metadata, loading, retry, paste, seek, empty lyrics |
| Settings overview | Consistent hierarchy and short-screen access (U-03, U-07) | All six sections reachable and return position retained |
| Audio, Library, Lyrics settings | Correct capabilities and action feedback (U-01, U-03) | Volume endpoints, source access, provider/reduced-motion changes |
| Data settings | Separate account launch, lyric auth and data actions (U-01, U-03) | Export/restore/cancel, unavailable capability, auth retry |
| Appearance and Language | Legible labels and discoverable controls (U-07, U-09) | Eight locales, six hues, reset, persistence |
| Shared shell/dialogs | Input access, focus, motion and safe placement (U-07 to U-10) | Keyboard, pointer, touch, resize, IME and inset layouts |

## Proposed work

### U-01 — P1: show operation-specific states and recovery

**Evidence:** Source: [main UI](../ui/main.slint) renders a shared status banner
and a common zero-track panel. Explore's empty action is labeled search but
invokes refresh; Search can show no-results copy before a query. Capture:
[Search](screenshots/refinement-desktop-search.png) and
[Library](screenshots/refinement-desktop-library.png) illustrate the generic states.

**Change:** Give each route idle, loading, empty and failed presentations.
Keep existing usable results during refresh; show Retry for failed requests,
focus the query field for Search, and provide the relevant import/like/playlist
action for local emptiness. Display persistent failures beside the affected
action; retain operation feedback across unrelated navigation when needed.

**Entry/dependency:** main UI status/empty blocks and controller result handling
in [native](../src/app.rs) and [web](../src/web.rs); coordinate F-01/F-09/F-10.
**Acceptance:** Initial Search requests a query; submitted zero results show the
query; failed Explore refresh offers a working retry while retaining old rows.
A retry success clears only that operation's error, including after navigation.

### U-02 — P2: make navigation labels describe the destination

**Evidence:** Source: Home/Explore chip handlers search the translated chip
label; All navigates Home even from Explore. The services proposal records the
same mismatch as F-07. Public detail navigation remains a documented gap (I-07).

**Change:** Present chips as explicit search shortcuts until actual filtering
exists; show the resulting query and Search destination. Keep Explore's All
action on Explore. Distinguish public detail cards from playable song cards,
and label any unavailable destination before activation.

**Entry/dependency:** chip row and navigation in main UI, both controllers,
[stage navigator](../ui/stage-navigator.slint); F-07/F-08.
**Acceptance:** Every chip has a predictable route/query in all locales;
Back restores the previous context. An album card opens its detail view only
after F-08 provides that capability. Rapid route changes settle at the last
selection with a single shared camera transform.

### U-03 — P1: describe platform capabilities at the action

**Evidence:** Source: browser add-source always opens a file picker while the
shared Library header can say Add folder. Browser tag actions report native-only
support; Android file/save dialogs return no selection. Data settings also
places account launch and authentication messaging close together.

**Change:** Drive labels and enabled actions from controller capabilities.
Browser offers Add files and reselection guidance; Android offers music access
and permission recovery, then document operations when implemented. Name the
metadata action Review metadata match; explain native-only writes. Visually
group lyric-provider verification separately from Open YouTube Music and
backup/restore. An external account launch must not imply account sync.

**Entry/dependency:** main UI Library/Data actions; controllers and
[localization catalog](../locales/catalog.tsv); F-03/F-04/F-06/F-10/F-12.
**Acceptance:** File/folder labels match the actual picker on every target.
Unsupported operations give an explanation before work is attempted. Android
denial and browser reload each offer the correct recovery action without
claiming success; cancelling a picker preserves the current view and state.

### U-04 — P2: preserve useful text on narrow players and Home

**Evidence:** Capture: [phone Player](screenshots/refinement-phone-lyrics.png)
truncates the idle title in both heading and metadata; the earlier phone Home
capture truncates its hero title. Source: hero/title fields use elision and
the mobile sheet allocates fixed controls around those text areas.

**Change:** Allow the Home hero up to two lines with content-driven height;
keep the primary action visible. Reserve the player heading for navigation and
give track title/artist a dedicated two-line metadata area. Place secondary
actions in the existing tools/menu surface when width is insufficient. Keep
full metadata available in a readable detail view and accessibility labels.

**Entry/dependency:** main UI hero, mobile sheet, mini-player and artwork slot;
U-09 localization review. **Acceptance:** At 320px with Japanese/German text,
the primary action and all controls remain reachable. Real long titles may
truncate in the mini-player but can be read in full in details; blank/idle
metadata does not consume duplicate headings. Resize retains playback position.

### U-05 — P1: make collection actions safe and reachable

**Evidence:** Source: Delete playlist is inside the nonempty-track header,
so an empty playlist lacks that action. It invokes deletion directly. Row
highlighting compares title/artist rather than unique track identity.
The playlist picker already uses real writable destinations (retain this).

**Change:** Put playlist actions in a header independent of track count.
Require an explicit, named confirmation before deletion; cancellation keeps
selection and scroll. Identify the playing row by track key. Keep primary row
activation distinct from like/remove/download controls with touch-sized targets.
Show an updated empty state when the last item is removed.

**Entry/dependency:** main UI collection headers, row model and playlist dialog;
controllers/store; F-05/F-14. **Acceptance:** Empty and populated playlists can
be deleted; cancel preserves them. Tracks with identical metadata highlight
only the intended source. Row actions do not accidentally start another song;
picker create-and-add and unavailable-source recovery continue to work.

### U-06 — P1: keep lyric and video states understandable

**Evidence:** Source/design: Windows/web use an external Braccato host with
loading/retry states; other targets use a native fallback. Video failure returns
to artwork. The empty Player capture establishes layout only.

**Change:** Use distinct no-track, searching, no-lyrics, invalid-import and
host-retry messages. Keep paste, refresh and resume-follow controls discoverable
without placing diagnostic text in the Home surface. Explain unavailable clips
near the cover/video choice without interrupting audio. Preserve existing lyrics
after an invalid paste or failed refresh.

**Entry/dependency:** main UI lyric tools, [lyric host](../src/lyrics_widget.rs),
[widget](../tools/braccato-widget/widget.ts), [video](../src/video.rs);
F-11/F-12/F-13. **Acceptance:** Seek and rapid track changes never display stale
lyrics/video as current; retry recovers a lost host; reduced motion preserves
timing. Clipboard failure is actionable; video fallback leaves audio playing.

### U-07 — P1: prove keyboard and short-screen access

**Evidence:** Source: [controls](../ui/controls.slint) supplies accessibility
roles/actions for several TouchArea controls; Action/NavItem do not themselves
show FocusScope/key handlers. Volume forwards focus to a standard slider.
This is a focus audit target, not a verified failure of all native controls.
Capture: [landscape Data](screenshots/refinement-landscape-data.png) has little
remaining form space after persistent chrome. I-12 documents web canvas limits.

**Change:** Establish logical Tab traversal and Enter/Space activation for
custom controls; visible focus must fit the current borderless design. Dialogs
take focus, Escape/cancel dismisses them, and focus returns to the opener.
Retain the frameless volume wrapper. Preserve scrollable forms, visible Back
and keyboard-aware dialog bounds; reduce nonessential chrome in short views.

**Entry/dependency:** controls and main UI layout/dialogs; F-16 accessibility
decision. **Acceptance:** Core native flows complete using keyboard alone;
no invisible focused control or focus trap. At 740x390 and Android inset/IME
fixtures, every form action can be scrolled into view and tapped. Web keyboard
results and screen-reader limitations are recorded separately from native results.

### U-08 — P2: standardize spacing, targets and content density

**Evidence:** Source: [token file](../ui/tokens.slint) is generated from
[design tokens](../assets/design-tokens.json), while main UI uses many local
spacing/font values. Capture: tablet Settings has large repeated tiles; phone
Appearance reveals only the first group before scrolling. These are density
review opportunities, not proof that remaining controls are unreachable.

**Change:** Use existing spacing/type roles for equivalent controls. Retain
44px touch actions, keep desktop compactness and make settings grouping obvious
without adding cards to the player. Adjust the sidebar breakpoint only after
populated tablet layouts establish actual content pressure.

**Entry/dependency:** design-token source/generator, controls and main UI;
U-10 fixtures. **Acceptance:** Shared action/row metrics agree; long labels fit
without colliding. Header, list and settings content remain usable at all
four viewport classes. Generated token output stays reproducible.

### U-09 — P2: review all locales and tonal states

**Evidence:** Source: eight locales share catalog-backed copy; source track
and lyric text is intentionally preserved. Existing locale captures principally
cover Settings, not every dialog or error. Palette changes already persist and
respect reduced motion.

**Change:** Review dynamic messages, plural counts, auth/download errors,
dialog labels and fallback strings alongside longer German/Japanese text.
Check actual foreground/background pairs with warm, cool, neutral, transparent
and bright artwork in Dynamic, Static and Background-only modes.

**Entry/dependency:** localization catalog, [i18n](../src/i18n.rs),
[material palette](../src/material.rs); F-15. **Acceptance:** Eight locales can
complete import, playlist and restore flows with no missing keys or overlapping
labels. Target text contrast is 4.5:1 for normal text and 3:1 for large text;
essential controls/focus indicators target 3:1. Capture any unmet pair before
adjusting roles; retain hue semantics, one-second fades and immediate reduced
motion changes. Source metadata/lyrics are never translated.

### U-10 — P1: strengthen the visual evidence before broad polish

**Evidence:** Capture/script: route screenshots often show empty collections;
the script seeds one public fixture track and records navigation without a full
populated-library/content-state matrix. No device acceptance follows from that.

**Change:** Extend isolated fixtures with local/public tracks, loaded/missing
art, duplicate titles, long metadata, several playlists and a queue. Capture
idle/loading/populated/empty/error states relevant to each route, plus create,
delete, metadata-match and lyric-paste dialogs. Start with U-01/U-03/U-05 so
fixtures check observable behavior as well as screenshots.

**Entry/dependency:** [snapshots](../src/snapshots.rs), refinement/phone/playlist
scripts and their reports; F-17. **Acceptance:** Every coverage-map row has a
recorded scenario, viewport, locale and result. Screenshots have actual manual
review notes; script exit alone is insufficient. Refresh reports only after
execution, keeping previous historical evidence identifiable.

## Delivery and verification

1. **Reliability and access:** U-01, U-03, U-05, U-06, U-07, with U-10
   fixtures for the affected paths; coordinate the owning F tasks first.
2. **Routine usability:** U-02, U-04, U-08 and U-09 after state/capability
   behavior is reliable. Keep public-detail UI dependent on real F-08 results.
3. **P3 polish:** only measured spacing/type adjustments left by fixture review;
   capture before/after on the same content rather than redesigning the app.

For later implementation use [workflow commands](../.agent/workflow.md): rebuild
affected native/WASM/widget assets, then run only relevant fixture suites.
From tools/braccato-widget, the existing commands include npm run test:phone,
npm run test:playlists, npm run test:appearance and npm run test:transitions.
Run npm run test:video before npm run test:refinement because it generates the
video fixture. Inspect script setup: the static fixture server (4181) and app
backend (4173) have different roles. Do not infer device results from browser
emulation. Record Windows native keyboard/manual results and actual Android
inset/IME/device results for any platform claim. Keep user data isolated.

This documentation pass checks links, source anchors, scope and cross-references.
It does not rerun the application checks listed above. Completion of a future
UI task requires its stated acceptance evidence and the associated functional
scenario, not just a matching screenshot.
