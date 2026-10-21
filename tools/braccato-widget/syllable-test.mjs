import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const root = fileURLToPath(new URL('../../', import.meta.url));
execFileSync('cargo', ['run', '--locked', '--quiet', '--example', 'lyric_syllables'], { cwd: root, stdio: 'inherit' });
const document = JSON.parse(await readFile(new URL('../../target/lyric-syllables.json', import.meta.url), 'utf8'));
assert.equal(document.options.syncType, 'richsync');
const source = document.lyrics.find(line => line.words === 'Syllable test');
const expected = source.parts.filter(part => part.words.trim()).map(part => [
  part.words.trim(), part.startTimeMs / 1000, part.durationMs / 1000, !!part.isBackground,
]);
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
const errors = [];
page.on('pageerror', error => errors.push(String(error)));
const update = state => page.evaluate(state => window.PlayerVFBraccato.update(state), state);
const cues = () => page.locator('.blyrics--word:not(.blyrics-word-highlight)').evaluateAll(words =>
  words.map(word => [word.dataset.content.trim(), Number(word.dataset.time), Number(word.dataset.duration), word.classList.contains('blyrics-background-lyric')]),
);

try {
  await page.goto(process.env.BRACCATO_TEST_URL || 'http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(() => !!window.PlayerVFBraccato);
  // The embedded widget is transparent; supply the same dark host surface
  // that Vey normally provides instead of photographing white text on white.
  await page.addStyleTag({ content: 'html,body{background:#10140f!important;}' });
  for (const stage of [false, true]) {
    await update({ visible: true, playing: false, position: 1.55, rate: 1, offset: 0,
      mode: 'lyrics', timed: true, stage, size: 48, reduced: false, disableRichsync: false, document });
    await page.waitForFunction(() => window.PlayerVFBraccato.diagnostics().syncType === 'richsync');
    assert.deepEqual(await cues(), expected, 'Rust syllable and backing cues reach the upstream DOM unchanged');
    const groups = await page.locator('.blyrics-word-group').evaluateAll(groups => groups.map(group =>
      [...group.querySelectorAll('.blyrics--word:not(.blyrics-word-highlight)')].map(word => word.dataset.content.trim()),
    ));
    assert.ok(groups.some(group => JSON.stringify(group) === JSON.stringify(['Syl', 'la', 'ble'])),
      'adjacent syllables stay inside one word group without added spaces');
    for (const position of [3.2, 1.45, 2.3]) {
      await update({ position });
      assert.equal(await page.evaluate(() => window.PlayerVFBraccato.diagnostics().currentTime), position);
      assert.deepEqual(await cues(), expected, 'seeking preserves supplied part clocks');
    }
    await update({ reduced: true, position: 1.55 });
    assert.deepEqual(await cues(), expected, 'reduced motion preserves synchronization');
  }
  await page.setViewportSize({ width: 390, height: 844 });
  await update({ stage: false, reduced: false, position: 1.55 });
  await page.evaluate(() => globalThis.document.fonts.ready);
  await page.waitForFunction(() => [...globalThis.document.querySelectorAll('.blyrics--line')].some(line => {
    const bounds = line.getBoundingClientRect();
    return Number(getComputedStyle(line).opacity) > 0.3 && bounds.bottom > 0 && bounds.top < innerHeight;
  }));
  await page.screenshot({ path: fileURLToPath(new URL('../../target/lyric-syllables.png', import.meta.url)) });
  assert.deepEqual(errors, []);
  console.log('Syllable integration passed: Rust -> Braccato, lead/backing, word grouping, stage, seek, reduced motion, phone capture.');
} finally {
  await browser.close();
}
