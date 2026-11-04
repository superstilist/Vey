# 11 — Spotify public playlist import → YTM

## Мета і межі

Вставлений публічний Spotify playlist стає звичайним Vey playlist із треками YouTube Music (`yt://VIDEO_ID`). Користувач не підключає Spotify account і не надає OAuth, Client ID, Client Secret чи cookies. Vey не читає приватні Spotify playlists і не відтворює Spotify audio.

Зовнішній потік:

```text
Spotify public playlist URL
  → strict host/ID validation
  → fixed unauthenticated Spotify embed page
  → ordered Spotify track metadata
  → exact YouTube Music match
  → yt:// video IDs
  → existing conflict-aware local playlist import
```

Відповідь public embed не має authoritative total count і наразі повертає до 100 рядків. Результат із 100 рядками відхиляється цілком, бо неможливо довести, що це повна playlist. Це навмисна перевірка цілісності, а не успішний імпорт 100 треків.

## Перевірені API та свідомі обмеження

- [x] Перевірити Spotify Development Mode для імпорту без підключення акаунта: Client Credentials — server-to-server і потребує Client Secret; за актуальною документацією Development Mode `GET /playlists/{id}/items` доступний для playlist, якими користувач володіє або спільно керує. Це не дає accountless доступу до довільних public playlists. Джерела: [Client Credentials](https://developer.spotify.com/documentation/web-api/tutorials/client-credentials-flow), [Dev Mode migration guide](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide).
- [x] Для імпорту довільних публічних посилань обрати embed page без акаунта й секретів; формат не є стабільним API-контрактом, тому невідома схема завершується помилкою.
- [x] Перевірити YouTube Data API: public search потребує API key; `search.list` має окрему денну квоту. Для YTM-конверсії наразі використовується song-only пошук YouTube Music, щоб отримати справжні YTM video IDs, а не довільні результати YouTube. Джерела: [YouTube API authorization](https://developers.google.com/youtube/data/guides/authentication), [quota costs](https://developers.google.com/youtube/data/determine_quota_cost).
- [ ] Якщо буде обрано тільки офіційні API, спершу спроєктувати серверне зберігання Spotify Client Secret і YouTube API key, квоти та покриття доступу; не вбудовувати секрети у Windows/Web/Android клієнти.

## Етап 0 — прибрати account/OAuth

- [x] Видалити Spotify з account picker, підключених identity, Client ID поля та OAuth UI.
- [x] Видалити Spotify OAuth, token refresh, `/v1/me`, private playlist sync і 15-хвилинний auto-pull.
- [x] Залишити окремі Vey cloud і YouTube Music account paths.
- [x] При startup мігрувати `music-accounts.json`, відкидаючи застарілий Spotify token record і зберігаючи YTM підключення.
- [x] Перенести лише валідний старий track-ID match cache у локальний public-ID cache.
- [x] Повторний імпорт зберігає єдиний unambiguous legacy Spotify playlist ID, щоб не створити дублікат.
- [ ] Fixture: старий JSON із двома акаунтами переписується без Spotify tokens і без втрати YTM account/auto-sync/baselines.
- [ ] Fixture: повторний імпорт оновлює єдину стару playlist; два legacy записи не обираються навмання.
- [ ] Fixture: read-only directory дає видимий migration error і не маскує збережені дані.

## Етап 1 — безпечне читання публічного списку

- [x] Приймати тільки `open.spotify.com/playlist/<id>` або Spotify playlist URI з перевіреним ID.
- [x] Формувати URL тільки на зашитому `https://open.spotify.com/embed/playlist/<id>` host.
- [x] Не посилати account headers, cookies, OAuth, credential query або redirect.
- [x] Обмежити запит таймаутом і HTML розміром; відобразити 403/404/429 та невідомий формат без body/log dump.
- [x] Перевірити root entity type/ID, playlist name, track rows, URI, track ID, title та artist.
- [x] Відхиляти 100-row boundary, unsupported episode/non-track і некоректний payload без Store write.
- [ ] Live acceptance: відомий public список із 1–99 треками; звірити назву й source row count.
- [ ] Live acceptance: 100 і більше рядків, private/deleted URL, 429/timeout та зміна HTML schema.

## Етап 2 — відповідники YouTube Music

- [x] Шукати кожну унікальну Spotify композицію в YTM з обмеженням до трьох одночасних запитів.
- [x] Приймати лише `song` із точним нормалізованим title, artist і duration delta `< 8s`.
- [x] Відхиляти відсутній або неоднозначний match і не зберігати playlist частково.
- [x] Обмежити пошук глобально трьома паралельними запитами та deadline 180 секунд.
- [x] Зберігати public source track ID → YTM video ID кеш локально, окремо від connection tokens.
- [x] Складати результат у початковому порядку та з повтореннями, кожен шлях — `yt://<11-char-id>`.
- [ ] Fixture: title punctuation/case, YouTube `- Topic` channel suffix, multi-artist title, duration boundary 7.99/8.00, zero duration.
- [ ] Fixture: duplicate Spotify source ID зберігається кілька разів у playlist, але шукається один раз.
- [ ] Fixture: match cache пошкоджений/завеликий — ігнорується; валідні джерела повторно шукаються.
- [ ] Live acceptance: прийняті відповідники прослуховуються як YTM audio та відкривають правильний track metadata.

## Етап 3 — застосувати імпорт без пошкодження бібліотеки

- [x] Використати існуючий `library_import::apply` і стабільний ID `spotify-<public-owner-hash>-<playlist-id>`.
- [x] Reuse лише один валідний account-scoped legacy ID за збіжним Spotify source ID; неоднозначні записи не перезаписувати.
- [x] Залишити локальний baseline conflict guard, межі Store і відмову при неповному імпорті.
- [x] Підключити один dispatcher до native UI, Android native імпорту та Web same-origin `/api/playlists/import`.
- [x] Зберегти YTM private-playlist URL fallback лише для існуючої явної YTM browser connection.
- [x] Прибрати тексти «підключіть Spotify», privacy consent і SDK/developer setup.
- [ ] Перевірити first import, repeat import, local edit conflict, renamed source playlist, empty playlist і duplicate rows.
- [ ] Перевірити private YTM URL fallback після login; Spotify приватні URL мають відхилятися без fallback.
- [ ] Перевірити cancel/navigation: stale result не має змінити playlist після виходу з імпорту.
- [ ] Перевірити, що web import вимагає Vey backend, а static hosting помилку показує зрозуміло.

## Етап 4 — migration та cleanup

- [x] Відділити старий Spotify account record від public playlist import.
- [x] При startup переписувати account file без access/refresh token, client ID/secret, owner і Spotify account label.
- [x] Документувати локальний `spotify-import-matches.json`; він містить лише public IDs і не синхронізується.
- [ ] Перевірити upgrade зі старого `music-accounts.json`, downgrade behavior і відновлення з backup.
- [ ] Перевірити відсутність `VEY_SPOTIFY_CLIENT_ID`, OAuth scope, redirect URI і Spotify account controls у source/release package.
- [ ] Не видаляти імпортовані Spotify playlists під час міграції: після імпорту це звичайна local playlist із YTM IDs.

## Етап 5 — перевірка середовищ

- [ ] Native Windows: публічний імпорт, черга, YTM playback, помилки мережі та cancel.
- [ ] Web: WASM UI + same-origin Vey backend, reload і conflict handling.
- [ ] Android: доступний пристрій/емулятор, інтернет-запит, long wait/cancel, queue/playback; окремо зафіксувати непроведені кейси.
- [ ] Перевірити помірну повну playlist на час імпорту й YTM rate-limit; помилка не змінює попередню playlist.
- [ ] У кожному звіті записати app/source version, platform, public/private mode, source row count і imported row count; не записувати personal playlists чи secrets.

## Залежності та acceptance

- Код: `src/spotify_import.rs`, `src/native.rs`, `src/music_accounts.rs`, `src/server.rs`, `src/home.rs`, `ui/main.slint`, `locales/catalog.tsv`.
- Документація: `docs/MUSIC-ACCOUNTS.md`, `checklist/05-integrations-accounts.md`, `checklist/README.md`.
- Spotify embed HTML не є стабільним публічним API-контрактом. При збої парсера імпорт має fail closed; не додавати auth/TOTP/cookie extraction як fallback.
- YTM пошук використовує недокументований YouTube Music endpoint; стежити за змінами відповіді та не підміняти його звичайним YouTube search без перевірки типу/метаданих треку.
- Feature acceptance complete лише після live public import, partial/error fixtures та окремих platform checks. Compile alone does not close these boxes.
