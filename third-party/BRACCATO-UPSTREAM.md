# Braccato upstream source

`braccato/` is the complete Git archive of
https://github.com/better-lyrics/braccato at
`1e6b6954b1c18136274681a9bf4985efb2332b01` (MIT).
All upstream files are unmodified, including all six packages, effects,
stylesheets, tests, demo and tooling. Git history and generated dependencies are
not part of the source archive.

`tools/braccato-widget/` contains PlayerVF's host adapter and reproducible build.
Run `npm ci` then `npm run build` there. The complete renderer and public runtime
exports of the other packages are bundled into `web/braccato/`; no CDN is needed
to run the renderer. These assets are also embedded in the Windows executable.
`web/braccato/LICENSES.txt` includes upstream, font and bundled runtime dependency
licenses and is also embedded in the executable.
