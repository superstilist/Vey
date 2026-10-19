# Vey — Rust + Slint

<img src="assets/vey-logo.png" alt="Vey" width="200">

Vey is a native music player with synchronized lyrics. Formerly PlayerVF;
existing library data and settings remain compatible.

The interface pairs a compact bento home with soft, organic shapes and
Material You inspired tonal colors that adapt to the current cover.
See [the tonal interface notes](docs/MATERIAL-STYLE.md).

Built in Rust.
The application shell is Slint. Lyrics use the original Braccato web component
in an embedded Windows WebView2 widget and in a browser iframe. There is no
Tauri dependency. The original player project remains untouched.

**Full parity is not finished.** See [the acceptance and parity report](docs/PARITY.md)
for working features, platform evidence, and remaining work.

```powershell
cargo run --bin vey
cargo test
cargo test --test native_services native_audio -- --ignored
cargo run -- --screenshots
```

The app scans the system Music directory plus configured folders and files.
State lives in `%APPDATA%\PlayerVF-Rust\state.json` on Windows,
`$XDG_DATA_HOME/playervf-rust` (or `~/.local/share/playervf-rust`) on Linux, and the
app's internal data directory on Android. `PLAYERVF_DATA_DIR` overrides the location.
Unreadable state is copied to a timestamped backup before recovery.

## Responsiveness

Pages stay mounted for fast returns and use one shared, interruptible camera.
The lyrics host follows that camera on the compositor. Background results
are processed in bounded batches; a dedicated writer coalesces settings saves
and flushes the latest state on normal shutdown. Existing data paths and
internal bridge names remain compatible with PlayerVF.

See [page motion](docs/PAGE-MOTION.md), [validation](docs/VEY-VERIFICATION.md),
and [Windows packaging](docs/RELEASE.md).

## Web

```powershell
./scripts/build-web.ps1 -Debug
cargo run -- --web-server
```

Open `http://127.0.0.1:4173`. The interface and controllers compile to Rust/WASM.
The JavaScript bridge handles browser file selection, exports, the Turnstile
iframe and the original Braccato rendering widget. Browser audio is controlled
from Rust through `web-sys`.
The same-origin Rust server supplies public YTM search, browse, radio, cached
streaming with range requests, and lyric-provider access. A static deployment
can use local files; public YTM requires this backend because browsers cannot
send the upstream native request headers.

