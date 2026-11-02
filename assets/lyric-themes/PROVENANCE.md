# Lyric preset provenance

Vey 0.1.31 contains 16 offline appearance presets: three existing styles,
six original Vey designs, and seven native adaptations of community themes.
The rendering engine remains Braccato. These are authored configurations for
Vey's supported lyric controls, not redistributed copies of upstream CSS/RICS,
fonts, artwork, videos, shaders, or YouTube Music interface modifications.

The community registry was inspected at
[better-lyrics/themes](https://github.com/better-lyrics/themes).
Its `index.lock.json` blob was `33fbbf534db3661d2b0e03c8ecc2a0a87e6e8537`,
updated 2026-10-28. Each reference below is pinned to the registry's source
commit; creator names come from that commit's `metadata.json`.

| Vey preset | Creator | Upstream version | Source commit |
| --- | --- | --- | --- |
| Minimal | semicolonhope | 3.0.5 | [2c47d051](https://github.com/ramansg/Minimal/tree/2c47d0512b39bc806a05716e5abdc8a7d73fb7f5) |
| Opacity Karaoke | semicolonhope | 3.1.29 | [f5b7148e](https://github.com/ramansg/Max-Performance-BlyricsTheme/tree/f5b7148ef45150e8afed4c6ccb298c52ced5701d) |
| Apple Music | tposejank | 1.0.3 | [64191f7f](https://github.com/tposejank/blyrics-am-theme/tree/64191f7f096cc5e05e19b36de5c69780ff536c30) |
| Lucid | drago-oo | 1.3.0 | [193735d5](https://github.com/drago-oo/lucid-theme/tree/193735d5f5de36ea5110417b243e46967f754964) |
| Spotlight | boidushya | 1.0.0 | [5ba22d84](https://github.com/better-lyrics/theme-spotlight/tree/5ba22d84039aba95d00426114fea8077a63eaf03) |
| Sustain | boidushya | 1.0.28 | [503f7cbf](https://github.com/better-lyrics/theme-sustain/tree/503f7cbf1f8c40af5db20970d07cdca233dda4eb) |
| Material 3 | adidotzip | 1.1.2 | [27624e01](https://github.com/adidotzip/Material3/tree/27624e01549beaf2f7747d3e3a44b1126e8eab6d) |

Aurora, Ember, Ocean, Rose Quartz, Neon Pulse and Nocturne are original Vey
palettes and motion profiles. Their CSS files live beside this document.

Adaptations preserve the source theme's broad lyric focus/typography intent
within Vey's existing controls. Bundled Satoshi/Roboto replace remote fonts.
Material 3 uses a fixed lilac palette; Vey's separate artwork background and
application palette remain under Appearance settings. Minimal and Opacity
Karaoke retain source-timed spatial fill while disabling costly decorative
motion. Lucid/Apple Music do not install their browser shaders or player skins.
Spotlight applies inactive-line blur without hiding future row layout. Sustain
uses the existing bounded letter wave and light font weight.

Presets contain no timing, scroll-offset, seek, provider, or network overrides.
Native-only controls such as `--vey-word-wobble-strength` use the native
renderer; browser rendering retains its own corresponding motion behavior.
Theme CSS is persisted in the existing settings field. Unknown/custom CSS is
preserved as Custom rather than replaced with a catalog entry. A separate
font override remains in effect until the user clears it.

## Whole-player profiles (0.1.33)

Graphite, Solar Dusk, Cobalt and Paper Night are additional original Vey styles,
with explicit --vey-* surface, geometry and ambient roles. Existing preset
strings stay unchanged so saved selections keep their identity.

Profiles live in src/player_themes.rs. Material 3's roles were evaluated from
the pinned style.css in Chromium dark mode with --m3v-source:#d1bcff. Results:
primary #79d7c2, on-primary #03372e, primary-container #005144,
secondary-container #603b50, tertiary-container #5c3a65, surface #161219,
surface-container #231e26, high #2e2830, highest #39333b, on-surface #eadfea,
on-surface-variant #d0c2d3. This is a color-role reference, not execution of the
original CSS inside Slint.

Apple Music's RICS supplies accent #fa586a, black background, 12px artwork
corners and a 55% lyric-panel share. Vey ports those roles with Segoe UI as a
local interface-font fallback and preserves the existing lyric preset/font
override. Other community profiles retain their adaptation status. Remote
fonts, DOM icon replacements and fragment shaders are not bundled or executed.
