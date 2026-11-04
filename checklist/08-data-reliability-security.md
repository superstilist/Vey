# 08 — Дані, надійність і безпека

## Store, identity та міграції

- [ ] Store schema має backwards-compatible defaults та явний migration version при несумісній зміні.
- [ ] Canonical identities стабільні між local track, YTM ID, download, likes, history, lyrics і playlists.
- [ ] Перевірити rename/move/relink/duplicate/remote-deletion conflict без втрати користувацьких даних.
- [ ] JSON backup має schema validation, size limit, preview/dry-run, checksum за потреби та rollback.
- [ ] Cloud sync allowlist не відкидає локальні налаштування мовчки; sync conflict показує результат і зберігає локальне.
- [ ] Неповний provider fetch ніколи не замінює чинний повний список.

## Network/cache/failure behavior

- [ ] Усі HTTP запити мають timeout, byte/page/concurrency cap і cancellation.
- [ ] Retry/backoff відрізняють transient network, auth expiry, 403/404, 429 і malformed response.
- [ ] Cache має стабільний key, TTL, size bound, invalidation і поведінку при corrupted cache.
- [ ] Offline mode показує cached/local data та не зависає, очікуючи зовнішній сервіс.
- [ ] Stale generation response не оновлює новішу сторінку/track/connection.
- [ ] Partial writes/downloads атомарні або прибираються при невдачі; retry не плодить дублікати.

## Приватність і секрети

- [ ] Мапа персональних даних: локальні шляхи, listening history, likes, lyrics, account metadata, provider tokens.
- [ ] Користувач може очистити history/search cache/account connection та відключити sync окремо.
- [ ] Cookies/access/refresh/JWT не з'являються в logs, crash reports, screenshot fixtures, web storage чи telemetry.
- [ ] Приватні account запити не йдуть у public catalog backend і не включаються в cloud sync без окремої явної функції.
- [ ] Сесія зберігається в окремому контрольованому storage; перевірити Windows ACL/Unix permissions та secure deletion expectations.
- [ ] Завантажені файли, stream cache та provider metadata чітко позначені за джерелом і retention.

## Recovery

- [ ] Після crash/restart відновити queue/settings/store з валідного стану.
- [ ] Після провалу міграції повернутися до попередньої копії без втрати останніх даних.
- [ ] Після logout/expiry відкрити local library і зберегти imported records.
- [ ] Після filesystem/permission/network failure надати точну наступну дію для відновлення.

## Acceptance

- [ ] Negative tests покривають пошкоджені файли/JSON, stale responses, відмову доступу, timeout, 429 і disk-full.
- [ ] Backup/restore та disconnect/reconnect пройшли на ізольованому тестовому профілі.
- [ ] В audit output немає реальних назв файлів, account IDs або secrets.
