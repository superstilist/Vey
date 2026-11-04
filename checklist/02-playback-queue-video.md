# 02 — Відтворення, черга й відео

Поточний стан: основні transport controls, queue, repeat/shuffle та platform adapters є. Аудіо реального пристрою, Android runtime, web stream parity та live clip acceptance слід оцінювати окремо.

## Transport і аудіо

- [ ] Перевірити play/pause/stop, seek на початок/середину/кінець, volume 0–100%, automatic advance на кожному підтримуваному backend.
- [ ] Перевірити WAV/MP3/FLAC/Ogg/Opus/M4A, variable bitrate, відсутню тривалість, некоректний header, короткий/великий файл.
- [ ] Перевірити rapid track changes: попередній load і HTTP ranges скасовані, стара відповідь не стає current.
- [ ] Перевірити retry, відсутній device, зміну OS output, Bluetooth connect/disconnect, sleep/resume.
- [ ] Перевірити stream cache: пошкоджений, неповний, валідний cached file, паралельні range chunks, межі, очищення partial file після помилки.
- [ ] Перевірити фактичну тривалість/позицію на різних публічних YTM треках; не прирівнювати знайдений search result до успішного playback.
- [ ] Вирішити вимірювання gapless/crossfade як окреме дослідження; не додавати до UI, доки backend і lyric clock не проходять точні тести.

## Queue та playback session

- [ ] Перевірити play all, play next, append, reorder, remove current/next/previous, clear upcoming і збереження навмисних повторів.
- [ ] Перевірити repeat off/one/all на останньому та єдиному треку, включно зі зміною режиму під час playback.
- [ ] Переробити shuffle на зафіксований порядок сесії: перемішати один раз, уникати повтору до обходу; забезпечити Back/Next історію фактично програних треків.
- [ ] Перевірити, що Previous у звичайному режимі працює з позицією/історією відповідно до зрозумілого правила, а не губить вибір після reorder.
- [ ] Додати за потреби save/restore queue і відновлення після restart, з чітким правилом для локальних файлів, яких більше немає.
- [ ] Додати “Save queue as playlist” та “remove duplicates” лише як явні дії.
- [ ] Перевірити queue із недоступними, private, podcast, unavailable-with-ID і local tracks.

## Системні контролери

- [ ] Перевірити Windows SMTC, Linux MPRIS, macOS Now Playing, Android MediaSession і browser Media Session окремо.
- [ ] Звірити title/artist/art/duration/position/play state, OS next/previous/play/pause, metadata update при трек-зміні.
- [ ] Перевірити controls після згорнутого вікна, background, audio interruption і повернення з паузи.

## Кліпи та відео

- [ ] Перевірити browser video, Android TextureView, Windows/Linux FFmpeg surface на реальних fixture файлах.
- [ ] Перевірити синхронність video seek/pause/rate із аудіо, ресурсне очищення при переході сторінки/треку, cover fallback.
- [ ] Перевірити випадки без clip ID, недоступного відео, live stream і помилки декодера.
- [ ] Явно визначити, коли відео є лише візуальним супроводом, а коли track video ID не має playability.

## Приймання

- [ ] Той самий набір transport/queue сценаріїв пройшов Windows, браузер та Android runtime.
- [ ] Cancel зупиняє зайву роботу, не завершує worker і не дає старому треку змінити поточний стан.
- [ ] Audio state, UI Queue та системні медіаконтроли завжди показують один current track/position.
