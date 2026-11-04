# 10 — Прибрати, спростити й переробити

Цей список не означає автоматичне видалення функцій. Спочатку потрібні докази дублювання/мертвого коду, перевірка залежностей і зрозуміла користувацька міграція.

## Тестова інфраструктура

- [ ] Замінити absolute coordinate clicks на role/label/identity-based input у player-performance та інших UI tests.
- [ ] Оновити artwork-queue test під актуальні 2 background + 1 reserved concurrency правила.
- [ ] Зменшити flaky pixel asserts software/GPU шляхом окремих renderer expectations і поведінкових clipping checks.
- [ ] Уніфікувати сценарії nav latency так, щоб controller-level test включав повний `on_navigate`/`refresh`, а isolated animation test лишився окремим.
- [ ] Прибрати дубльовані fixture дані/скріншоти тільки після посилання на актуальний test source.

## Код і моделі

- [ ] Знайти недосяжні callbacks, unused types, duplicated parsers та старі compatibility branches через search + compiler warnings + feature usage.
- [ ] Уніфікувати typed SearchResult/CollectionPage лише після native/WASM consumers inventory.
- [ ] Видаляти duplicate state/cache лише після визначення ownership, invalidation і race conditions.
- [ ] Перевірити public APIs/schema й усі call sites перед перейменуванням/видаленням.
- [ ] Автоматично форматувати/генерувати тільки source-owned output; не редагувати generated widget напряму як постійне рішення.

## UI та поведінка

- [ ] Інвентаризувати надлишкові theme/layout options; зберігати ті, що мають distinct behavior або користувацькі дані.
- [ ] Порівняти blur/streak/ambient effects у профілі й A/B; прибрати лише якщо витрата доведена та втрата орієнтації не виникає.
- [ ] Консолідувати повторні кнопки/меню в однаковий component із тими самими keyboard/touch actions.
- [ ] Виявити inert Settings fields і або під'єднати до справжньої поведінки, або прибрати з UI з migration plan.
- [ ] Прибрати прапорці `VEY_LEGACY_*` після визначеного періоду стабільності й наявного replacement/rollback.

## API та integrations

- [x] Прибрати застарілий Spotify OAuth account/import path; залишити один public URL importer, що конвертує точні збіги у YTM IDs. Повний checklist: [11 — Spotify public import](11-spotify-public-import.md).
- [ ] Свіжо перевірити міграцію старого Spotify account JSON і підтвердити, що tokens/Client ID/secret не лишилися в активному Vey сховищі.
- [ ] Прибрати застарілий provider endpoint/parser лише після fixtures на реальних scrubbed payloads і error fallback.
- [ ] Відокремити user session APIs від public search/catalog endpoints.
- [ ] Переконатися, що кеш/парсерні diagnostics не зберігають secret-bearing request data.

## Міграція та контроль

- [ ] Для кожного proposed removal записати: що прибирається, чим замінено, хто залежить, що втрачає користувач.
- [ ] Store/settings/data migration має preview/backup, backwards behavior або явний recovery path.
- [ ] Перевірити мову/старі preferences та Cloud allowlist, аби видалення control не скидало збережене значення.
- [ ] Видалити diagnostic output/artifacts з release package, залишивши потрібні provenance/license notices.
- [ ] Після видалення пройти native, web та Android parity/regression; неперевірену платформу не закривати.

## Не прибирати без окремого рішення

- Особисті playlist, likes, history, lyrics offsets/choices, themes, backups або local files.
- Підтримку навмисних playlist repeats і unavailable references із валідним ID.
- Скасування запитів, generation guards, size/concurrency/time bounds і безпечне atomic write behavior.
- Accessibility/keyboard actions лише тому, що візуально дублюють mouse/touch action.
