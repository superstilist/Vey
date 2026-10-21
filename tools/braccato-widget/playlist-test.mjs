import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const browser=await chromium.launch({channel:'msedge',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900}});
const errors=[];
page.on('pageerror',error=>{if(!error.message.includes('Using exceptions for control flow'))errors.push(error.message);});
const track={path:'yt://abcdefghijk',videoId:'abcdefghijk',title:'Playlist fixture',artist:'Vey test',durationSecs:30};
const lists=Array.from({length:20},(_,i)=>({id:`local-${i}`,name:`Список ${i+1} — довга назва для перевірки відступів`,tracks:[]}));
await page.addInitScript(({track,lists})=>{
  localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks:[track],history:[{track,playedAt:1,playCount:1}],playlists:lists,settings:{rate:1.75,volume:.5}}));
},{track,lists});
const wav=Buffer.alloc(44+32000*30);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);
wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);
wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
await page.route('**/api/audio/*',route=>route.fulfill({contentType:'audio/wav',body:wav}));
await page.route('**/api/lyrics*',route=>route.fulfill({contentType:'application/json',body:'[]'}));
const state=()=>page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')));
const screenshot=name=>page.screenshot({path:fileURLToPath(new URL(`../../docs/screenshots/${name}.png`,import.meta.url))});
const waitSaved=predicate=>page.waitForFunction(predicate);
try {
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});
  await page.waitForTimeout(900);
  await page.mouse.click(99,855);await page.waitForTimeout(400);
  await page.mouse.click(1096,855);await page.waitForTimeout(150);
  await screenshot('playlist-picker-desktop');
  await page.mouse.click(700,360);
  await waitSaved(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).playlists[0].tracks.length===1);
  assert.deepEqual((await state()).playlists[0].tracks,[track.path]);
  await page.mouse.click(1096,855);await page.waitForTimeout(100);
  await page.mouse.move(700,440);await page.mouse.wheel(0,2400);await page.waitForTimeout(200);
  await screenshot('playlist-picker-scrolled');
  await page.mouse.click(700,500);
  await waitSaved(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).playlists[19].tracks.length===1);
  await page.mouse.click(1096,855);await page.waitForTimeout(100);
  await page.mouse.click(700,572);await page.waitForTimeout(100);
  await screenshot('playlist-picker-create');
  await page.mouse.click(700,430);await page.keyboard.type('Created from current track');await page.keyboard.press('Enter');
  await waitSaved(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).playlists.some(p=>p.name==='Created from current track'));
  assert.deepEqual((await state()).playlists.find(p=>p.name==='Created from current track').tracks,[track.path]);
  await page.mouse.click(1096,855);await page.waitForTimeout(100);
  await page.setViewportSize({width:320,height:640});await page.waitForTimeout(400);
  await screenshot('playlist-picker-mobile');
  await page.mouse.click(150,280);
  await waitSaved(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).playlists[1].tracks.length===1);
  assert.deepEqual(errors,[]);
  const report={passed:true,checks:['actual playlist destinations','desktop selection persists the track','long list scroll reaches the final playlist','create from picker adds the current track','320px picker selection','no browser errors']};
  await writeFile(new URL('../../docs/playlist-check.json',import.meta.url),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
} catch(error) {
  await screenshot('playlist-test-failure');throw error;
} finally {await browser.close();}
