# Vey tonal interface

The interface uses an open dark canvas, restrained near-neutral surfaces and
one artwork-derived accent. Search fields, the mobile dock and modal surfaces
keep a background for clear interaction boundaries. Page canvases, home shortcuts,
settings sections and album cards have no decorative backing cards. Settings
icons and mobile navigation use color and text emphasis without nested pills.
Artwork placeholders share one neutral tone; covers use consistent rounded
corners. Hover feedback, selected controls and the primary transport remain clear.
The Vey plate logo uses a transparent background and rounded edges. See LOGO.md.

`src/material.rs` samples a bounded number of pixels on artwork changes,
picks a dominant hue for accents and produces near-neutral dark surface and text roles. Neutral artwork produces a
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

Phone search refinement (2026-10-22): the mobile header uses one full-width
search field with a touch submit icon, a persistent 44 px clear-action slot and
16 px input text. Desktop branding stays in the desktop header. Mobile song
rows are 60 px tall with 44 px artwork/actions and artist-only secondary text.
Search pages use tighter headings/insets. The web controller also selects the
compact layout below 500 px viewport height; the compact header aligns with the
existing 52 px content offset. Android safe-area handling remains shared.

The mobile bottom dock is one tonal surface containing playback and navigation.
A thin internal divider separates the two interaction rows; do not restore
separate rounded backgrounds or an exterior gap between them. Reserve dock
height through the shared viewport properties in ui/main.slint.

Background motion adds a slow rotation to the warped artwork texture, with an
overscanned crop that fills portrait and landscape corners. The same bounded
256x144 texture is reused; no artwork decoding or blur is performed per frame.
Pause freezes motion; reduced motion applies a stable unrotated frame. Warp
intensity zero also disables rotation. Android/Linux use bilinear sampling of
the prepared native texture; browser/Windows retain the real Kawarp renderer.

Mobile shared actions default to 44 px touch targets, including the dock's
transport and next control. Filter rows accommodate the same button height.
Pressed feedback is transient, keeping the open canvas and restrained palette.

Background presets provide Calm (lower opacity, warp and saturation) and Expressive
(stronger motion) starting points. All values remain editable and persist through
the existing appearance preferences. Native texture updates use a 33 ms interval.
