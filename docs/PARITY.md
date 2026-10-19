# Port status and acceptance

This is a working rewrite in progress, not a claim of complete feature or pixel parity.
The TypeScript/Tauri source project was read as the reference and was not deleted.

| Area | Status |
|---|---|
| Native shell | Home, Explore, Library, Liked, Search, local Playlist, Settings, Now Playing, queue; Ukrainian labels, original neutral/orange palette, bundled Roboto and Lucide icons |
| Responsive layouts | Desktop, collapsed sidebar, tablet, compact mobile navigation; native fixture screenshot capture provided |
| Audio | Original threaded Rodio decoder including Opus; play/pause/seek/rate/volume, queue next/previous, repeat, shuffle, automatic advance, load errors |
| Library | Original recursive folder/file scan, metadata, artwork cache, deduplication and unavailable-source reporting |
| YTM | Original public search, clip pairing, suggestions service, browse, recommendations, stream resolution, ranged download/cache and file downloads |
| Local data | Likes, playlist create/delete/add/remove, history, sources, settings, JSON backup/restore and one-time allowlisted migration |
| Lyrics | LRC, enhanced timing, QRC, TTML, SRT/WebVTT, nested background vocals, translations, malformed-provider fallback, SSE alternatives, Unison, captions and LRCLib |
| Lyrics renderer | Original upstream Braccato source and effects in embedded Windows WebView2 and browser iframe; Rust styled-text fallback on other native platforms |
| Themes | Original CSS and full upstream RICS compiler in the web widget; original highlighted paste editor; Rust fallback supports a smaller theme subset |
| JWT | Automatic startup prewarm, hidden official challenge, cached JWT, background renewal in the last hour, invalidation and one retry after 401; live Windows acquisition verified |
| Account boundary | Voice and Google session/account modules excluded. No embedded Google login, header import, cookie capture or account playlist/liked APIs |
| Web | Rust/WASM Slint canvas, browser audio, local file picker, durable local state, public network functions through same-origin Rust backend |

## Remaining parity work

- Live public streaming reached the upstream media service but both tested tracks failed with HTTP 403 during ranged download. Search is verified; end-to-end public audio playback is not. The browser reports the playback failure.
- Inline video clip rendering is not implemented. The cover/clip preference is retained and the UI reports the missing renderer. Public clip lookup and URL resolution services are ported.
- Android compiles as a native-activity library, but SAF file dialogs, storage permissions, browser intents and on-device packaging/runtime checks remain. Manual app-accessible source paths are available. Desktop file dialogs do not work on Android.
- The embedded challenge host is implemented for Windows and web. Linux/Android still need a platform challenge host.
- Community CSS themes are executed by the upstream widget on Windows/browser. An exhaustive visual audit of every external theme and preset remains. The Rust fallback still uses converted properties.
- Public album/artist/playlist detail navigation, continuation UX, all original context-menu/gesture shortcuts, download progress/cancellation and exhaustive settings parity remain.
- Browser metadata/artwork extraction from selected file bytes and complete web artwork parity remain. Browser file handles need reselection after page reload.
- The migration adapter preserves original supported preference records, but not every old setting has a matching native control. It does not read the old WebView profile automatically.
- Lyrics support timed translations and nested TTML backing vocals; not every provider-specific translation/romanization dialect is covered.
- Home shelves use local history or the library; original account-personalized home content is intentionally unavailable. Public Explore and radio are supported.

## Verification evidence

Recorded during implementation on Windows; see adjacent build/test logs in the workspace.

- Rust unit/fixture tests passed; final counts are recorded in `VERIFICATION.md`.
- Native audio-device smoke passed with a silent generated WAV: load, play, pause, seek, rate, volume, stop, and failed-load recovery.
- Live public YTM search returned songs/videos and paired real clips.
- Windows native screenshots generated at 1440×900, 900×900 and 390×844; reference data is fixed and separate from user state.
- Native UI was inspected through Windows UI Automation; the Library callback was exercised. Accessibility actions were subsequently added to custom controls.
- Rust/WASM compiled and bindings generated. The browser rendered the UI, accepted search input, returned 15 public tracks, and updated the player on selection. Browser checks found and fixed relative API URLs, an unsupported WASM clock, initial sizing, and a false startup error. Media download then failed upstream with HTTP 403; see `screenshots/28-wasm-stream-error.png`.
- Android aarch64 library check and link succeeded with NDK 28.2.13676358; `adb devices` returned no devices. No Android runtime pass is claimed.
- Linux runtime/build was blocked: the installed Ubuntu WSL distribution failed to attach its missing `ext4.vhdx`. Linux CI is supplied but has not run here.
- Automatic Windows acquisition was verified against the official challenge and exchange service with a fresh auth cache (about three seconds, no manual paste). Contract/error handling is also covered by local HTTP fixtures.
- All 163 vendored Braccato source files match upstream commit `1e6b6954b1c18136274681a9bf4985efb2332b01` byte for byte. Browser checks verify original letter spans, Web Animations, stage duets, instrumental SVG, seeking, RICS, reduced motion, highlighted editing and mobile resizing. Native WebView2 diagnostics confirm rendering, stage and editor geometry; see the Braccato integration notes.

The screenshot comparison is a visual reference, not an automated assertion of pixel identity.

Reference caveat: the original file named `10-nowplaying-lyrics.png` actually shows Home. Its comparison row is not evidence of a matched before/after Now Playing state.
