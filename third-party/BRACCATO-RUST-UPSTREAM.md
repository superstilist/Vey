# Rust Braccato source snapshot

Copied on 2026-10-25 from the user-requested local working tree:
`C:/Users/superstilist/3D Objects/test/braccato-upstream`.
Git HEAD: `12b51af762fefb92f399a2041f899a11188ffe74`.
Repository: https://github.com/better-lyrics/braccato (MIT).

This is a **local Rust-port snapshot with uncommitted changes**, not a claim
that this exact code was published upstream. Original checkout is untouched.
`braccato-rust/source-sha256.json` records SHA-256 hashes of the copied crate
files before the host adaptations below. The MIT license is retained.

Copied runtime crates: types, engine, parsers, providers, highlight, themes,
slint. The application depends on braccato-slint and its runtime dependencies;
it keeps Vey's existing canonical parser/provider/storage contracts.
The demo executable, generated target directories, JavaScript packages and
demo media are not required. Three small RICS/CSS fixtures used by the theme
crate's tests are retained under crates/demo/assets/themes/ and demo/.
The original demo/src/glyph_plans.rs is retained as the reference for Vey's
adapted glyph-plan cache.
`Cargo.upstream.toml` retains the source workspace
manifest; `Cargo.toml` omits the uncopied demo member.
The runtime workspace's generated Cargo.lock supports reproducible standalone
crate tests; the Vey application continues to use its root Cargo.lock.

Host adaptations (all other copied crate source is unchanged):

- `crates/slint/ui/lyrics.slint`: namespace the Slint `LyricRow` struct as
  `BraccatoRow` to avoid a Slint 1.18 generated-export collision with Vey's
  canonical `LyricRow`. Expose a measured viewport callback so retained native
  Flickable layout resolves after resize even with a paused playback clock;
  Vey observes those metrics instead of depending on deferred painting.
  Retain an observed scroll target and explicitly apply it when follow resumes:
  native pointer input replaces Flickable's initial content-y binding.
  Align blurred glyph masks to the same scale/origin as their sharp text during
  focus crossfades. Vey's focus worker and typography bindings live outside the
  copied component.
  Keep overlapping active vocals sharp even when their source differs from the
  current scroll anchor.
  Add optional `glow-levels` atlas metadata and crossfade cached radii through
  the highlight sweep. Retain whole-word width and normalized glyph positions
  when asynchronous letter plans arrive.
  Clip cached glow at the actual fill edge so unsung letters cannot receive it.
  Normal fill mixes base/highlight in one Text brush. Only viewport-adjacent
  scrolling bodies and visible stage roles construct word views. Sharp and blur
  paint share one row transform. Optional `external-focus-mix` consumes the
  host's retained `BraccatoRow.focus-mix` progress; Vey updates only the bounded
  nearby band (180ms in, 280ms out). Settled rows keep one body. Optional smooth
  highlight fade has flat endpoints; scroll easing profile 7 adds a soft start
  and stop. Fixed row geometry and post-layout follow restore
  preserve reflow/resume behavior with lazy bodies. Opt-in letter effects retain
  a local cache only for scaled long cues.
  Project wrapped cue offset/span into both whole-run and letter fill/glow masks.
  The Vey host selects focus from actual sung cues independently of an optional
  advanced camera clock. Highlighting follows the unshifted word clock. Scroll planning and
  grouping live in Vey's `src/braccato/engine.rs` and `src/native_lyric_follow.rs`.
- `crates/slint/src/lib.rs`: retain the Rust `LyricRow` API as a type alias;
  initialize optional glow atlas/focus progress metadata to zero and full cue
  spans for existing callers.
- `crates/slint/src/layout.rs`: keep whitespace-separated pieces of one timed
  cue in a single shaped run until a real wrap. Wrapped fragments retain measured
  cue offset/span so the fill continues across rows; source timings/indices remain
  unchanged. Separately timed words remain separate cues.
  Prefer whitespace/hyphen boundaries, keep a word's separately timed syllables
  together and use grapheme-safe emergency wrapping for oversized tokens. Vey's
  follow lookup projects the cue's fill into each continuation without modifying
  its canonical timing. The host enables a gentle sweep wave and smooth scrolling.
  `finish-fill-at-cue-end` is an opt-in host property: Vey normalizes the fill
  feather to finish inside each cue/fragment, gates continuation ink/glow until
  the previous fragment completes, and removes the wave at those boundaries.
  Native stage/focus selection uses actual cues in the Vey adapter.
  Optional `settled-word-motion` bounds word/letter motion to supplied cue or
  fragment intervals, uses a smooth attack/return and attenuates short cues.
  `LyricWord.motion-start-ms`, `motion-duration-ms` and `motion-origin-em` are
  layout-only host metadata: Vey groups timed syllables under one transform
  origin without changing text or fill clocks. Cached glyph bodies exist only
  during an optional letter wave; word wobble and letter waves are independent.
  Vey opts into `cue-bounded-glow`, a smooth attack/release inside each supplied
  cue/fragment. Solid atlas tint and disjoint source-texel feather bands follow
  the ink ramp without recoloring the full texture each tick. Empty and
  completed ink use solid brushes. With `finish-fill-at-cue-end`, letter masks
  project the whole-run spatial ramp into each measured glyph, including its
  one-pixel feather, instead of mixing grapheme-count and measured-width clocks.
- `crates/slint/src/glow.rs`: apply the requested variable-font weight when
  measuring mask bounds and sweep glyphs, matching the native text font instance.
- Vey's `src/native_lyric_glyphs.rs` is based on demo/src/glyph_plans.rs, with
  Vey's generated glyph type and bundled Roboto test font. The original source
  license applies. Integration lives in src/native_lyrics.rs and ui/native-lyrics.slint.

The previous JavaScript snapshot in `third-party/braccato/` is preserved for
the browser iframe. WebView2 is still used for Windows video/authentication,
but it is no longer the native lyric renderer.

The Rust demo's Satoshi 500/700 and Noto Sans fonts were copied into assets/fonts
for reference typography. FFL.txt and NotoSans-OFL.txt retain their font licenses.
