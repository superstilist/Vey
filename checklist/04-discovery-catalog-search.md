# 04 — Пошук, каталог і відкриття музики

Поточний стан: YTM public search/suggestions/browse/radio і mixed-media Home є. Public artist cards відкривають пошук; повні сторінки артиста/альбому/playlist та продовження великих колекцій незавершені.

## Типи даних та ідентичність

- [ ] Створити типізовані сутності пісні/відео/артиста/альбому/playlist із provider, стабільним ID, title, art, доступністю й destination.
- [ ] Розрізняти provider ID, video ID, local path і display name; однакова назва не є доказом однакового треку.
- [ ] Додати match confidence та ручний remap для cross-provider match; не зливати live/remix/cover/explicit версії автоматично.
- [ ] Підтримати відсутні поля, unavailable item із реальним ID, explicit flag лише якщо сервіс повернув це поле.

## Search

- [ ] Локальні результати з'являються миттєво та не блокують public search.
- [ ] Типізовані вкладки/фільтри Songs, Videos, Artists, Albums, Playlists.
- [ ] Enter запускає негайний запит; debounce зберігається при наборі; cancel/generation відкидають старі результати.
- [ ] Підказки та історія запитів мають clear history і opt-out від локального збереження.
- [ ] Перевірити no query, one-character query, Cyrillic/Latin, діакритики, emoji, duplicate provider results, offline/429/5xx.
- [ ] Пошуковий результат кожного виду веде у правильну сторінку, а не перетворюється на випадковий трек.

## Home, Explore, recommendations

- [ ] Усі Home layouts показують коректні дії типізованих song/album/artist/playlist/video cards.
- [ ] Кеш каталогу має TTL, stale-while-revalidate і зберігає останній валідний Home при помилці.
- [ ] Перевірити shelf paging/filter/order, пустий стан, duplicate track на двох shelves та правильний queue context.
- [ ] Explore browse має типізовані categories, retry, back, continuation та скасування.
- [ ] Radio/recommendation explain reason, manual refresh, excluded tracks/artists, offline fallback.
- [ ] Немає передачі особистої history/likes у public catalog запит без прозорої необхідності й дозволу.

## Artist pages

- [ ] Probe YTM browse ID з Home та перевірити реальні response sections до контракту UI.
- [ ] Показати лише наявні секції: header/art, top tracks, albums/EP/singles, related artists, videos/description якщо повертаються.
- [ ] Кожна секція має окрему порожню/помилкову/частково-завантажену поведінку.
- [ ] Вибір артиста з Home, search, album та локальної медіатеки відкриває відповідну public/local сутність.
- [ ] Related artist navigation зберігає стек і scroll; rapid selection відкидає stale response.
- [ ] Немає вигаданих біографій, popularities, follower counts або provider IDs.

## Album/playlist pages і pagination

- [ ] Album page має verified header, artist link, release info якщо доступно, track list і Play all.
- [ ] Playlist page має owner/description/art/count за наявності, збереження порядку й повторів.
- [ ] Continuation cursors обмежені, дедупліковані й захищені від циклів; скасування запиту працює.
- [ ] Перевірити довгу колекцію, порожню, private/unavailable/deleted, змішане відео/аудіо, невідомий duration та unavailable row.
- [ ] Не замінювати кеш повною відповіддю часткової першої сторінки; retry продовжує з валідного cursor.
- [ ] Back повертає до того самого scroll position та не запускає непотрібне повторне завантаження.

## Acceptance

- [ ] Усі типи пошуку/карток мають fixture parser та UI navigation regression.
- [ ] Live probe перевіряє лише дозволену/власну сесію, зберігає лише scrubbed fixture та короткий підсумок.
- [ ] Desktop, web та Android проходять Home→artist→album→track→back; неперевірена платформа відкрита.
