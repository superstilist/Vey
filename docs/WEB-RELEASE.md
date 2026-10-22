# Vey 0.1.16 — Web

This archive contains the optimized WebAssembly application, JavaScript bindings,
original Braccato widget, styles and logos. Extract it before use. Serve `web/`
over HTTP(S); opening index.html directly with file:// is not supported.

## Full online features

The public YouTube Music catalog, streaming, lyric providers, translation and
romanization require the Vey same-origin backend. Use the matching Windows or
Linux release binary from the same GitHub release.

- Windows: place vey.exe beside start-web.ps1, then run that script. Alternatively
  pass `-Binary` with the executable's full path.
- Linux: install the Vey .deb and run `sh start-web.sh`. For a portable binary,
  run `VEY_BINARY=/full/path/to/vey sh start-web.sh`.

Open http://127.0.0.1:4173. Stop the foreground server with Ctrl+C. The backend
binds only to loopback; use an HTTPS reverse proxy for remote hosting. This
archive does not deploy a public website or include operating-system binaries.

## Static hosting

For browser-local files and manually imported lyrics, any static HTTP server can
serve the web directory with application/wasm for .wasm files. For example:

```sh
python3 -m http.server 4173 --directory web
```

Static hosting alone does not provide the /api endpoints listed above. Browser
file URLs are session-scoped; reselect local files after a browser restart.
Browser/OS audio codec support varies. The Slint canvas shell has no web screen
reader support; the lyric widget uses DOM rendering.

## Packaging scope

The same web assets run in supported desktop/mobile browsers, including on
macOS; this is not a native macOS application. Compilation and archive checks do
not establish browser playback, GPU/device compatibility or live service access.
Native desktop and Android packages are separate release downloads.

Rebuild with `scripts/package-web.ps1`. Source modules remain in the repository;
the archive contains generated runtime assets and third-party notices.
