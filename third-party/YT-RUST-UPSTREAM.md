# yt-rust local source snapshot

Copied on 2026-10-21 from the user-supplied standalone project at
`C:/Users/superstilist/3D Objects/yt-rust` into `third-party/yt-rust/`.
The copy contains Cargo.toml, README, src, tests, examples and docs. It excludes
the original build output and lockfile. There is no dependency on the sibling
checkout at build or runtime.

The source manifest declares version 0.1.0, Rust 2021, and license
`MIT OR Apache-2.0`. The supplied source did not include separate license files;
the declaration is preserved in its copied manifest.

Original `src/providers/youtube_music.rs` SHA-256:
`666c042165c0e20172697432764f2ab930b8c095024946e441fe20d670dcc052`.
The sibling source is unchanged. Local changes in the copied module:

- Expose search and suggestion JSON decoders for an application-owned transport.
- Decode direct top-result cards and bare shelf continuation responses through
  the same search decoder.
- Extract linked artists and albums separately, prefer fixed-column duration,
  exclude greyed-out rows, and reject view counters as album metadata. Untyped playable rows do not imply music videos.
- Read both supported-config and watch-endpoint video-type hints.

Vey enables only the youtube-music feature, on native targets. It uses the
decoders with its existing public HTTP requests; it does not instantiate the
crate's session-configured provider or require a YouTube Data API key.
Browser search uses Vey's same-origin Rust server, so it shares the decoder
without compiling native HTTP/runtime dependencies into WASM.

See [integration notes](../docs/YT-RUST-INTEGRATION.md) for the adapter boundary
and verification. Future source updates must preserve the local decoder changes
or replace them with equivalent upstream behavior and run the adapter fixtures.
