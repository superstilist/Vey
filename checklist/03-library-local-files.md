# 03 — Медіатека та локальні файли

Поточний стан: desktop scan, локальна бібліотека, групи за альбомом/артистом, downloads, metadata tools і web tag readers частково/переважно присутні. Android SAF та runtime, browser reselection після reload і повне tag parity залишаються відкритими.

## Сканування та групування

- [ ] Перевірити додавання файлу, папки, кількох джерел і System Music; включно з недоступним/видаленим шляхом.
- [ ] Перевірити великі папки, дублікати з різними path та однакові пісні з різними версіями.
- [ ] Перевірити cancel/rescan, видалення source під час роботи, generation check і відновлення після помилки доступу.
- [ ] Перевірити зміни файлу/тегу/обкладинки поза Vey і визначити правила refresh.
- [ ] Перевірити категорії Songs/Albums/Artists/Playlists/Downloads, порожні теги, Unknown Artist, Unicode й однакові назви артистів.
- [ ] Перевірити сортування title/artist/album/duration/source/recently played та фільтри local/downloaded/liked/missing/excluded.
- [ ] Перевірити multi-select і bulk like, playlist, queue, remove/exclude, restore та правильність canonical identities.

## Метадані й обкладинки

- [ ] Звірити підтримувані tag formats для читання й запису: MP3 ID3, FLAC Vorbis/Picture, Ogg/Opus, MP4/M4A та інші фактично заявлені формати.
- [ ] Для кожного формату перевірити мінімальний, unicode, некоректний, дуже великий файл і кілька embedded pictures.
- [ ] Перевірити import/replace/export cover, розміри/байти, прозорість і backup попередньої обкладинки.
- [ ] Перевірити tag backup/list/restore і rollback при частковому write failure; файли публікувати атомарно.
- [ ] Android: перевірити MediaStore metadata, permission revoke і SAF picker/save round-trip на пристрої.
- [ ] Web: перевірити metadata extraction із selected bytes, title fallback, malformed/oversized metadata та повторний вибір файла.

## Downloads та кеш

- [ ] Додати видимий progress/total/size/speed, pause/resume за можливості та cancel.
- [ ] Перевірити перерваний download, нестачу місця, повтор після помилки й видалення часткового файла.
- [ ] Розрізняти permanent user download, original local file і disposable streaming cache у UI та cleanup.
- [ ] Перевірити local file priority при виборі однакового remote track.
- [ ] Перевірити artwork cache TTL/size cap/invalid response/offline; пріоритет поточного треку зберігається.

## Relink, backup та restore

- [ ] Relink одного файлу та всієї папки після перенесення; зберегти likes, history, playlist, lyrics choices і offsets.
- [ ] Перевірити кілька кандидатів із однаковою назвою: запропонувати preview, не вибирати неоднозначний файл мовчки.
- [ ] Export backup з описом включених даних; restore у порожню та непорожню медіатеку з dry-run.
- [ ] Перевірити формат version, missing/extra fields, пошкоджений JSON, duplicate IDs і rollback після помилки.
- [ ] Окремо документувати web object URLs, які не переживають reload, та зрозумілий шлях повторного вибору.

## Приймання

- [ ] 10k-track scan/grouping/sort закінчується без UI freeze на цільовому desktop; Android має device результат.
- [ ] Жодна дія над тегами не видаляє оригінал без явного запиту й recovery backup.
- [ ] Після scan/relink/download/tag edit усі views, queue, like state і collection counts узгоджені.
