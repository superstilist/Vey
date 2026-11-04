# 06 — Тексти пісень

Поточний стан: широкий parser/rendering pipeline уже існує, з line/word/syllable timing, translations, romanization, backing vocals та кількома форматами. Provider dialect coverage і точність на кожній платформі не повні.

## Ввід і зберігання

- [ ] Перевірити plain text, LRC, enhanced LRC, TTML, QRC, SRT, VTT, JSON/envelopes, fenced/quoted input і malformed documents.
- [ ] Перевірити синхронізацію line/word/syllable та не створювати штучних timestamps, яких немає у джерелі.
- [ ] Перевірити backing vocals, overlap, translations, romanization, instrumental gaps, offsets і source choice.
- [ ] Невалідний новий текст не стирає останню валідну версію.
- [ ] Provider alternatives мають зрозумілі attribution, sync classification і явне збереження вибору користувача.
- [ ] Перевірити lyrics cache caps, stale entry eviction і matching після змін track identity.

## Відображення

- [ ] Перевірити line/word/syllable fill, seeking, scroll follow, wrapped lines, backing layers і translations.
- [ ] Перевірити reduced motion, короткі cues, довгі gaps, rapid seek, track change і paused/resume.
- [ ] Desktop Rust Braccato, browser JS widget, Android native та web iframe мають окремі fixtures.
- [ ] Native fonts: kerning/ligatures/diacritics/Cyrillic/JP, 1x/1.25x, imported fonts і software/GPU.
- [ ] Arbitrary browser CSS/RICS не видавати за native capability; документувати supported subset.

## Розвиток

- [ ] Додати fixtures для provider dialects, що реально зустрілись у збоях, з upstream provenance.
- [ ] Перевірити переклад/romanization language selection, target=interface language та fallback.
- [ ] Поліпшити edit/paste/import UX із preview, parser error location та збереженням попереднього джерела.
- [ ] Перевірити auth challenge, expiry, retry та offline; JWT не потрапляє в render document чи logs.
- [ ] Додати доступний DOM/editor маршрут там, де canvas shell обмежує screen reader використання.

## Acceptance

- [ ] Regression parser fixtures і native/browser visual/timing fixtures пройшли окремо.
- [ ] Seek/retarget/reduced-motion сценарії не створюють скачків чи вигаданого word timing.
- [ ] Платформні неперевірені renderer/font межі лишаються явно позначеними.