The Slint application shell renders to a canvas and has **no screen-reader
support on web**. The Braccato lyric surface and highlighted editor use DOM.
See the [Slint web documentation](https://docs.slint.dev/latest/docs/slint/guide/platforms/web/).
Browser files must be selected again after a reload; the file identities used
by likes and local playlists remain stable when the same files are reselected.

## Data migration

1. Open the original app's DevTools and run `scripts/export-local-data.js`.
2. In the Rust app, open **Налаштування → Перенести дані** and choose the exported JSON.
3. Rescan the library, then check the imported likes, history, sources and lyric offsets.

Migration is one-time and allowlisted. It excludes account data, Google cookies,
JWTs, and Turnstile tokens. Existing lyrics are reparsed with the Rust parsers.
State backups include local settings, likes, playlists, history, offsets, native
lyric caches and imported theme data. Tag backups are stored separately under
`tag-backups/` before applying a reviewed YT metadata match.

The Windows challenge window hosts only the lyric provider challenge. Its IPC
passes just the Turnstile token to Rust, which exchanges and caches the JWT in a
separate `lyrics-auth.json`; that file is never part of data migration or backups.
Google sign-in opens the system browser and has no account callback or session APIs.

## Source map

- `src/native.rs`: adapted original audio, scanning, public YTM and caption services.
- `src/services.rs`: typed portable store, queue, migration and source normalization.
- `src/app.rs`: native callbacks, background work, stale-result protection and player polling.
- `src/lyrics.rs`, `src/renderer.rs`: parser models and native styled-text word rendering.
- `src/lyrics_widget.rs`: embedded upstream renderer, geometry and playback bridge.
- `third-party/braccato/`: complete unmodified upstream source at the pinned revision.
- `tools/braccato-widget/`: reproducible bundle and browser integration checks.
- `src/lyrics_service.rs`: provider fallback, cache and JWT lifecycle.
- `src/local_sync.rs`: metadata writes with tag backups and restore.
- `src/themes.rs`: supported CSS properties and explicit unsupported-rule reporting.
- `src/web.rs`, `web/bridge.js`, `src/server.rs`: WASM controller and host bridges.
- `ui/`: responsive Slint screens and shared controls.
- `assets/design-tokens.json`: copied original visual contract; `ui/tokens.slint` is its native mapping.
- `docs/reference/`, `docs/screenshots/`: original and native fixture screenshots.

Roboto and Lucide assets retain their upstream licenses in `assets/`.

## Player and lyrics

Braccato is the only lyric theme, using the upstream package's colors, glow
and word effects. Legacy theme names and custom CSS are ignored on load and
restore. The theme selector and CSS import are removed. Desktop uses Braccato's
3rem type size, with a compact cap on mobile. Reduced motion remains available.
Windows and web show a Braccato loading/retry state if the widget is unavailable,
instead of silently substituting the native fallback. Windows retries failed
startup automatically; the retry button recreates the host and resends lyrics.

The widget smooths small audio-clock corrections while applying seeks, pause
and speed changes immediately. It holds bounded interpolation across late
samples and keeps the current lyrics if a malformed document arrives.
The widget reuses playback options, ignores duplicate paused samples, and
pauses its lyric animations while hidden or editing. During playback the
animation frame loop owns rendering; host clock updates only resynchronize it.
The browser host updates iframe geometry only when bounds or viewport change,
sends lyric documents only when changed, and suspends rendering messages while
the tab is hidden. Unsent artwork and document updates are retained for resume.
`docs/braccato-widget-check.json` records the regression checks and the repeated
sample test; these checks measure avoided work, not overall application FPS.

The home and player pages use locally bundled `@kawarp/core` 1.3.1 for an
album-art-driven fluid background. The existing WebView2/iframe bridge renders
a 256×144 decorative texture at about 30 fps, while Slint draws the controls
and contrast scrim. Motion freezes on pause and respects reduced motion; other
pages and minimized windows do not request frames. Platforms without the web
widget, or without WebGL, use a static gradient. No CDN is required.
On Windows the lyrics child is opaque and paints a crop of that same Kawarp
frame and contrast scrim. This avoids stale-window artifacts from transparent
WebView2/OpenGL composition. Its crop follows resize and display scaling;
unchanged frames reuse the cached background and hidden lyrics skip painting.
Only one background request is in flight; slower rendering reduces the frame
rate instead of accumulating work. A lost response permits retry after 500ms.
Run `npm run test:ambient` in `tools/braccato-widget` with the fixture server
running (`npm run serve`) to check GPU output, motion and artwork updates.

The player uses an open, borderless layout, neutral controls, a red progress
accent, and plain underlined tabs. Lyrics follow the active line with an eased
scroll, use their measured row heights for wrapping, and fade words in as they
start. Plain text remains freely scrollable.

`lyrics::parse_payload` is the shared Rust equivalent of the original project's
Braccato/Better Lyrics parsing pipeline in `src/lyrics/pasteLyrics.ts` and
`src/lyrics/dacubeking/dacubekingV2.ts`. Desktop paste, browser paste, migration,
and provider responses use the same format detection and synchronization
classification. It accepts LRC/enhanced LRC, TTML, QQ QRC/XML, SRT, WebVTT, plain text,
and the nested JSON lyric envelopes returned by providers. QRC word times are
absolute song times; enhanced LRC end markers preserve gaps between lines.
Translations and backing vocals remain separate from lead words. Invalid XML
is rejected so another provider can supply usable lyrics.

Windows and browser builds mount the original upstream `<braccato-lyrics>`
element for the lyric surface, with word-by-word timing, translations,
romanization, backing vocals, instrumental effects and upstream themes.
`ui/braccato-lyrics.slint` is the native fallback when the web renderer is
unavailable. Source provenance and platform details are in
`src/braccato/README.md`.

Authentication matches the WebStorm app: `/verify-turnstile` takes JSON
`{token}` and accepts either `jwt` or `token` in its response. Lyrics use
`POST /v2/lyrics` with all seven form fields, including `videoId`,
`alwaysFetchMetadata=false`, and `token`, with the YouTube Music Origin and
Referer. A shared cookie-aware client handles both requests. Cached JWT expiry
is decoded from the token, and a valid token remains usable during its last
hour while renewal runs in the background. A rejected token triggers one
renewal and retry. Settings → **Вставити JWT** accepts a JWT, a Bearer-prefixed
JWT, a JSON token response, or a Turnstile token in a masked input. Successful
verification refreshes the current lyrics; use **Перевірка Turnstile** when an
interactive challenge is required.

JWT acquisition starts automatically at launch, as in the WebStorm project's
`prewarmAuthenticationToken()`. Missing/expired tokens use the hidden official
challenge and are exchanged and cached automatically. Concurrent callers share
one refresh, including failures; failed attempts have a 30-second cooldown.
The helper drains and flushes its IPC output while running, so long challenge
tokens cannot block the process. Challenge errors reach the status display
without logging credentials. `vey --lyrics-auth-check` checks this flow
without opening the player and prints only whether authentication succeeded.
The navigation filter allows WebView2's `data:text/html;charset=utf-8;base64,`
representation only when it matches the exact bundled challenge host. Rejecting
that navigation previously prevented the iframe from ever starting.


The default Windows and browser renderer now executes the original upstream
Braccato code, including letter waves, glow, word wobble, background vocals,
instrumental SVG effects, overlapping scroll animations, stage layout and the
full RICS compiler. All six upstream packages and their public runtime exports
are bundled locally. The full copied source has 163 unmodified upstream files.
The lyric paste dialog uses upstream's highlighted editor. See
[the integration notes](src/braccato/README.md) and
[source provenance](third-party/BRACCATO-UPSTREAM.md).
Use the lyric tools drawer to switch between scrolling and **Сцена**, choose
reduced motion, or enable **Автопрокрутка** for plain lyrics. These preferences
are saved. The widget uses the browser animation frame clock while playing,
interpolates native playback samples and stops its frame loop when hidden or
paused. Playback updates do not rebuild lyrics or recompile themes. The Rust
styled-text renderer remains a fallback when WebView2 cannot initialize and on
other native platforms; its theme/effect subset does not limit the web widget.

Provider requests now run concurrently and reuse HTTP connection pools. Old
automatic parser caches are refreshed for the new metadata model; previously
pasted custom lyrics remain available. `cargo run --example lyrics_bench`
regenerates the timing report in `docs/braccato-performance.json`.
