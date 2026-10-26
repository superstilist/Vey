# Braccato integration

Native Vey uses the copied Rust Braccato Slint component in
`third-party/braccato-rust/`, through `src/native_lyrics.rs` and
`ui/native-lyrics.slint`. Browser/WASM retains the original upstream iframe.
Windows lyrics no longer create a WebView2 child window.

Read [native integration](../../docs/NATIVE-BRACCATO.md) and
[Rust snapshot provenance](../../third-party/BRACCATO-RUST-UPSTREAM.md).
The Rust snapshot comes from the user-specified local working tree at
`C:/Users/superstilist/3D Objects/test/braccato-upstream`; its uncommitted
changes and source hashes are recorded, and that checkout is unchanged.
The older JavaScript source snapshot remains in third-party/braccato/.

Canonical parsing/storage/provider behavior remains in this module and
src/lyrics.rs. The native adapter preserves actual word/syllable, backing and
romanization cues, display-only translation filters, offsets and source-index
seek mapping. Line-only input does not gain synthetic word starts. Rust
Braccato supplies native wrapping, sweep gradients, cached glow, letter waves,
wobble, stage roles and instrumental masks. Arbitrary browser CSS/DOM plugins
remain outside the native renderer's API.

The existing engine.rs remains Vey's sampled playback clock, interval index
and manual/passive scroll policy. Browser fallback rendering is in
src/renderer.rs and ui/braccato-lyrics.slint; the browser normally uses the
iframe. All native targets share the copied Slint renderer.

Run cargo test --locked --lib --tests and
cargo run --locked --example native_lyrics. Inspect actual native screenshots
under target/native-lyrics-check. Device acceptance is separate from compile
checks and Windows native visual fixtures.