# Per-song lyric choice and compact settings

The provider/variant selected in the player's source selector is remembered for
that song. Native state and browser local storage serialize `Store.lyrics_choices`.
Each choice records provider, format, synchronization type and a source-payload
fingerprint, rather than a transient list index. On cached or fresh results the
controller restores the exact variant, then the closest matching provider variant.
If unavailable, the first available result is shown without erasing the choice.

Known YTM download paths share the public song's choice key. Other tracks use
their stable track path. The default provider applies to songs without an
individual choice; requests for a remembered song use automatic source discovery
so a global provider filter cannot silently exclude its saved selection.

## Player → track menu → Lyrics

The shared `ui/lyrics-settings.slint` menu uses the app's tonal card, provider
selector and compact on/off rows. It is embedded in the player's scrollable track
menu (the … button), with no separate entry in global Settings. The overlay stays
within the player viewport; opening it does not move the cover or lyric surface.
The default source, reduced motion, duet stage and passive scrolling are under
**Advanced**; display toggles and translation language remain immediately visible.

- **Romanization** and **Translation** independently control display, default on.
- **Translation language** follows the interface by default, or can be selected
  independently (uk/en/cs/pl/de/es/fr/ja).
- Missing translations and romanization are enriched asynchronously through
  Unison, with Google Translate as fallback. Source translations explicitly
  tagged with the requested language and existing romanization take precedence.
- Toggles rebuild the display immediately. Enrichment requests are separate
  from lyric-provider requests; cache hits need no network. Disabling both
  layers prevents new enrichment requests.
- Cached alternatives, source fingerprints, clipboard/export data and timing
  remain complete. Filtering uses a display clone and leaves lead/backing cues
  unchanged; timed romanization is hidden together with its text.
- New imports are cached, selected and remembered. They invalidate pending lyric
  responses so an older request cannot overwrite the imported source. Browser
  responses also check a lyric generation after downloading their JSON body.

`src/services.rs` owns choice matching. `src/preferences.rs` applies/captures
`lyrics_romanization` and `lyrics_translation` for both controllers.
`src/braccato.rs::visible_layers` filters the display clone before both adapters.

## Automatic translation

src/lyrics_translation.rs follows the request contracts reviewed in
[Better Lyrics translation.ts](https://github.com/better-lyrics/better-lyrics/blob/c5dbb80c75c78ab72c590f569a2ef424dcadcfb3/src/modules/lyrics/translation.ts)
and [its endpoint constants](https://github.com/better-lyrics/better-lyrics/blob/c5dbb80c75c78ab72c590f569a2ef424dcadcfb3/src/core/constants.ts).
This is a Rust host implementation; vendored Braccato remains unchanged.

- Unison: POST https://unison.betterlyrics.org/translate with lines, to,
  optional from and videoId. Translation and romanization share the request.
- Google fallback uses translate.googleapis.com/translate_a/single with
  client=gtx, dt=t and, for romanization, dt=rm.
- Repeated chorus lines are deduplicated. Requests are chunked by encoded URL
  size; Latin/non-Latin translation groups are separated. A fallback batch is
  applied only if the number of returned lines matches the request.
- Native requests run on Tokio; the browser posts to /api/lyrics/enrich on its
  same-origin Vey backend. At most three enrichment jobs run concurrently;
  individual HTTP requests have a 20-second timeout and bounded input sizes.
- Store.lyrics_enrichment retains up to 128 request results. Keys include the
  source line text, target/source language, video ID and enabled layers, so
  source edits or language changes cannot reuse another translation.
- Responses are applied only when their key matches the current selection.
  Failed requests leave original lyrics visible. Refresh lyrics clears the
  current enrichment cache and allows another attempt.
- Generated layers exist only on the display clone. Original provider choices,
  source payloads, cue times, backing parts and clipboard/export data are intact.
  Generated romanization is line text; no word timings are fabricated.
- The menu shows loading/failure feedback. Translation requires internet access
  on a cache miss; availability depends on the external services.

## Evidence

Before relocation into the player menu, native shared-menu previews were inspected at desktop and 360px widths using
`cargo run --locked --example lyrics_settings_preview` (add `--compact`). Images
stay under `target/lyrics-settings/`. Native and WASM compilation passed for that earlier change.
Provider-network restoration, persistence across a real app restart and advanced
menu interaction were not exercised; no automated tests were run for this change.

Live service requests, restart restoration and playback interaction have not
been exercised for automatic translation. No automated tests were run.

Automatic translation: the native binary and shared-menu preview build passed.
The updated menu was visually inspected at desktop and 360px widths using the
existing lyrics_settings_preview example; output remains under target/. This
checks menu rendering only, not requests to either translation service.

Final WASM compilation passed after the controller/cache changes.

## Independent automatic romanization

Romanization works while Translation is disabled. Missing non-Latin lines first
use Unison, then Google dt=rm with a source language inferred independently from
metadata, detected language and script. Kana/Hangul identify Japanese/Korean;
Han-only lines use an applicable source/song hint, otherwise Chinese. Other
supported scripts use a language hint where ambiguous (for example ar/fa/ur).
Mixed-language lines are grouped by inferred source language before fallback.
Script inference is a fallback, not a general language detector.

Only changed Latin-script results are accepted. A batch must retain its line
count. Existing source romanization, including real timed romanization, wins.
Generated readings remain plain line text: original word/syllable cues are not
invented or changed. Romanization-enabled keys use v2 to bypass older empty
results. A romanization-only failure does not become a successful empty cache
entry; Refresh lyrics allows another attempt. The menu has dedicated
romanization loading/failure feedback and an independent persisted on/off switch.

Native/WASM compilation is checked for this addition; live service requests and
playback interactions are not exercised. No automated tests were run.

Romanization menu: the 360px native preview was inspected; both switches and
service hints fit without clipping. WASM compilation passed for the new shared
cache key and feedback paths. Script inference scans text in linear time.
