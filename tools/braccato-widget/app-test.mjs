import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const browser=await chromium.launch({channel:'msedge',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900}});
const errors=[];
const consoleErrors=[];
page.on('console',m=>{if(m.type()==='error')consoleErrors.push(m.text());});
page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
const track={path:'yt://abcdefghijk',title:'Widget test',artist:'PlayerVF',album:'Test',durationSecs:30,videoId:'abcdefghijk',thumbnailUrl:''};
const lyrics={provider:'Widget test',sync_type:'RichSync',format:'lrc',lyrics:{offset_ms:0,language:'en',songwriters:['Test Writer'],lines:[
  {start_ms:1000,duration_ms:4000,text:'Keep the music moving',translation:'Нехай музика звучить',romanization:'Keep the music moving',parts:[
    {start_ms:1000,duration_ms:1000,text:'Keep ',is_background:false},
    {start_ms:2000,duration_ms:1000,text:'the ',is_background:false},
    {start_ms:3000,duration_ms:1000,text:'music ',is_background:false},
    {start_ms:4000,duration_ms:1000,text:'moving',is_background:false}]},
  {start_ms:5000,duration_ms:4000,text:'Another line to sing',parts:[{start_ms:5000,duration_ms:4000,text:'Another line to sing',is_background:false}]}]}};
