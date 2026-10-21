# Braccato integration and Rust fallback

The implementation now follows the upstream Better Lyrics Braccato repository,
not the earlier local demo adapter. Reference revision:
`1e6b6954b1c18136274681a9bf4985efb2332b01` (MIT).
Source: https://github.com/better-lyrics/braccato
License: `third-party/BRACCATO-LICENSE`.

Windows and browser builds now use the **original upstream renderer**, not an
approximation of its effects in Slint. `third-party/braccato/` contains the
complete, unmodified Git source archive: all six packages, styles, tests, demo
and tooling (163 files). `docs/braccato-source-check.json` records the byte-for-
byte verification against the pinned upstream checkout.

Braccato is a lyrics-rendering library, not a standalone music-player shell.
PlayerVF keeps its own player navigation and transport controls, and mounts the
unmodified upstream `<braccato-lyrics>` element for the lyrics surface. The
upstream highlighted editor is used for pasted lyrics. The built-in PlayerVF,
Classic, Karaoke and Contrast choices now map to upstream-compatible CSS themes;
Classic no longer turns off timed word highlighting.

`tools/braccato-widget/` builds all published runtime exports of core/element,
constants/text/themeSettings/util, parsers/format, highlight, provider-blyrics
and RICS into local assets. Types are included as upstream source. The native
Windows executable embeds those assets and serves them through a private
WebView2 protocol; the WASM player loads the same widget in a same-origin iframe.
Rendering needs no CDN. The installed WebView2/browser supplies CSS and Web
Animations support exactly as it does for upstream.

The host connects Rust playback, rate, pause, seek, offsets, visibility, document
changes, stage layout and geometry to the component. Upstream injection helpers
render translations and timed romanization. The paste dialog uses upstream's
incremental highlighted textarea editor. CSS themes use the full upstream RICS
compiler and original styles, including custom image paint, glow, letter waves,
word wobble, instrumental ripples and scroll animations.

Playback, authenticated requests, canonical parsing and storage remain Rust.
No JWT or account credential is passed into the rendering widget. Playback
samples do not rebuild the DOM or recompile themes; frame interpolation stops
while paused/hidden. The native widget is disposed before the Slint window.
Word and syllable highlighting follows the explicit part timestamps supplied by
the lyric source. Adjacent syllables keep their text boundaries and separate
clocks through the shared Rust adapter. See [syllable support](../../docs/SYLLABLE-LYRICS.md).
Line-synced lyrics still scroll by line because they contain no reliable part
timestamps to animate.

## Rust parser and fallback implementation

The following modules remain available to Rust callers and serve platforms
without the web widget, or Windows systems where WebView2 cannot initialize.

| Upstream capability | Native implementation |
| --- | --- |
| Shared lyric/word models | `types.rs`, lossless canonical models in `../lyrics.rs` |
| TTML/Apple/AMLL, LRC, QRC, SRT, plain | `parsers.rs`, `ttml.rs`, `../lyrics.rs` |
| Metadata/agents/repeated keys/translation dialects | `ttml.rs` |
| Romanization/timed romanization/background/explicit flags | parser models, `../renderer.rs`, shared Slint view |
| Credits and instrumental normalization | `credits.rs`, `instrumental.rs` |
| Upcoming/active/past words, overlap selection | `engine.rs` |
| Playback-rate clock, pause/seek, visibility | `engine.rs`, `../renderer.rs` |
| Scroll grouping, measured active band, user pause/resume | `engine.rs`, view geometry callbacks |
| Plain-lyric passive scrolling | `PassiveScroll`; opt-in, does not invent cue timestamps |
| Scroll/stage layouts | stage planning in Rust, `ui/braccato-lyrics.slint` |
| Reduced motion and theme behavior settings | `settings.rs`, `../themes.rs` |
| Provider priority, validation, cancellation and bounded song cache | `providers.rs`; live HTTP adapters in `../lyrics_service.rs` |
| Lossless source highlighting tokens | `highlight.rs` |
| Theme preprocessing | `rics.rs`: bounded variables, nested selectors, parent selectors, media/supports/layer/container rules |

The **fallback** theme backend applies supported color, font, scroll duration, playback
offsets, passive scrolling, reduced motion and rich-sync behavior values. Other
settings remain available through the Rust settings API. Arbitrary browser CSS
effects and external RICS language
extensions are not a browser engine: unsupported rules/directives produce
explicit diagnostics. Browser custom-element lifecycle, DOM class selectors,
Web Animations objects, extension image replacement hooks and framework/npm
packaging are host integrations. These fallback limitations do not apply to
the upstream WebView2/iframe renderer.

## Compatibility and validation

The checked-in oracle corpus was produced by compiling and executing the real
upstream parser at the pinned revision, outside the app. Tests require Rust
only. It includes 22 TTML and 47 line-format inputs; 68 compare directly after
normalizing formatting-only XML whitespace. The remaining QRC input has an
upstream text-loss bug: the Rust implementation intentionally keeps the untimed
words and has a separate regression test for it.

Other deliberate native corrections:

- Invalid/non-finite timing is rejected instead of propagating NaN.
- Repeated LRC timestamps repeat the lyric; vocalist-labelled simultaneous
  lines remain separate instead of becoming translations.
- Integer LRC offsets are milliseconds; decimal-exporter offsets are seconds.
  Both are subtracted, including the document end, as upstream's timing expects.
- Invalid/incomplete XML is rejected rather than rendering partial markup.
- Instrumental gaps use the maximum covered end, so an overlapping held note
  cannot create a false silent gap.
- Provider caches include track identity; a hit for one song cannot serve another.

Run `cargo test --lib --tests`, `cargo run -- --screenshots`, and
`cargo run --example lyrics_bench`. The benchmark writes
`docs/braccato-performance.json`; these are debug microbenchmarks, not a claim
about end-to-end frame rate. Dormant styled text is compiled once, active-line
lookup uses a prefix-end interval index, paused/unchanged rows are not rewritten,
geometry is measured only on layout changes, and hidden views stop frame work.

## Original-widget validation

Build: `cd tools/braccato-widget; npm ci; npm run build`. Generated assets are
checked in so ordinary `cargo build` does not need Node.js.

Run `npm run serve` there to serve `web/` at `http://127.0.0.1:4181`, then run
`npm test` in another terminal in the same directory.
`npm run test:app` exercises the complete Rust/WASM player. Tests use a separate
headless Edge profile and fixture audio, not the user's browser session.
`cargo run -- --widget-smoke` exercises the native widget against fixture songs,
stage, instrumental, desktop/mobile sizes and editing. Its diagnostics are
written to `braccato-widget-smoke.json` under `PLAYERVF_DATA_DIR` (or the normal
application data directory). Slint's own screenshot API excludes child native
windows; widget visuals are captured separately by the browser checks.
