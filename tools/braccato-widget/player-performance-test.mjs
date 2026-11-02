import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {mkdir, writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';

if (!process.argv[2]) throw new Error('Usage: npm run test:player-performance -- NEW_OUTPUT_DIRECTORY');
const output = resolve(process.argv[2]);
await mkdir(output);
const browser = await chromium.launch({channel:'msedge', headless:true});
const page = await browser.newPage({viewport:{width:1440,height:900}});
const errors = [];
page.on('pageerror', e => {if (!e.message.includes('Using exceptions for control flow')) errors.push(e.message);});
await page.route('**/*', r => new URL(r.request().url()).hostname === '127.0.0.1' ? r.continue() : r.abort());
const tracks = Array.from({length:2000}, (_, i) => ({path:`yt://fixture${i}`, videoId:`fixture${i}`,
  title:`Fixture song ${i}`, artist:'Vey test', album:'Performance fixture', durationSecs:30}));
await page.addInitScript(tracks => {
  if (!localStorage.getItem('playervf.rust.v1')) localStorage.setItem('playervf.rust.v1', JSON.stringify({
    version:1, tracks, likes:tracks.slice(0,10).map(t=>t.path), history:tracks.slice(0,12).map(track=>({track,playedAt:1,playCount:1})),
    settings:{language:'en',volume:.5,lyrics_translation:false,lyrics_romanization:false}
  }));
  window.storeWrites = [];
  const original = Storage.prototype.setItem;
  Storage.prototype.setItem = function(key,value) {
    if (key === 'playervf.rust.v1') window.storeWrites.push(performance.now());
    return original.call(this,key,value);
  };
  const NativeAudio = window.Audio;
  window.Audio = class extends NativeAudio {constructor(...args){super(...args);window.fixtureAudio=this;}};
}, tracks);
const wav = Buffer.alloc(44+32000*30);
wav.write('RIFF'); wav.writeUInt32LE(wav.length-8,4); wav.write('WAVEfmt ',8);
wav.writeUInt32LE(16,16); wav.writeUInt16LE(1,20); wav.writeUInt16LE(1,22);
wav.writeUInt32LE(16000,24); wav.writeUInt32LE(32000,28); wav.writeUInt16LE(2,32);
wav.writeUInt16LE(16,34); wav.write('data',36); wav.writeUInt32LE(wav.length-44,40);
await page.route('**/api/audio/*', r=>r.fulfill({contentType:'audio/wav',body:wav}));
await page.route('**/api/lyrics*', r=>r.fulfill({contentType:'application/json',body:'[]'}));
const stored = () => page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')));
const shot = name => page.screenshot({path:resolve(output,`${name}.png`)});
try {
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});
  await page.waitForTimeout(1200);
  await page.mouse.click(99,855);
  await page.waitForFunction(()=>window.fixtureAudio && !window.fixtureAudio.paused);
  await page.waitForTimeout(350);
  await page.mouse.click(99,855);
  await page.waitForFunction(()=>window.fixtureAudio.paused);
  await page.mouse.click(99,855);
  await page.waitForFunction(()=>!window.fixtureAudio.paused);
  await page.mouse.click(150,855);
  await page.waitForFunction(()=>window.fixtureAudio.src.endsWith('/fixture1'));
  await page.mouse.click(45,855);
  await page.waitForFunction(()=>window.fixtureAudio.src.endsWith('/fixture0'));
  await page.mouse.click(720,821);
  await page.waitForFunction(()=>window.fixtureAudio.currentTime>14 && window.fixtureAudio.currentTime<18);
  // The mini-player like edits one identity across all mounted models.
  await page.mouse.click(946,855);
  await page.waitForFunction(()=>!JSON.parse(localStorage.getItem('playervf.rust.v1')).likes.includes('yt://fixture0'));
  await page.mouse.click(90,242); // Liked
  await page.waitForTimeout(750);
  await shot('liked-after-unlike');
  // Removing the first liked row retains the following delegate but changes
  // its callback index. Selecting it must play fixture2, not the removed song.
  await page.mouse.click(1314,254);
  await page.waitForFunction(()=>!JSON.parse(localStorage.getItem('playervf.rust.v1')).likes.includes('yt://fixture1'));
  await page.mouse.click(500,254);
  await page.waitForFunction(()=>window.fixtureAudio.src.endsWith('/fixture2'));
  await shot('liked-after-row-removal');
  await page.mouse.click(90,98);
  await page.waitForTimeout(750);
  await page.mouse.click(946,855);
  await page.waitForFunction(()=>!JSON.parse(localStorage.getItem('playervf.rust.v1')).likes.includes('yt://fixture2'));
  await page.mouse.click(946,855);
  await page.waitForFunction(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).likes.includes('yt://fixture2'));
  await page.evaluate(()=>window.storeWrites=[]);
  await page.mouse.move(1200,855);
  await page.mouse.down();
  for (let i=0;i<60;i++) await page.mouse.move(1200+(i%30)*2,855);
  await page.mouse.up();
  await page.waitForTimeout(400);
  const dragWrites = await page.evaluate(()=>window.storeWrites.length);
  assert.ok(dragWrites > 0 && dragWrites < 20, `60 slider events generated ${dragWrites} full-store saves`);
  const finalVolume = await page.evaluate(()=>window.fixtureAudio.volume);
  assert.ok(Math.abs((await stored()).settings.volume-finalVolume)<.0001, 'latest slider value persists');
  // Pagehide and hidden visibility flush pending changes synchronously.
  const flushChecks=[];
  // Pagehide suspends Slint input, so exercise it last, then reload.
  for (const event of ['visibilitychange','pagehide']) {
    await page.mouse.click(1240,855);
    await page.waitForTimeout(350);
    const beforeVolume = await page.evaluate(()=>window.fixtureAudio.volume);
    await page.evaluate(event=>{
      window.storeWrites=[];
      // Same event-loop task as input: the 200ms timer cannot win this race.
      addEventListener('pointerup',()=>{
        if(event==='pagehide') dispatchEvent(new Event('pagehide'));
        else {
          Object.defineProperty(document,'hidden',{configurable:true,value:true});
          document.dispatchEvent(new Event('visibilitychange'));
          Object.defineProperty(document,'hidden',{configurable:true,value:false});
        }
      },{once:true});
    },event);
    await page.mouse.click(1200,855);
    const current=await page.evaluate(()=>window.fixtureAudio.volume);
    assert.ok(Math.abs(beforeVolume-current)>.1, `${event} input actually changes volume`);
    assert.ok(Math.abs((await stored()).settings.volume-current)<.0001, `${event} flushes latest state`);
    assert.equal(await page.evaluate(()=>window.storeWrites.length),1, `${event} performs one pending save`);
    flushChecks.push(event);
  }
  const saved=await stored();
  await page.reload();
  await page.locator('iframe').waitFor({state:'attached'});
  await page.waitForTimeout(1000);
  assert.deepEqual((await stored()).likes,saved.likes);
  assert.equal((await stored()).settings.volume,saved.settings.volume);
  await shot('restored-home');
  assert.deepEqual(errors,[]);
  const report={passed:true,tracks:tracks.length,sliderEvents:60,dragWrites,flushChecks,
    checks:['play/pause','next/previous','seek','like identity across navigation','selection after removing a liked row','coalesced volume saves','latest value persisted','pagehide flush','hidden-page flush','reload persistence','no browser errors']};
  await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
} catch(error) {await shot('failure');throw error;}
finally {await browser.close();}
