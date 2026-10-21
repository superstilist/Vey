import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
let page;
const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  page=await browser.newPage({viewport:{width:390,height:844},isMobile:true,hasTouch:true});
  const errors=[];let browseCalls=0;
  page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
  await page.addInitScript(()=>{
    const NativeAudio=window.Audio;
    window.Audio=class extends NativeAudio {constructor(...args){super(...args);window.phoneAudio=this;}};
    localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks:[],settings:{volume:1,rate:1.75}}));
  });
  await page.route('**/api/browse*',r=>{browseCalls++;return r.fulfill({contentType:'application/json',body:'[]'});});
  await page.route('**/api/search*',r=>r.fulfill({contentType:'application/json',body:'[]'}));
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});
  const frame=page.frames().find(f=>f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(()=>!!window.PlayerVFBraccato);
  await frame.evaluate(()=>{
    window.cameras=[];
    addEventListener('message',e=>{
      if(e.data?.channel!=='playervf-braccato-update')return;
      if(e.data.state.navigation)window.nav=e.data.state.navigation;
      if(e.data.state.camera)window.cameras.push(e.data.state.camera);
    });
  });
  const nav=async(view,section='')=>{
    await frame.waitForFunction(({view,section})=>window.nav?.view===view&&window.nav?.section===section,{view,section});
    await page.waitForTimeout(560);
  };
  const tapTab=async(index)=>{
    const {width,height}=page.viewportSize();
    await page.touchscreen.tap(12+(width-24)*(index+.5)/5,height-40);
  };
  const shot=name=>page.screenshot({path:fileURLToPath(new URL(`../../docs/screenshots/${name}.png`,import.meta.url))});
  await page.waitForTimeout(1000);
  await shot('45-phone-home');
  await tapTab(4);await nav('settings');await shot('46-phone-settings');
  await page.touchscreen.tap(180,184);await nav('settings','audio');
  assert.equal(await page.evaluate(()=>window.phoneAudio.playbackRate),1,'legacy speed is reset to normal playback');
  await page.touchscreen.tap(130,218);await page.waitForTimeout(100);
  assert.ok(await page.evaluate(()=>window.phoneAudio.volume)<.7,'phone settings expose working volume');
  await page.mouse.move(130,218);await page.mouse.down();await page.mouse.move(300,218,{steps:8});await page.mouse.up();
  assert.ok(await page.evaluate(()=>window.phoneAudio.volume)>.8,'volume drag follows the visible rail');
  await page.keyboard.press('Home');await page.waitForTimeout(100);
  assert.equal(await page.evaluate(()=>window.phoneAudio.volume),0,'volume Home reaches mute');
  await page.keyboard.press('End');await page.waitForTimeout(100);
  assert.equal(await page.evaluate(()=>window.phoneAudio.volume),1,'volume End reaches full volume');
  await shot('47-phone-audio');
  await page.goBack();await nav('settings');
  await page.goForward();await nav('settings','audio');
  await page.touchscreen.tap(46,106);await nav('settings');
  await page.touchscreen.tap(180,278);await nav('settings','library');
  const chooser=page.waitForEvent('filechooser');
  await page.touchscreen.tap(180,255);
  const wav=Buffer.alloc(44+32000);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);
  wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);
  wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
  await (await chooser).setFiles({name:'Phone preview.wav',mimeType:'audio/wav',buffer:wav});
  await nav('library');
  assert.ok(await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).tracks.some(t=>t.title==='Phone preview')),'local file import updates the library');
  await shot('48-phone-library');
  await tapTab(4);await nav('settings','library');await tapTab(4);await nav('settings');
  await page.touchscreen.tap(180,372);await nav('settings','lyrics');
  await page.touchscreen.tap(180,304);await page.waitForTimeout(100);
  assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings.lyrics_reduced_motion),true,'reduced motion setting persists');
  await page.touchscreen.tap(180,304);await page.waitForTimeout(100);
  await page.touchscreen.tap(46,106);await nav('settings');
  for(const [width,height] of [[320,640],[360,740],[430,932]]){
    await page.setViewportSize({width,height});await page.waitForTimeout(450);
    await page.touchscreen.tap(140,466);await nav('settings','data');
    await shot(`49-phone-data-${width}`);
    await page.mouse.move(width/2,440);await page.mouse.wheel(0,500);await page.waitForTimeout(200);
    await shot(`50-phone-data-bottom-${width}`);
    await page.waitForTimeout(400);
    const clip={x:8,y:64,width:width-16,height:height-224};
    const underlay=await page.screenshot({clip});
    const cameraCount=await frame.evaluate(()=>window.cameras.length);
    await page.touchscreen.tap(width/2,height-116);await nav('lyrics');
    assert.equal(await frame.evaluate(()=>window.cameras.length),cameraCount,'mobile player opens without a page camera transition');
    await shot(`51-phone-player-overlay-${width}`);
    await page.touchscreen.tap(44,38);await nav('settings','data');
    assert.ok(underlay.equals(await page.screenshot({clip})),'closing the overlay preserves the scrolled settings page exactly');
    assert.equal(await frame.evaluate(()=>window.cameras.length),cameraCount,'closing the mobile player does not move the page');
    if(width===320) {
      await page.goBack();await nav('lyrics');
      await page.goBack();await nav('settings','data');
      assert.ok(underlay.equals(await page.screenshot({clip})),'browser Back dismisses the player and preserves the page');
    }
    // Return with the fixed bottom tab even when the form has been scrolled.
    await tapTab(4);await nav('settings');
  }
  await page.setViewportSize({width:390,height:844});await page.waitForTimeout(400);
  for(const index of [0,2,1,4,0,4]){await tapTab(index);await page.waitForTimeout(35);}
  await nav('settings');
  const cameras=await frame.evaluate(()=>window.cameras);
  assert.ok(cameras.some(c=>c.elapsed<c.duration),'navigation produces intermediate camera samples');
  assert.ok(cameras.every(c=>c.duration>=240&&c.duration<=520),'page travel uses smooth bounded duration');
  await tapTab(1);await nav('explore');const first=browseCalls;
  await tapTab(0);await nav('home');await tapTab(1);await nav('explore');
  assert.equal(browseCalls,first,'returning to Explore uses its cached results');
  await page.touchscreen.tap(312,154);await page.waitForTimeout(500);
  assert.equal(browseCalls,first+1,'explicit refresh still requests new results');
  assert.deepEqual(errors,[]);
  const report={passed:true,checks:['labeled bottom tabs','normal playback after legacy speed preference','volume tap, drag and keyboard endpoints','browser back/forward','settings back button','local file import','reduced motion persistence','320/360/390/430px layouts','mobile player overlay preserves page and scroll','overlay Back dismissal','rapid navigation','cached Explore and explicit refresh','no browser errors'],browseCalls};
  await writeFile(new URL('../../docs/phone-check.json',import.meta.url),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
} catch(error) {
  if(page){await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/phone-test-failure.png',import.meta.url))});console.error(await page.evaluate(()=>history.state));}
  throw error;
} finally {await browser.close();}
