# Download marks and local artwork

The small rounded triangle means a track is available as a local library file,
whether imported/scanned manually or downloaded through Vey.
By default it points up, sits inside the cover's bottom-right corner, and uses
the supplied dark-blue example style. It is shown only on song lists and cards,
including gallery, home and the standalone queue page. Player-page covers,
the embedded player queue and mini-player never show a badge. Its accessible
label and activation message are “Available locally”. Temporary streaming cache files
do not receive this mark.

Settings → Appearance → Local file badge can hide the mark, choose a rounded
triangle or local-file/check icon, and select any of the four corners. The icon
uses a light stroke on a dark circular plate and a minimum 20px badge, preserving
legibility on small, bright or dark covers. Native and browser
preferences share persisted `download_badge_shown`, `download_badge_style` and
`download_badge_position` settings. Defaults are shown/triangle/bottom-right;
older settings receive these defaults. Badge visibility does not change whether
a file is saved or available offline. Player media surfaces have no badge API.

Native library scans populate a normalized local-path index used by both initial
rows and cached-page/player updates. Successful Vey downloads join that same index.
Browser imports receive the mark only while their file URL is available in the
current session; an old stored browser path alone does not establish availability.
The existing `TrackRow.downloaded`/`current-downloaded` property names remain for
compatibility, but now drive this broader local-availability presentation.

`SongRow` in `ui/controls.slint` supplies the same artwork, title/artist spacing,
duration and action alignment to page lists, compact home rows and player queue.
Album columns appear in wide lists; compact rows include the album below the
title. Active rows compare track keys rather than potentially duplicate titles.

## Native desktop

- Download completion registers both the public track and its local file,
  adds the file as a library source, and persists their association. Repeated
  clicks during a download are ignored. The player prefers the saved file
  when the public track is selected again, including without network access.
- Downloads include title, artist, album and an embedded JPEG front cover.
  The catalog thumbnail is preferred, with a video-thumbnail fallback if its
  request fails. Files are tagged on a worker before publication by rename.
  A failed artwork/tag write does not publish a completed download or mark.
- Filenames include the public video ID to distinguish recordings that share
  a title and artist. New downloads reuse the stream cache instead of fetching
  the audio again when it is already complete.
- Track tools offer cover selection for local/downloaded files and JPEG cover
  export. Cover replacement supports MP3, M4A/M4B/MP4, FLAC, Ogg and Opus tags.
  Other formats report that artwork writing is unsupported.
- Replacement edits a staged copy, preserves text/other pictures, and backs up
  original embedded front-cover bytes beside the audio file before commit.
  Embedded front covers take priority over other pictures and remote thumbnails.
  Invalid folder images are skipped so another valid image can be used.
- Artwork and download flags update cached page models as well as the active
  list. Missing permanent files are removed from download associations on
  startup/library scan; selecting a missing saved track falls back to streaming.

The player tools panel scrolls to keep its actions reachable at short heights.
Android's unfinished file dialogs remain hidden for artwork editing/export.
Browser rows do not infer permanent downloads from temporary audio responses.

## Implementation and checks

`src/media_library.rs` owns bounded image requests, JPEG conversion, tag writes
and staged local cover replacement. `src/app.rs` owns download identity/state
and worker callbacks; `Store.downloads` persists public-key-to-local-path links.
`src/artwork.rs` reads front covers first. The shared Slint shell draws the
triangle and exposes the track actions.

Native and WASM compilation passed. Native shared-component previews were
visually inspected at desktop and 360px widths, including rounded cover corners,
the supplied blue-cover reference, hidden marks and the icon variant. The preview
is `cargo run --locked --example song_layout_preview` with optional `--compact`,
`--reference`, `--hidden` or `--icon`; output stays under `target/song-layout/`.
Full application navigation, settings interaction and file-dialog/tag operations
were not exercised in this change; no automated tests were run.
