import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const [catalogFile, outputArg] = process.argv.slice(2);
assert(catalogFile && outputArg, 'Usage: theme-presets-test.mjs CATALOG_JSON NEW_OUTPUT_DIRECTORY');
const output = resolve(outputArg);
await mkdir(output);
const presets = JSON.parse(await readFile(resolve(catalogFile), 'utf8'));
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/*', route => route.request().url().startsWith('http://127.0.0.1:4181/') ? route.continue() : route.abort());
  await page.goto('http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(() => !!window.PlayerVFBraccato);
  await page.evaluate(() => window.PlayerVFBraccato.update({visible:true,playing:false,timed:true,position:2.5,size:48,
    document:{lyrics:[{startTimeMs:1000,durationMs:8000,words:'Colors linger',parts:[{startTimeMs:1000,durationMs:4000,words:'Colors'},{startTimeMs:5000,durationMs:4000,words:'linger'}]}]}}));
  const results = [];
  for (const preset of presets) {
    assert(!preset.css.includes('url('), `${preset.id} must remain offline`);
    await page.evaluate(p => window.PlayerVFBraccato.update({customTheme:p.css,fontFamily:p.family,reduced:false}), preset);
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const result = await page.evaluate(() => {
      const view = document.querySelector('braccato-lyrics');
      const word = view.querySelector('.blyrics--word');
      if (!word) throw new Error('Missing timed words');
      const container = word.closest('.blyrics-container');
      const style = getComputedStyle(container);
      return {active:style.getPropertyValue('--blyrics-lyric-active-color').trim(),font:getComputedStyle(word).fontFamily,
        wave:style.getPropertyValue('--blyrics-animate-letter-wave').trim(),words:Array.from(view.querySelectorAll('.blyrics--word:not(.blyrics-word-highlight)')).map(w=>({text:w.dataset.content,time:w.dataset.time,duration:w.dataset.duration})),
        diagnostics:window.PlayerVFBraccato.diagnostics()};
    });
    if (preset.css.includes('--blyrics-lyric-active-color')) assert.equal(result.active.toLowerCase(), preset.active, `${preset.id} active color`);
    assert(result.font.includes(preset.family), `${preset.id} font`);
    assert.deepEqual(result.words,[{text:'Colors',time:'1',duration:'4'},{text:'linger',time:'5',duration:'4'}],`${preset.id} must retain real word boundaries and times`);
    await page.screenshot({path:resolve(output,`${preset.id}.png`)});
    results.push({id:preset.id,...result});
  }
  const wavePreset = presets.find(p => p.wave);
  assert(wavePreset, 'Need a wave preset to verify reduced motion');
  await page.evaluate(p => window.PlayerVFBraccato.update({customTheme:p.css,fontFamily:p.family,reduced:true}), wavePreset);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const reduced = await page.evaluate(() => getComputedStyle(document.querySelector('braccato-lyrics .blyrics-container')).getPropertyValue('--blyrics-animate-letter-wave').trim());
  assert.equal(reduced,'0','reduced motion must override a wave preset');
  await page.evaluate(p => window.PlayerVFBraccato.update({customTheme:p.css,fontFamily:p.family,reduced:false}),presets[0]);
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const reset = await page.evaluate(() => getComputedStyle(document.querySelector('braccato-lyrics .blyrics-container')).getPropertyValue('--blyrics-lyric-active-color').trim());
  assert.equal(reset,results[0].active,'reset must discard the previous palette');
  assert.deepEqual(errors,[]);
  await writeFile(resolve(output,'results.json'),JSON.stringify(results,null,2));
  console.log(`PASS: ${presets.length} offline browser presets, word boundaries, fonts, colors and reduced motion`);
} finally { await browser.close(); }
