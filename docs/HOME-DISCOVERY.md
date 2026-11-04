# Home discovery shelves

Home uses the shared `ui/home-feed.slint` surface on native and WASM. It combines
local listening/playlists, the private recommendation snapshot, and a public
YouTube Music catalog cache. Cards retain their actual type: square songs,
albums and playlists, circular artists, and 16:9 music videos. Shelves support
dragging, horizontal paging, type filters and a playable queue. Artwork updates
individual rows through the existing bounded artwork/cache services.

`src/discovery.rs` owns portable catalog data, decoding, composition and the UI
adapter. `src/native.rs::ytm_home_catalog` fetches public Home metadata; when no
video shelf is available, one videos-filtered query uses the recent artist,
a public catalog artist, or a general music-video query on a cold start.
The browser calls the same service through `/api/home`. Public catalog results
are bounded to eight sections with ten cards per section. The cached catalog
stays in the local Store and refreshes hourly while Home is open. A failed
refresh preserves the cache. There is one in-flight catalog request; no request
blocks the UI thread. Artwork reuses the existing bounded loading and cache
services; browser image decoding still runs on the WASM thread.

Songs select a shelf queue. Videos select their real video ID, enable clip mode
and open Now Playing. Actions retain the shelf identity when the same track
also occurs on Listen again or recommendations. Albums open their track
collection; artists open search.
Public playlist collections reuse the complete playlist importer, preserving
order and intentional repeats. New public song/video metadata enters the
existing catalog-import path. Personal listening history and profile signals
are not sent as shared catalog records.

The six existing Home choices now control density and ordering: standard,
compact, quick picks first, discovery first, playlists first and artists first.
Custom player colors remain available. The default desktop Home and player dock
use open, flat surfaces; the mobile dock retains its safe-area layout.

Verification commands (use fresh output directories):

```powershell
cargo test --locked --lib
cargo check --locked --lib --target wasm32-unknown-unknown
cargo run --locked --example home_library -- target/home-feed-native-new
cargo run --locked --example home_library -- target/home-feed-phone-new --mobile
./scripts/build-web.ps1 -Debug
# In tools/braccato-widget, with npm run serve on port 4181:
npm run test:home -- ../../target/home-feed-native-new ../../target/home-feed-browser-new
```

An optional public-service probe writes only public catalog metadata:
`cargo run --locked --example home_library -- target/home-feed-live-new --live`.
Fixture screenshots establish layout and input behavior, not audible/device
playback or accessibility support beyond the current Slint shell.

Verification on Windows, 2026-11-03:

- `cargo test --locked --lib`: 246 passed, 7 intentionally ignored integration
  tests. The catalog parser, local album fallback and empty-feed regressions pass.
- `cargo build --locked --bin vey --example home_library` and the WASM build plus
  bindings pass. Native captures of all six layouts and the updated picker were
  inspected at 1440x900 and 360x780 (`target/home-feed-native-ready-*`).
- Headless Edge checks cover cached startup, card kinds, paging, album and artist
  actions, playlist queue repeats, repeated track identity across shelves,
  explicit refresh and failed-refresh cache retention, plus 320px/390px and
  landscape layouts (`tools/braccato-widget/home-feed-test.mjs`). Video fixtures
  verify identity and clip-mode handoff; they do not establish video decoding.
- A live native `/api/home` probe returned bounded song, playlist and video
  sections; `/api/browse` opened a real public playlist with 45 tracks. A browser
  public-catalog preview loaded 39 real covers without failed requests
  (`target/home-feed-public-preview-01`). These counts describe that probe only.
- New Home and picker labels are present in all eight locale columns. No audible
  playback, physical Android device, Linux or macOS runtime acceptance is claimed.
