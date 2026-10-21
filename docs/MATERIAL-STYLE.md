# Vey tonal interface

The design is Material You inspired: a compact bento home, tonal surfaces,
soft rounded controls and a small number of organic artwork silhouettes.
The Vey plate logo uses a transparent background and rounded edges. See LOGO.md.
The dark palette starts from a muted
lime seed and follows the current cover when artwork becomes available.

The home has one main listening tile and two desktop shortcuts. Mobile keeps
one main tile and the existing navigation. Lyrics retain their open surface;
the cover and metadata sit directly on the player surface, without a backing
card. Loaded images have no colored placeholder below them. Selected tabs use filled
pills, and playback buttons change between circular and rounded-square shapes.

`src/material.rs` samples a bounded number of pixels on artwork changes,
picks a dominant hue and produces dark tonal roles. Neutral artwork produces a
neutral dynamic palette; transparent artwork uses the lime seed. While artwork is loading, the previous palette
stays in place. Palette extraction does not run on playback ticks or frames.
This is a cover-derived palette, independent of operating-system wallpaper.

Version 0.1.9 adds persistent Dynamic, Static and Background-only modes under
Settings → Вигляд. Palettes fade over one second rather than jumping; six fixed
hues support static accents. Reduced motion applies changes immediately. See
[appearance behavior](APPEARANCE.md) for mode boundaries and animation details.

The native lyric backdrop receives the same surface color as Slint and uses
the same 28% ambient texture. Bright covers are dimmed without adding a color tint.
The camera and mounted pages remain intact.
Tonal sliders retain the standard widget's pointer, keyboard and accessibility
behavior beneath their custom appearance. Standard form controls use dark mode.

Validation evidence is in `material-check.json`, the `material::` unit tests,
and screenshots 41–44. The browser test changes warm/cool artwork, delays the
second cover, navigates between pages, checks real audio volume through pointer
and keyboard input, and captures desktop/mobile views.

Native renderer startup retry feedback stays on the lyric surface instead of
leaving implementation diagnostics in a persistent home-page banner.

Version 0.1.3 keeps primary/selected emphasis while secondary icon actions use
transparent backgrounds. Redundant sidebar/home captions and the routine lyric
provider footer are removed. Missing-art placeholders and errors remain visible.