await page.addInitScript(track=>localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks:[track],history:[{track,playedAt:1,playCount:1}]})),track);
await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify([lyrics])}));
const wav=Buffer.alloc(44+16000*2*30);
wav.write('RIFF',0);wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
await page.route('**/api/audio/*',r=>r.fulfill({contentType:'audio/wav',body:wav}));
try {
  await page.goto(process.env.BRACCATO_APP_URL || 'http://127.0.0.1:4181/');
  await page.locator('iframe[title="Braccato lyrics"]').waitFor({state:'attached',timeout:30000});
  const frame=page.frames().find(f=>f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(()=>!!window.PlayerVFBraccato);
  await frame.evaluate(() => {
    window.ambientFrames = 0;
    window.documentUpdates = 0;
    addEventListener('message', e => {
      if (e.data?.channel === 'playervf-braccato-update') {
        window.latestHostState = {...window.latestHostState,...e.data.state};
        if (e.data.state?.document !== undefined) window.documentUpdates++;
      }
    });
    addEventListener('playervf:braccato-message', e => {
      if (e.detail.type === 'ambient') window.ambientFrames++;
    });
  });
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/36-kawarp-home.png',import.meta.url))});
  await page.mouse.click(90,865);
  await page.waitForTimeout(250);
  await page.mouse.click(450,865);
  await page.locator('iframe[title="Braccato lyrics"]').waitFor({state:'visible'});
  await frame.waitForFunction(()=>window.PlayerVFBraccato.diagnostics().lineCount>=2);
  const before=await frame.evaluate(()=>{const d=window.PlayerVFBraccato.diagnostics();delete d.html;return d;});
  assert.equal(before.status,'rendering');
  assert.ok(before.letters>10);
  const documentsBefore = await frame.evaluate(() => window.documentUpdates);
  await page.waitForTimeout(250);
  assert.equal(await frame.evaluate(() => window.documentUpdates), documentsBefore, 'clock samples do not resend the full lyrics document');
  await frame.locator('.blyrics--line').nth(1).click({force:true});
  await frame.waitForFunction(()=>{const t=window.PlayerVFBraccato.diagnostics().currentTime;return t>=5&&t<8;});
  const box=await page.locator('iframe[title="Braccato lyrics"]').boundingBox();
  assert.ok(box.x>500 && box.width>300 && box.height>200,JSON.stringify(box));
  await frame.waitForFunction(() => window.ambientFrames > 0);
  const activeFrameCount = await frame.evaluate(() => window.ambientFrames);
  await frame.waitForFunction(count => window.ambientFrames > count + 1,activeFrameCount,{timeout:3000});
  assert.equal(await frame.evaluate(() => getComputedStyle(document.body).backgroundColor), 'rgba(0, 0, 0, 0)');
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/35-braccato-wasm-integrated.png',import.meta.url))});
  const buildsBeforeTools = await frame.evaluate(() => window.PlayerVFBraccato.diagnostics().builds);
  await page.mouse.click(1374,100);
  await page.waitForFunction(() => document.querySelector('iframe').style.clipPath.includes('evenodd'));
  assert.deepEqual(await page.locator('iframe[title="Braccato lyrics"]').boundingBox(), box, 'settings do not resize or move lyrics');
  const overlay = await frame.evaluate(() => window.latestHostState.overlay);
  assert.ok(overlay.width<=380 && overlay.height<=304,JSON.stringify(overlay));
  assert.equal(await page.evaluate(({x,y}) => document.elementFromPoint(x,y)?.tagName, {x:overlay.x+overlay.width/2,y:overlay.y+overlay.height/2}), 'CANVAS', 'overlay receives input above iframe');
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/38-player-settings-overlay.png',import.meta.url))});
  await page.mouse.click(overlay.x+overlay.width-58,overlay.y+222);
  await frame.waitForFunction(() => window.latestHostState.offset===100);
  await page.mouse.click(overlay.x+148,overlay.y+222);
  await frame.waitForFunction(() => window.latestHostState.offset===0);
  await page.mouse.click(overlay.x+overlay.width-32,overlay.y+32);
  await page.waitForFunction(() => document.querySelector('iframe').style.clipPath==='');
  assert.equal(await frame.evaluate(() => window.PlayerVFBraccato.diagnostics().builds),buildsBeforeTools,'tools do not rebuild lyrics');
  await page.mouse.click(306,100);
  await page.locator('iframe[title="Braccato lyrics"]').waitFor({state:'hidden'});
  await page.mouse.click(450,865);
  await page.locator('iframe[title="Braccato lyrics"]').waitFor({state:'visible'});
  await page.waitForTimeout(250);
  assert.deepEqual(await page.locator('iframe[title="Braccato lyrics"]').boundingBox(),box,'reopening retains widget geometry');
  assert.equal(await frame.evaluate(() => window.PlayerVFBraccato.diagnostics().builds),buildsBeforeTools,'reopening preserves lyric document');
  await page.mouse.click(90,865);
  await frame.waitForFunction(()=>window.latestHostState.playing===false);
  await frame.waitForFunction(()=>{
    const now=performance.now();
    if(window.quietFrames!==window.ambientFrames){window.quietFrames=window.ambientFrames;window.quietAt=now;}
    return now-window.quietAt>=200;
  },undefined,{timeout:4000});
  const pausedFrames = await frame.evaluate(() => window.ambientFrames);
  await page.waitForTimeout(300);
  assert.equal(await frame.evaluate(() => window.ambientFrames), pausedFrames, 'pause stops frame requests after any pending color morph settles');
  await page.setViewportSize({width:390,height:844});
  await page.waitForTimeout(350);
  const mobile=await page.locator('iframe[title="Braccato lyrics"]').boundingBox();
  assert.ok(mobile.x>=0 && mobile.width<390 && mobile.height>200,JSON.stringify(mobile));
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/37-kawarp-mobile-player.png',import.meta.url))});
  await page.mouse.click(348,38);
  await page.waitForFunction(() => document.querySelector('iframe').style.clipPath.includes('evenodd'));
  assert.deepEqual(await page.locator('iframe[title="Braccato lyrics"]').boundingBox(),mobile,'mobile settings preserve lyric bounds');
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/39-mobile-settings-overlay.png',import.meta.url))});
  assert.deepEqual(errors,[]);
  const report={passed:true,checks:['Rust WASM startup','original widget mounted','canonical Rust document','word effects','clock bridge','seek reaches Rust audio','Kawarp frame bridge','transparent lyric background','pause freezes background','desktop geometry','mobile resize','settings overlay input','settings preserve widget geometry','player close and reopen preserve renderer'],before,desktopBounds:box,mobileBounds:mobile};
  await writeFile(new URL('../../docs/braccato-wasm-check.json',import.meta.url),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
} catch(e) {
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/player-overlay-test-failure.png',import.meta.url))});
  console.error(JSON.stringify({errors,consoleErrors,body:await page.locator('body').innerText()},null,2));
  throw e;
} finally {await browser.close();}
