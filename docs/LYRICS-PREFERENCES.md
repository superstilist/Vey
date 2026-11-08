# Per-song lyric choice and compact settings

Lyric appearance now includes 16 offline presets with a color/type preview and
custom CSS editor. See [theme choices and creator references](LYRIC-THEMES.md).

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
selector and compact on/off rows. It is available from global Settings → Lyrics
and the player's scrollable track menu (the … button). The default source,
reduced motion, duet stage and passive scrolling are under **Advanced**; display
toggles, romanization spacing and translation language remain immediately visible.

The current Better Lyrics source uses `https://api.betterlyrics.org/getLyrics`
with song, artist, album, duration and an optional YouTube ID. It runs alongside
Unison, LRCLIB and the legacy Dacubeking source in automatic mode. Select
**Better Lyrics** under Advanced to explicitly request it and bypass older lyric
cache and local sidecars. On a missing TTML cache entry it also tries the QQ and
Kugou endpoints, preserving their word/line timing. Cache hits do not require a
key; uncached requests may return 401 when the service requires an API key.
The legacy Turnstile JWT is not sent to this API. See the [API reference](https://docs.betterlyrics.org/reference/get-lyrics)
and [authentication policy](https://docs.betterlyrics.org/docs/authentication).

Vey also supports a direct KuGou lookup based on the public protocol used by
[Metrolist](https://github.com/MetrolistGroup/Metrolist/blob/main/app/src/main/kotlin/com/metrolist/kugou/KuGou.kt).
Automatic mode matches title and artist and, when known, duration within eight
seconds before fetching LRC. Existing line/plain cache results appear immediately;
the app tries KuGou once per song per session and adds a result in the background.
User-supplied lyrics and already word/syllable-timed caches are not supplemented.
KuGou LRC is line-synced and is not promoted to word timing.

Selecting **Dacubeking**, clicking **Refresh lyrics** in automatic mode, or
using the player Lyrics access action can open the official Turnstile challenge
when no valid JWT is saved. Windows Vey embeds it over the lyrics panel; the
system browser opens only if WebView initialization fails or the user chooses
the fallback after the page fails to load. Other native platforms still use
the system-browser flow. Vey exchanges the returned one-use token for a JWT,
stores it locally and retries that lyric request. Normal song startup never
opens a challenge, and duplicate refreshes reuse the in-flight challenge. A
Better Lyrics `401` is unrelated: that separate API requires its own cache/API
key and does not accept the Dacubeking JWT. Vey does not solve CAPTCHA or
manufacture a token. The browser edition shows the challenge in its in-page frame.

- **Romanization** and **Translation** independently control display, default on.
- **Spacing** adjusts the gap between a lyric and its romanization/translation
  from 0% to 150% of the lyric font size. The default is 65%; the setting is
  saved locally and shared by native and browser renderers.
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
