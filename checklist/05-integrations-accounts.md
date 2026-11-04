# 05 — Інтеграції та акаунти

Поточний Spotify scope: тільки імпорт публічних playlist URL, пошук відповідників у YouTube Music та збереження треків як `yt://VIDEO_ID`. Spotify account/OAuth, приватна бібліотека й Spotify audio playback не підтримуються.

## Інвентар і межі

- [x] Залишити Vey cloud account як окремий шлях синхронізації бібліотеки.
- [x] Залишити YouTube Music desktop browser connection для read-only імпорту приватних playlist.
- [x] Прибрати Spotify account connection, OAuth redirect/scopes, Client ID UI та автоматичний Spotify sync.
- [x] Залишити Spotify URL import без входу та без developer credentials.
- [ ] Свіжо перевірити приватні/видалені Spotify URL і HTTP 403/404/429; старі OAuth результати не вважати актуальними.
- [ ] Зафіксувати реальну доступність desktop, web і Android після platform acceptance.

## Spotify public playlist → YouTube Music

Повний етапний список: [11 — Spotify public import](11-spotify-public-import.md).

- [x] Валідувати лише playlist URL/URI з `open.spotify.com`; не робити fetch за довільним host.
- [x] Читати публічні embed-дані без cookies, OAuth чи Spotify account.
- [x] Зіставляти за нормалізованими title/artist і різницею тривалості менш ніж 8 секунд.
- [x] Відхиляти пропущені, неоднозначні, неподтримувані або потенційно обрізані списки цілком.
- [x] Зберігати збережені треки як `yt://<video-id>`, з порядком і повторами джерела.
- [x] Перенаправити desktop та web same-origin API import на спільний URL dispatcher.
- [ ] Свіжо пройти public playlist із 1–99 треками, дублікатами, no-match та ambiguous match.
- [ ] Перевірити 100-row boundary, cancel, timeout, private playlist та 429 без часткового запису.
- [ ] Перевірити імпорт/відтворення у Windows native, Web і Android на доступних середовищах.

## YouTube Music внутрішні запити

- [ ] Перевірити artist/album/playlist browse IDs на контрольованих public прикладах.
- [ ] Перевірити related artists, releases, top tracks, artwork і continuation renderer variants.
- [ ] Перевірити locale/region, unavailable/private/deleted rows, timeout, 429 та 5xx.
- [ ] Зберігати очищені parser fixtures без cookies, visitor tokens, request headers і playlist особистої бібліотеки.
- [ ] Unknown renderer має давати контрольовану помилку/часткову секцію без зависання сторінки.
- [ ] Залишати cache/fallback до останніх валідних даних та відкидати застарілі відповіді.

## YouTube Music account connection

- [x] Залишити звичайний Google account chooser у виділеному Chrome/Edge профілі.
- [x] Не читати повсякденний профіль браузера, не копіювати cookies, не вводити пароль у Vey.
- [x] Залишити pull read-only, cancel/generation guard та baseline для конфліктів локальних змін.
- [ ] Перевірити повторний login, account switch, sign-out, закриту вкладку, disconnect і restart.
- [ ] Окремо прийняти синхронізацію на Windows, web та Android; web/Android external connect наразі відсутній.

## Безпека, зберігання та відновлення

- [x] Зберігати YTM browser connection окремо від Store backup і cloud sync.
- [x] На старті переписати застарілий `music-accounts.json` без Spotify OAuth поля; показати помилку, якщо запис не вдався.
- [x] Перенести валідні старі match IDs і не дублювати єдину стару імпортовану Spotify playlist.
- [x] Зберігати Spotify↔YTM match cache як локальні public IDs без токенів.
- [x] Задати fixed host, redirect refusal, timeout, response byte cap, concurrency cap і size limit.
- [ ] Додати/пройти fixture acceptance для застарілого account file та відмови у записі міграції.
- [ ] Переконатися, що logs/UI не показують повні provider payloads, access tokens чи підписані media URLs.
- [ ] Перевірити, що disconnect/restore не видаляє імпортовані playlists, likes, history чи локальні файли.

## Готовність

- [ ] Усі імпортовані Spotify rows мають валідний `yt://` ID, source order і повтори збережені.
- [ ] Будь-який no-match/ambiguous/unsupported row повертає зрозумілу помилку, playlist лишається без змін.
- [ ] Відповідь embed із 100 рядками не зберігається, доки джерело не надає перевірений total/pagination.
- [ ] Desktop і web використовують той самий імпортер; Android acceptance має власний запис і не виводиться з browser tests.
- [ ] У docs, UI, settings і release notes ніде не обіцяється Spotify account connect або Spotify playback.
