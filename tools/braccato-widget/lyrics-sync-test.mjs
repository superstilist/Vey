import { chromium } from 'playwright';
import assert from 'node:assert/strict';

const browser = await chromium.launch({ channel: 'msedge', headless: true });
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
const update = state => page.evaluate(state => window.PlayerVFBraccato.update(state), state);
const diagnostics = () => page.evaluate(() => window.PlayerVFBraccato.diagnostics());

try {
  await page.goto(process.env.BRACCATO_TEST_URL || 'http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(() => !!window.PlayerVFBraccato);

  await update({
    visible: true,
    playing: false,
    position: 1.8,
    rate: 1,
    offset: 0,
    mode: 'lyrics',
    timed: true,
    stage: false,
    size: 48,
    disableRichsync: false,
    document: {
      lyrics: [
        {
          startTimeMs: 1000,
          durationMs: 3000,
          words: 'first real word',
          parts: [
            { startTimeMs: 1000, durationMs: 650, words: 'first ' },
            { startTimeMs: 1650, durationMs: 950, words: 'real ' },
            { startTimeMs: 2600, durationMs: 700, words: 'word' },
          ],
        },
        {
          startTimeMs: 5000,
          durationMs: 2000,
          words: 'line synced words',
          parts: [
            { startTimeMs: 5000, durationMs: 0, words: 'line ' },
            { startTimeMs: 5000, durationMs: 0, words: 'synced ' },
            { startTimeMs: 5000, durationMs: 0, words: 'words' },
            { startTimeMs: 5000, durationMs: 0, words: '(yeah)', isBackground: true },
          ],
        },
      ],
    },
  });
  await page.waitForFunction(() => window.PlayerVFBraccato.diagnostics().lineCount === 2);

  assert.equal((await diagnostics()).syncType, 'richsync');
  const lines = await page.locator('.blyrics--line').evaluateAll(elements =>
    elements.map(line => [...line.querySelectorAll('.blyrics--word:not(.blyrics-word-highlight)')].map(word => ({
      text: word.dataset.content,
      time: Number(word.dataset.time),
      duration: Number(word.dataset.duration),
      lineSynced: word.classList.contains('blyrics-line-synced-word'),
    }))),
  );
  assert.deepEqual(lines[0].map(word => word.time), [1, 1.65, 2.6]);
  assert.deepEqual(lines[0].map(word => word.duration), [0.65, 0.95, 0.7]);
  assert.deepEqual(lines[0].map(word => word.lineSynced), [false, false, false]);
  assert.deepEqual(lines[1].map(word => word.time), [5, 5, 5, 5]);
  assert.deepEqual(lines[1].map(word => word.duration), [0, 0, 0, 0]);
  assert.ok(lines[1].every(word => word.lineSynced), 'line-only parts use simultaneous line-sync cues');
  assert.equal(
    await page.locator('.blyrics-background-lyric.blyrics--word:not(.blyrics-word-highlight)').count(),
    1,
    'line-only backing vocals remain visible',
  );

  await update({
    position: 1.7,
    document: {
      lyrics: [{
        startTimeMs: 1000,
        durationMs: 3000,
        words: '你好世界美丽歌词',
        parts: [
          { startTimeMs: 1000, durationMs: 500, words: '你好世界' },
          { startTimeMs: 1500, durationMs: 700, words: '美丽' },
          { startTimeMs: 2300, durationMs: 900, words: '歌词' },
        ],
      }],
      options: { syncType: 'richsync' },
    },
  });
  await page.waitForFunction(() => window.PlayerVFBraccato.diagnostics().lineCount === 1);
  assert.deepEqual(await page.locator('.blyrics--word:not(.blyrics-word-highlight)').evaluateAll(
    words => words.map(word => [word.dataset.content, Number(word.dataset.time)]),
  ), [['你好世界', 1], ['美丽', 1.5], ['歌词', 2.3]], 'Chinese QRC segments retain separate word cues');

  await update({
    position: 0,
    timed: false,
    document: { lyrics: [{ startTimeMs: 0, durationMs: 0, words: 'plain unsynchronized words' }] },
  });
  await page.waitForFunction(() => window.PlayerVFBraccato.diagnostics().lineCount === 1);
  assert.equal((await diagnostics()).syncType, 'none');

  await update({
    position: 0.5, timed: true,
    document: {
      lyrics: [{ startTimeMs: 0, durationMs: 2000, words: 'Zero origin line' }],
      options: { syncType: 'synced' },
    },
  });
  await page.waitForFunction(() => window.PlayerVFBraccato.diagnostics().syncType === 'synced');
  assert.deepEqual(await page.locator('.blyrics--word:not(.blyrics-word-highlight)').evaluateAll(
    words => words.map(word => [Number(word.dataset.time), Number(word.dataset.duration)]),
  ), [[0, 0], [0, 0], [0, 0]], 'line cues highlight simultaneously with no stagger');
  await page.waitForFunction(() => !!document.querySelector('.blyrics--line.blyrics--active'));
  await update({ position: 2.5 });
  assert.equal((await diagnostics()).currentTime, 2.5);
  await update({ position: 0.5 });
  await page.waitForFunction(() => !!document.querySelector('.blyrics--line.blyrics--active'));

  await update({
    position: 0.5,
    document: {
      lyrics: [{ startTimeMs: 0, durationMs: 2000, words: 'First word', parts: [
        { startTimeMs: 0, durationMs: 0, words: 'First ' },
        { startTimeMs: 1000, durationMs: 0, words: 'word' },
      ] }],
      options: { syncType: 'richsync' },
    },
  });
  await page.waitForFunction(() => window.PlayerVFBraccato.diagnostics().syncType === 'richsync');
  assert.deepEqual(await page.locator('.blyrics--word:not(.blyrics-word-highlight)').evaluateAll(
    words => words.map(word => [Number(word.dataset.time), Number(word.dataset.duration)]),
  ), [[0, 0], [1, 0]], 'real start-only cues stay unchanged');

  await update({ timed: false, document: {
    lyrics: [{ startTimeMs: 0, durationMs: 0, words: 'Plain again' }],
    options: { syncType: 'none' },
  } });
  await page.waitForFunction(() => window.PlayerVFBraccato.diagnostics().syncType === 'none');
  for (const position of [0, 10, 1]) {
    await update({ position });
    assert.equal(await page.locator('.blyrics--line.blyrics--active').count(), 0, 'plain text never gets a playback highlight');
  }
  console.log('lyrics sync mode checks passed');
} finally {
  await browser.close();
}
