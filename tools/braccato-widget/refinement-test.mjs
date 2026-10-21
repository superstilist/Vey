import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const browser=await chromium.launch({channel:'msedge',headless:true});
let page;
const errors=[];
const screenshots=[];
try {
  page=await browser.newPage({viewport:{width:1440,height:900}});
  page.setDefaultTimeout(20000);
  page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
  const track={path:'yt://abcdefghijk',videoId:'abcdefghijk',title:'Video and lyrics fixture',artist:'Vey fixture',durationSecs:4};
  await page.addInitScript(track=>{
    const AudioClass=window.Audio;window.Audio=class extends AudioClass{constructor(...a){super(...a);window.fixtureAudio=this;}};
    if(!localStorage.getItem('playervf.rust.v1'))localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,settings:{language:'en',art_mode:'clip'},tracks:[track],history:[{track,playedAt:1,playCount:1}]}));
  },track);
  const video=await readFile(new URL('../../target/video-surface-fixture.mp4',import.meta.url));
  await page.route('**/fixture.mp4',r=>{
    const range=r.request().headers()['range']?.match(/bytes=(\d+)-(\d*)/);
    if(!range)return r.fulfill({contentType:'video/mp4',headers:{'Accept-Ranges':'bytes'},body:video});
    const start=Number(range[1]),end=range[2]?Math.min(Number(range[2]),video.length-1):video.length-1;
    return r.fulfill({status:206,contentType:'video/mp4',headers:{'Accept-Ranges':'bytes','Content-Range':`bytes ${start}-${end}/${video.length}`},body:video.subarray(start,end+1)});
  });
  await page.route('**/api/video',r=>r.fulfill({contentType:'application/json',body:JSON.stringify({url:'http://127.0.0.1:4181/fixture.mp4',mime:'video/mp4'})}));
  await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify([{provider:'Fixture',sync_type:'Synced',format:'lrc',lyrics:{offset_ms:0,language:'en',songwriters:[],lines:[{start_ms:0,duration_ms:2000,text:'Line synchronization',parts:[]},{start_ms:2000,duration_ms:2000,text:'Uses real timestamps',parts:[]}]}}])}));
  for(const path of ['browse','search'])await page.route(`**/api/${path}*`,r=>r.fulfill({contentType:'application/json',body:'[]'}));
  const wav=Buffer.alloc(44+32000*4);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
  await page.route('**/api/audio/*',r=>{
    const range=r.request().headers()['range']?.match(/bytes=(\d+)-(\d*)/);
    if(!range)return r.fulfill({contentType:'audio/wav',headers:{'Accept-Ranges':'bytes'},body:wav});
    const start=Number(range[1]),end=range[2]?Math.min(Number(range[2]),wav.length-1):wav.length-1;
    return r.fulfill({status:206,contentType:'audio/wav',headers:{'Accept-Ranges':'bytes','Content-Range':`bytes ${start}-${end}/${wav.length}`},body:wav.subarray(start,end+1)});
  });
  const shot=async name=>{const path=new URL(`../../docs/screenshots/refinement-${name}.png`,import.meta.url);await page.screenshot({path:fileURLToPath(path)});screenshots.push(name);};
  const nav=async(view,section='')=>{await page.evaluate(({view,section})=>dispatchEvent(new PopStateEvent('popstate',{state:{vey:{view,section}}})),{view,section});await page.waitForTimeout(700);};
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(1000);
  await page.mouse.click(99,855);await page.waitForTimeout(400);await nav('lyrics');
  await shot('video-loading');
  await page.waitForFunction(()=>{const v=document.querySelector('video');return v?.readyState>=2&&v.style.display==='block';});
  assert.equal(await page.locator('video').evaluate(v=>v.muted),true);
  await page.evaluate(()=>{fixtureAudio.pause();fixtureAudio.currentTime=1.5;});
  await page.waitForTimeout(500);
  await page.waitForFunction(()=>Math.abs(document.querySelector('video').currentTime-1.5)<0.15);
  await shot('desktop-video');
  await nav('settings','language');
  await page.mouse.click(800,274);await page.waitForTimeout(300);
  assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings.language),'cs');
  await page.reload();await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(800);
  assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings.language),'cs');
  await nav('settings','language');await shot('language-picker');
  await nav('settings','appearance');
  const before=await page.screenshot({clip:{x:1410,y:700,width:1,height:1}});
  await page.mouse.click(1000,610);await page.waitForTimeout(1200);
  const prefs=()=>page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings);
  const opacity=(await prefs()).kawarp.opacity;
  assert.ok(opacity>0.5&&opacity<0.8,'background opacity control updates shared preferences');
  assert.notDeepEqual(await page.screenshot({clip:{x:1410,y:700,width:1,height:1}}),before,'opacity changes the actual background');
  await page.reload();await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(700);
  assert.equal((await prefs()).kawarp.opacity,opacity,'background preference survives reload');
  await nav('settings','appearance');await page.mouse.move(1100,700);await page.mouse.wheel(0,1200);await page.waitForTimeout(300);await shot('background-controls');

  for (const locale of ['uk','en','cs','pl','de','es','fr','ja']) {
    await page.evaluate(locale=>{const value=JSON.parse(localStorage.getItem('playervf.rust.v1'));value.settings.language=locale;localStorage.setItem('playervf.rust.v1',JSON.stringify(value));},locale);
    await page.reload();await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(450);await nav('settings');await shot(`locale-${locale}`);await page.setViewportSize({width:320,height:740});await page.waitForTimeout(350);await shot(`locale-${locale}-phone`);await page.setViewportSize({width:1440,height:900});await page.waitForTimeout(350);
  }
  for(const [name,width,height] of [['desktop',1440,900],['phone',320,640],['tablet',900,900],['landscape',740,390]]){
    await page.setViewportSize({width,height});await page.waitForTimeout(300);
    for(const route of ['home','explore','library','liked','search','queue','playlist','settings','lyrics']){await nav(route);await shot(`${name}-${route}`);}
    for(const section of ['language','appearance','data']){await nav('settings',section);await shot(`${name}-${section}`);}
  }
  assert.deepEqual(errors,[]);
  const report={passed:true,locales:8,viewports:4,screenshots,checks:['real muted video','song-clock seek','language UI selection','language persistence','Kawarp control and pixel output','Kawarp persistence','all routes','settings forms'],errors};
  await writeFile(new URL('../../docs/refinement-check.json',import.meta.url),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({passed:true,screenshots:screenshots.length,errors}));
} catch(error) {if(page)console.log('video diagnostics',await page.evaluate(async()=>{const url=performance.getEntriesByType('resource').map(r=>r.name).find(s=>s.includes('/snippets/')&&s.endsWith('/video-surface.js'));return url?(await import(url)).videoSurfaceDiagnostics():null;}));if(page)await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/refinement-error.png',import.meta.url))});throw error;} finally {await browser.close();}
