# Player, languages and Android refinement

The interface language is selected under Settings → Language. Ukrainian remains
the default; English, Czech, Polish, German, Spanish, French and Japanese are
available. App copy lives in `locales/catalog.tsv`; the native and web controllers
share it, including parameterized status messages. Track names and lyric content
remain source text. Japanese uses the bundled OFL-licensed Noto Sans JP font.

Settings → Appearance also contains background opacity, warp, blur, saturation,
dithering, bright-cover dimming and reset. Defaults match the supplied reference:
0.28, 1.00, 15, 1.50, 0.020 and dimming enabled. Preferences persist in native
state, browser storage and JSON backups; older backups receive these defaults.
Windows/web retain Kawarp. Linux/Android have a bounded native blur/warp backdrop
using the same controls; it is a native fallback rather than the Kawarp shader.

`ui/motion.slint` supplies semantic control, panel, sheet, palette and navigation
timings. Reduced Motion propagates to those controls and finite palette changes.
Navigation retains its shared interruptible camera and the mobile player sheet.
Landscape reserves less space for navigation and header chrome; the content and
forms remain scrollable within the system insets and keyboard-aware dialog bounds.

## Video surface

Video mode replaces the cover-art slot with muted video. The song audio remains
the sole audible source and owns position, pause/play and playback rate. Lyrics,
queue and controls continue using that song clock. Failed/missing/finished clips
reveal the cover; they do not interrupt the audio or invent a replacement clip.
Native requests discard stale results and cancel the preceding resolver task.
Media views pause when hidden; removing the source releases media resources.

- Browser: HTML video surface positioned over the Slint artwork bounds.
- Windows: child WebView2 surface, using the same video implementation.
- Android: JNI-loaded platform bridge, TextureView and muted MediaPlayer.
- Linux: FFmpeg frame decoder into the shared Slint image slot. `ffmpeg` must be
  available on PATH; without it, cover mode remains available. FFmpeg is an
  optional external executable and is not bundled by this project.

The browser backend exposes `POST /api/video` with the existing Track payload;
the response is a nullable `{url,mime}`. It uses the existing public video
resolver and matching service. Upstream stream availability is still independent
of successful search or a fixture playback pass.

## Android lyrics authentication

The Android bridge hosts the same bundled official challenge document used on
Windows. It exchanges challenge tokens through the existing Rust service;
Java never persists JWTs. Refresh coordination, expiry validation, the one-time
401 retry, response parsing and provider fallbacks remain shared Rust behavior.
JWT input normalization accepts direct tokens, Bearer values and JSON exchange
envelopes. Credentials stay in the separate auth cache and outside backups.

`build.rs` compiles `android/java/app/vey/VeyBridge.java` with the installed JDK
and Android SDK and embeds the resulting DEX in Android builds. API 26's
InMemoryDexClassLoader loads it; no extra activity or Gradle dependency is needed.

## Validation

Repeatable checks include `cargo test --locked --lib --tests`, the native Android
inset example, and the widget scripts `test:video`, `test:sync`, `test:clock`,
`test:ambient`, `test:transitions` and `test:refinement`. The refinement check uses
isolated browser state and fixtures and captures all routes at four sizes and
all eight languages. `test:video` generates its small MP4 fixture with FFmpeg.

Browser fixture checks do not establish Android device playback, Android
Turnstile acquisition, Linux runtime behavior or live public stream availability.
Record actual target checks in the task report; do not infer them from builds.
