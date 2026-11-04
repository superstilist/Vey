# 07 — UI, мобільний досвід і доступність

## Навігація та layout

- [ ] Перевірити основні сторінки: Home, Explore, Search, Library, Liked, Playlist, Queue, Settings, Player і detail pages.
- [ ] Узгодити back/forward, scroll restore, selected tab, modal/panel dismissal і browser history.
- [ ] Перевірити desktop, collapsed sidebar, tablet, compact mobile, portrait/landscape та 320–430 px width.
- [ ] Android: status/navigation/cutout insets, edge-to-edge, keyboard/IME, Back gesture, rotation та safe-area.
- [ ] Всі важливі дії мають loading, success, empty, error, offline та retry state.
- [ ] Немає перекриття mini-player, bottom tabs, modal, system bars та keyboard.

## Controls та input

- [ ] Кожна кнопка/slider/list row доступна keyboard-ом і має видимий focus.
- [ ] Touch targets, gestures, drag/drop та scrolling перевірені на справжньому touch device.
- [ ] Контекстні дії: Play next, Add to queue, Add to playlist, Like, Download, Details, Copy/share за наявності.
- [ ] Keyboard shortcuts мають таблицю, конфлікти з OS/browser та спосіб discoverability.
- [ ] Розглянути вибір елементів без крихких absolute pointer coordinates у UI fixtures.

## Settings, appearance, themes

- [ ] Повний inventory Settings проти підтримуваних Store fields: мова, playback, lyrics, appearance, Home, accounts, data.
- [ ] Налаштування мають зрозумілу дію Save/Reset і preview; старі/missing values нормалізуються без втрати інших налаштувань.
- [ ] Перевірити theme contrast, custom colors/fonts, lyrics-only opt-out і native supported effects.
- [ ] Appearance transition не перевантажує navigation/render і не оновлює hidden surfaces без потреби.
- [ ] Theme/community assets мають джерело, attribution, форматну перевірку та fallback.

## Доступність і локалізація

- [ ] Перевірити WCAG контраст, focus order, labels, states, slider value, dialog semantics і keyboard trap.
- [ ] Визначити screen reader support для Slint canvas shell; DOM lyrics/editor coverage не поширювати на весь застосунок.
- [ ] Перевірити reduced motion, text scaling, high contrast/OS theme та зменшену анімацію.
- [ ] Перевірити всі 8 мов на truncation, pluralization, fallback, menu width і date/number formatting.
- [ ] Додати переклади нових каталожних, provider, error і permission states.

## Візуальне доведення

- [ ] Огляд screenshots для desktop/phone/tablet кожного великого релізу.
- [ ] Перевірити empty/error/loading states, довгі назви, без cover, RTL/діакритики/японські glyphs.
- [ ] Pixel tests мають контроль renderer/font/scale; use tolerance там, де субпіксельний AA змінюється.
- [ ] Всі UI fixtures ізольовані від реальних user storage/account/files.

## Acceptance

- [ ] Ключові задачі завершуються mouse, keyboard та touch, де пристрій їх підтримує.
- [ ] Збережені accessibility обмеження й manual acceptance для всіх непідтриманих surface-ів.
