# 09 — Платформи, якість і реліз

## Windows desktop

- [ ] Clean/debug/release запуск на актуальній Windows версії.
- [ ] GPU renderer: navigation, clipping, DPI 100/125/150%, resize, minimize/restore, HiDPI monitors.
- [ ] Software renderer: clipping regression із tolerance, keyboard/input/audio smoke.
- [ ] WASAPI output switch, SMTC, dialogs, local library, signed-in browser integration boundary.
- [ ] Перевірити release package contents, version, hashes, notices і запуск на чистому профілі.

## Android

- [ ] Зібрати APK для кожного цільового ABI; verify manifest, min SDK, signing, size та install.
- [ ] Запустити на реальному телефоні й емуляторі; окремо записати модель/OS/ABI.
- [ ] Перевірити audio permission grant/deny/revoke, MediaStore scan, SAF select/save, rotation/insets/IME/Back.
- [ ] Перевірити local playback, seek, Bluetooth/output change, MediaSession, pause/resume, process death.
- [ ] Перевірити download cancel/recovery, network loss, storage low, offline start і lyric auth browser return.
- [ ] Не зараховувати compile/link/package check як device runtime.

## Browser

- [ ] Clean optimized WASM build та bindings за поточним Rust/UI source.
- [ ] Chrome і Edge: local file open/tag reads, playback, reload/reselect, browser history, keyboard/touch.
- [ ] Public network функції через same-origin backend; static host правильно показує недоступність backend.
- [ ] Quota/cache/timeout/429/error tests; responsive 320/390/430/tablet widths.
- [ ] Перевірити вкладку в background, pagehide/visibility flush і відновлення audio state.
- [ ] Accessibility обмеження Slint canvas задокументоване й основні доступні DOM маршрути перевірені.

## Linux та інші targets

- [ ] Native build і launch на підтриманому Linux host, аудіо device, MPRIS, dialogs, FFmpeg optional path.
- [ ] CI workflow реально завершився; не вважати конфігурацію CI доказом успішного запуску.
- [ ] macOS native acceptance лише на Apple host/SDK; browser support не прирівнювати до native release.
- [ ] Задокументувати неперевірені target-arch/driver/codec межі.

## Якість збірки

- [ ] `cargo fmt -- --check`, targeted Rust suites, native build і WASM compile/build.
- [ ] Для UI змін: screenshot review + browser/native input fixture; для renderer змін: GPU/software tests.
- [ ] Для widget змін: rebuild generated bundle з source і відсутність випадкового edit generated bundle як джерела правди.
- [ ] Для Android зміни: APK build плюс device runtime.
- [ ] Для account/network зміни: scrubbed parser fixtures, security checks і окремий opt-in live probe.
- [ ] Перевірити git diff/status перед/після; зберегти всі заздалегідь наявні user changes.

## Release gates

- [ ] Release version/notes відповідають фактично зібраному source.
- [ ] Чистий isolated profile запускається; backup/restore та локальні файли не зачеплені.
- [ ] Package checksum і потрібні артефакти перевірені; лишився rollback шлях.
- [ ] У release report наведені команди, pass/fail, platform/runtime scope, відомі обмеження й skipped перевірки.
