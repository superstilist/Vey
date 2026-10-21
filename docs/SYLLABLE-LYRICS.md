# Syllable timing in Vey

Vey accepts real timed syllables from TTML, enhanced LRC, QRC and Braccato JSON,
including lyrics inside Better Lyrics SSE and Unison response envelopes.
Syllables use the existing RichSync representation and stored lyric-part fields;
there is no new backup format or guessed timing for plain/line-synced lyrics.

## Upstream review — 2026-10-21

Reviewed the requested repositories and the relevant data path:

- [Better Lyrics lyric modules](https://github.com/better-lyrics/better-lyrics/tree/master/src/modules/lyrics):
  [TTML source handling](https://github.com/better-lyrics/better-lyrics/blob/master/src/modules/lyrics/providers/ttmlSource.ts)
  selects rich or line rendering from parsed timing. Its
  [Unison adapter](https://github.com/better-lyrics/better-lyrics/blob/master/src/modules/lyrics/providers/unison.ts)
  reads data.lyrics and distinguishes TTML, LRC and plain payloads. Unified
  provider timingType is quality metadata, not a substitute for real cue times.
- [Braccato shared types](https://github.com/better-lyrics/braccato/blob/master/packages/types/src/index.ts)
  use LyricPart for words and syllables with startTimeMs, durationMs and words.
  The renderer has richsync/synced/none modes; provider syllable/word quality is
  a separate concept. Vey's already vendored renderer supports adjacent timed
  syllables grouped into a single visual word, including background parts.
- [Unison text extraction](https://github.com/better-lyrics/unison/blob/master/src/utils/extract-text.ts)
  preserves adjacent TTML spans and collects timed leaf spans as syllables.
  Its editing/diff Syllable shape is not a new playback payload to request.

The fetched source-file blob IDs were respectively
9c990695bc7f4ebe7c5008c4b3c399a6a8119cf5 (TTML source),
cff3b540da4217d0b26c5b58931c69b5543c5aa4 (Unison adapter),
f63aa8fb32ce0d563308b3cb19c470d83e1de8b5 (Braccato types), and
a75c5e9e3fc65f98340cb5161a62a8489a38ae52 (Unison extraction).
The implementation changes Vey's own Rust classification and display adapters.
The pinned upstream source under third-party/braccato remains unchanged;
no extension account, editing or submission code was copied into Vey.

## Behavior

Previously Vey required one timed part per whitespace-delimited word. Valid
Latin parts such as Sin + gle failed that condition, were classified as line
sync and lost their display timing even though Braccato could render them.

The shared parser now recognizes a syllable partition when nonempty parts have
explicit starts, progress in source order with at least two distinct starts,
and reconstruct the line text with its original word boundaries. Separate
untimed whitespace between TTML spans is allowed. Multiword phrases, incomplete
partitions, missing starts, equal starts and reversed syllable starts do not
qualify. Existing word and CJK timing behavior is retained.

Display conversion keeps each accepted cue's start and duration. Lead and
backing-vocal timing are validated independently; an untimed backing phrase
does not acquire estimated word timing. Timed romanization uses the same
part-level check. Native fallback rows receive these same parts; Windows/web
pass them to the unchanged Braccato renderer as richsync. Existing timestamps,
language, translations, offsets, vocalist metadata and reduced motion continue
through the existing pipeline. Missing syllable durations are not interpolated.

The alternative selector prefers verified syllable timing over word/line/plain
alternatives. A provider's claim of syllable/richsync quality does not promote
a payload that lacks valid cues. Cached alternatives are re-evaluated for
display, so cached syllables retaining explicit starts can render without a
schema migration.
Enhanced-LRC copy/export retains syllable starts and text boundaries.

## Validation

The new tests/syllable_lyrics.rs fixtures cover all four formats, Unison and SSE
wrappers, false timing claims, fixed source clocks, background syllables,
romanization, Cyrillic/Arabic/Japanese text, native model rows and LRC round trip.
The existing fixture that intentionally downgraded Sin + gle is updated to
require RichSync while retaining the zero-origin line-sync check.

From the root, cargo test --locked --lib --tests runs the Rust suite. From
tools/braccato-widget, npm run test:syllables generates a document using Vey's
actual Rust parser and tests it in the bundled renderer. It checks DOM clocks,
word grouping without inserted spaces, backing vocals, scroll/stage views,
seek, pause, reduced motion and a phone capture. It uses the static fixture
server started with npm run serve. The generated JSON and screenshot live under
ignored target/; they are test output rather than user lyrics.

The regular sync, clock and recovery suites remain relevant. Native build,
WASM build and browser fixtures establish their respective integration layers;
physical Android/Linux timing/rendering acceptance remains separate. Full
upstream browser effects are available only through the Windows/web renderer.

### Recorded results — 2026-10-21

- Rust suite: 190 passed, seven environment-dependent checks ignored, including
  seven new syllable integration tests and the revised JSON regression.
- Native debug build, WASM library check and rebuilt WASM/bindings passed.
- Widget bundle rebuilt. Browser test:syllables, test:sync, test:clock and
  test:recovery passed in headless Edge. The syllable suite checks actual
  Rust-generated documents, both layouts, lead/backing clocks, seeks and reduced
  motion. Manually inspected its 390x844 capture: the active syllable is lit,
  the rest remains dim, adjacent parts form one word, and backing text is visible.
- Changed Rust formatting checks passed. The pinned third-party/braccato source
  has no diff. Temporary static server stopped after verification; generated
  JSON and capture remain under ignored target output.

No live provider catalog completeness or Android/Linux device run is established
by these fixtures. The feature works with syllable timestamps actually supplied
by a provider or import; it cannot produce syllables for a line-only source.
