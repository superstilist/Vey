# Player and lyric themes

Vey 0.1.33 has 20 offline player/lyric presets. Open Appearance → Player themes,
or the player's lyric settings, then
choose **Lyrics appearance**, select a theme and press **Apply**. The chooser
shows a Braccato sample, a description and creator credit. A selected preset
is restored from its saved CSS when the app restarts.

| Group | Presets |
| --- | --- |
| Existing styles | Braccato, Soft glow, Crisp text |
| Vey originals | Aurora, Ember, Ocean, Rose Quartz, Neon Pulse, Nocturne |
| Better Lyrics adaptations | Minimal, Opacity Karaoke, Apple Music, Lucid, Spotlight, Sustain, Material 3 |
| New whole-player Vey styles | Graphite, Solar Dusk, Cobalt, Paper Night |

Aurora uses mint ink; Ember uses amber; Ocean uses blue and light letter waves;
Rose Quartz uses soft pink; Neon Pulse combines lilac glow with letter waves;
Nocturne uses quiet blue-gray text and restrained motion. Community adaptations
translate the source themes' lyric focus and typography into Vey's supported
controls. They do not install YouTube Music skins, browser shaders, remote
fonts, videos or images. See the [registry](https://github.com/better-lyrics/themes)
and [pinned creator/source references](../assets/lyric-themes/PROVENANCE.md).

Leave **Lyrics font** blank to use the preset font. An explicit font override
continues to apply across presets; desktop font import remains available.
**Custom CSS / RICS theme** opens the existing editor. Unknown or edited CSS is
shown as Custom and retained rather than overwritten with a preset.

The appearance panel has separate typography and effect controls. Size,
weight, line spacing, blur, glow radius, word motion, letter waves and fading
edit a draft. A small marked CSS block stores control overrides without
rewriting the original theme or its timing rules. Repeated slider changes
replace that block instead of appending unlimited rules. Apply validates and
saves; Cancel, Escape and clicking outside discard changes; Reset clears the
draft font override and selects Braccato. The footer stays outside the scroll
area, including on 360px layouts and when the virtual keyboard reduces space.

The sample has its own clock and fixed demonstration cues, independent of the
song. Native uses the same word-paint worker, shaped glyph planning, focus mask
worker and Slint renderer as playback. Paint is prepared when the draft changes,
not on every animation frame. Pause freezes the sample; reduced motion wins.
The browser sample shares the typed Slint appearance subset; its actual song
continues to use the upstream DOM/CSS/RICS renderer. Browser-only rules cannot
be represented exactly in this native-style preview.

Native conversion resolves bounded CSS var aliases/fallbacks, strips
`!important`, accepts short hex alpha and rejects non-finite colors. Inactive
selectors no longer match the active-color path. Conditional at-rules and
translation/romanization rules are reported rather than applied globally.
This remains a supported subset, not a browser CSS cascade implementation.
RICS syntax/variable errors preserve the last saved theme. Browser-only
directives remain available to the upstream browser compiler.

Presets do not change source cue times, offsets, provider choices or scroll
policy. They keep spatial fill for real timed words and do not create word
timing for line-only text. Reduced motion wins over preset letter waves and
glow. Vey's separate application palette and artwork background stay under
Appearance settings. Native-only motion controls retain their native scope;
this is not a promise of pixel parity with the original extension themes.

## Whole-player appearance (0.1.33)

**Theme the entire player** applies a profile to transport, menus, navigation,
queue, artwork corners, surfaces and all other pages as well as lyrics. It is
enabled by default; Braccato keeps the existing cover-derived appearance until
a visual player override is added. Turn it off and Apply to retain only lyric
appearance and restore the application's saved color mode/hue. Draft changes
do not repaint the live shell before Apply. Scope is saved in
`Settings.theme_entire_player`, including through backups/restoration.

Controls include accent swatches, corners, artwork corners, desktop artwork
share and ambient strength. Ambient strength multiplies the existing background
opacity; zero stops hidden background preparation. They use one managed CSS
block and never add cue/scroll offsets.

Community themes are native ports of supported appearance roles. The Material 3
palette was evaluated from its pinned CSS in dark mode with source #d1bcff.
Apple Music uses source accent #fa586a, a black base, 12px artwork corners and a
local system-font fallback. Original YouTube DOM layouts, remote fonts,
extension scripts and custom fragment shaders do not execute in Slint. The
browser lyric iframe retains upstream CSS/RICS support; the application canvas
uses the same typed profiles as desktop. These ports do not reproduce every
pixel of the original YouTube Music document.

Browser camera presentation uses bundled GSAP 3.15.0 and CustomEase. One timeline
follows the host's endpoints, easing and elapsed time; repeated samples retain
it. Interruption/reduced motion cancel it and restore host-owned styles. Native
camera and lyric clocks remain Rust/Slint. Theme text cannot execute arbitrary
JavaScript. Build the shared motion ESM and widget with `npm run build` before
generating WASM bindings; no CDN is needed.

Additional native color forms include HSL/HSLA, Display-P3 and OKLCH, converted
to finite sRGB output with gamut clipping. Basic body/html and YouTube player
container color/background/font-family rules map to player roles. Conditional
and complex selectors continue to be reported.

| Whole-player CSS property | Supported value |
| --- | --- |
| `--vey-accent`, `--vey-background`, `--vey-surface`, `--vey-text`, `--vey-muted` | Supported CSS color |
| `--vey-font-family` | One installed family, up to 128 bytes |
| `--vey-corner-radius`, `--vey-artwork-radius` | 0–40px |
| `--vey-control-radius` | 0–999px |
| `--vey-artwork-share` | 0.32–0.50, desktop layout |
| `--vey-ambient-strength` | 0–1.5, relative background opacity |
| `--vey-theme-duration` | 0–2000ms, palette changes |

The GSAP source is `tools/braccato-widget/player-motion-source.js`;
`web/player-motion.js` is its generated ESM. The widget build includes the
dependency's license in THIRD-PARTY-LICENSES and records its version/hash in
the manifest. `src/player_themes.rs` resolves profiles; `src/material.rs`
retains one palette transition and yields to the saved artwork mode on opt-out.

## Source and checks

`src/theme_presets.rs` owns the shared catalog and preview models. New CSS lives
in `assets/lyric-themes/`; the three existing CSS strings retain their exact
identity. `src/themes.rs::apply_settings` populates the catalog for native and
WASM controllers. Settings continue to use `lyrics_theme_css` and the existing
font override; no new storage schema is needed. The picker is
`ui/lyric-theme-picker.slint`, mounted by the player appearance dialog.
`ui/lyric-appearance.slint` owns the responsive panel and preview controls;
`src/lyric_appearance.rs` owns draft validation, bounded overrides and native
sample paint. Native and WASM controllers install the same draft callbacks.

```
cargo run --locked --example lyric_themes -- target/theme-preview-new
cargo run --locked --example lyric_themes -- target/theme-mobile-new --mobile
cargo run --locked --example lyric_appearance -- target/appearance-desktop-new
cargo run --locked --example lyric_appearance -- target/appearance-phone-new --mobile --uk
cargo run --locked --example player_themes -- target/player-themes-new
cd tools/braccato-widget
npm run test:theme-presets -- ../../target/theme-preview-new/catalog.json ../../target/theme-browser-new
```

Use a new output directory each time. The browser fixture needs the static
fixture server on 127.0.0.1:4181 (`npm run serve`). It blocks external requests.

Windows GPU desktop and 360px mobile captures verify all 16 selections, saved
CSS restoration and custom CSS. Unit checks compile every preset without
unsupported rules, compare timing/follow settings with the unchanged baseline,
and retain explicit font overrides. Browser checks cover all 16 profiles,
colors/fonts, real word cue boundaries, reduced-motion overrides and reset to
the default palette. The existing 100-steady-update custom-theme test passes.
New descriptions/labels have eight-language catalog entries.
The native fixture serializes/restores Settings in memory and exercises the
selection/apply callbacks without reading user state. Software rendering also
passes all 16 selections, custom restoration and reset to Braccato. The widget
bundle, WASM compile check and browser UI build/bindings succeed.
The optimized Windows build also passes all 16 selections, custom restoration
and default reset before packaging.

The 0.1.31 library executable reported 194 passed, 7 ignored and the pre-existing
locale-catalog failure (one field where eight are required). The 22 new theme
translation rows have eight fields and unique keys; the unrelated earlier
catalog defect was preserved. Android/Linux device behavior is not newly
validated by these Windows and browser fixtures.

## Appearance panel checks (0.1.32, 2026-10-29)

`examples/lyric_appearance.rs` uses isolated in-memory settings and actual
native pointer, wheel and Escape input. Desktop GPU, 360px Ukrainian GPU and
software rendering pass: real prepared glow/glyphs, independent preview,
draft isolation, saved preset restoration on Cancel, Apply, font override,
reset, invalid RICS retention, body scrolling and fixed footer. The Ukrainian
sample now selects the same font fallback as production before creating its
blur mask, retaining the whole line.

The headless Edge fixture `test:lyric-appearance` passes real app navigation,
combo keyboard selection, draft isolation, Cancel, Apply/persistence and
desktop/360px scrolling/footer captures without browser errors. It uses the
WASM app, fixture audio/lyrics and isolated browser storage; no user state or
external service is read. Its screenshots go only to the requested new folder.

The library run has 199 passed, 7 ignored and the existing locale-catalog
failure caused by a blank line at row 273. The new conversion/compiler/editor
checks pass, covering inactive colors, alias/fallback/important handling,
conditional/secondary rule isolation, finite colors, comments/quoted dollars,
scan limits and bounded control overrides preserving cue offsets. New UI
labels have all eight catalog fields. The WASM UI and bindings build succeeds.
All 16 presets also pass the current native and browser fixtures, including
custom CSS restoration, word boundaries, colors/fonts and reduced motion. The
browser custom-theme fixture retains the same compiled theme for 100 steady
updates.
The optimized Windows build and the same input/preview fixture linked against
its release library also pass.

## Whole-player validation (0.1.33, 2026-10-29)

The whole-player fixture passes all 20 profiles on Windows GPU desktop, 360px
mobile and software rendering, including real cached artwork background, shell
colors/shapes, independent drafts, custom accent/corners, scope persistence and
restoring artwork colors. It captures Home, Queue and Appearance as well as the
player. All 20 legacy lyric selections also preserve saved/custom CSS and cue
time. Native palette/shape assertions run after each profile settles.

Headless Edge passes the shared editor with actual pointer/keyboard input,
custom accent verified against rendered transport pixels, opt-out restoring
the prior palette, desktop/360px body scrolling and fixed actions. GSAP tests
cover exact midpoint/end transforms, joining elapsed camera time, duplicate
samples, interruption, style restoration, full-scene blur and reduced motion.
WASM navigation passes diagonal camera/retarget and desktop/mobile clipping.
All 20 browser lyric presets keep word boundaries, colors/fonts and reduced
motion; 100 steady updates reuse one custom theme.

The library suite has 202 passed and 7 ignored when explicitly excluding the
existing catalog test. That test was also run separately and still fails only
on the pre-existing blank row 273. New locale rows have eight fields and no
duplicate keys. Formatting checks pass. Linux/Android device acceptance and
literal execution of original browser skins/shaders are not established here.
The optimized Windows build and all 20 whole-player cases linked to its release
library pass. The portable ZIP was checked against the fresh executable hash,
0.1.33 release notes and bundled GSAP license.
