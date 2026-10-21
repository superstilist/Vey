import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const browser=await chromium.launch({channel:'msedge',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900}});
const errors=[];
page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
const tracks=['abcdefghijk','lmnopqrstuv'].map((id,index)=>({path:`yt://${id}`,videoId:id,title:index?'Blue fixture':'Red fixture',artist:'Appearance test',durationSecs:30,thumbnailUrl:`http://127.0.0.1:4181/appearance-${index}.png`}));
await page.addInitScript(tracks=>{if(!localStorage.getItem('playervf.rust.v1'))localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks,history:tracks.map(track=>({track,playedAt:1,playCount:1}))}));},tracks);
const covers=await page.evaluate(()=>['#ff0000','#0000ff'].map(color=>{const c=document.createElement('canvas');c.width=c.height=64;const x=c.getContext('2d');x.fillStyle=color;x.fillRect(0,0,64,64);return c.toDataURL().split(',')[1];}));
await page.route('**/appearance-*.png',route=>route.fulfill({contentType:'image/png',body:Buffer.from(covers[Number(route.request().url().match(/appearance-(\d)/)[1])],'base64')}));
await page.route('**/api/lyrics*',route=>route.fulfill({contentType:'application/json',body:'[]'}));
const wav=Buffer.alloc(44+32000*30);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
await page.route('**/api/audio/*',route=>route.fulfill({contentType:'audio/wav',body:wav}));
const screenshot=name=>page.screenshot({path:fileURLToPath(new URL(`../../docs/screenshots/${name}.png`,import.meta.url))});
const pixel=async(x,y)=>{
  const buffer=await page.screenshot({clip:{x,y,width:1,height:1}});
  return page.evaluate(async base64=>{const b=await createImageBitmap(new Blob([Uint8Array.from(atob(base64),c=>c.charCodeAt(0))],{type:'image/png'}));const c=new OffscreenCanvas(1,1);const ctx=c.getContext('2d');ctx.drawImage(b,0,0);b.close();return [...ctx.getImageData(0,0,1,1).data].slice(0,3);},buffer.toString('base64'));
};
const colors=async()=>({surface:await pixel(1100,16),accent:await pixel(82,855)});
const pref=()=>page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings);
const switchTrack=async()=>{await page.mouse.click(150,855);await page.waitForTimeout(1400);};
try {
  await page.goto('http://127.0.0.1:4181/');await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(1000);
  await page.mouse.click(99,855);await page.waitForTimeout(1500);const warm=await colors();
  await page.mouse.click(150,855);await page.waitForTimeout(100);const fading={accent:await pixel(82,855)};await page.waitForTimeout(1300);const cool=await colors();
  assert.notDeepEqual(warm,cool,'dynamic mode follows cover colors');
  assert.notDeepEqual(fading.accent,warm.accent,'palette makes intermediate progress');
  assert.notDeepEqual(fading.accent,cool.accent,'palette does not jump to the destination');
  await page.mouse.click(90,290);await page.waitForTimeout(650);await page.mouse.click(700,564);await page.waitForTimeout(400);
  await screenshot('appearance-dynamic');
  await page.mouse.click(700,288);await page.waitForTimeout(1300);assert.equal((await pref()).color_mode,'static');const fixed=await colors();
  await switchTrack();assert.deepEqual(await colors(),fixed,'static mode ignores cover changes');
  await page.mouse.click(150,855);await page.waitForTimeout(1400);assert.deepEqual(await colors(),fixed);
  await page.mouse.click(700,346);await page.waitForTimeout(1300);assert.equal((await pref()).color_mode,'background');const backgroundBlue=await colors();
  await switchTrack();const backgroundRed=await colors();
  assert.notDeepEqual(backgroundRed.surface,backgroundBlue.surface,'background-only mode follows the cover');
  assert.deepEqual(backgroundRed.accent,backgroundBlue.accent,'background-only mode retains the accent');
  await screenshot('appearance-background');
  await page.mouse.click(700,288);await page.waitForTimeout(1300);
  await page.mouse.click(493,516);await page.waitForTimeout(1300);
  assert.equal((await pref()).color_hue,210,'static color choice is saved');
  const selected=await colors();assert.notDeepEqual(selected.accent,fixed.accent);
  await screenshot('appearance-static');
  await page.reload();await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(1500);
  assert.equal((await pref()).color_mode,'static');assert.equal((await pref()).color_hue,210);
  assert.deepEqual((await colors()).surface,selected.surface,'selected colors restore after reload');
  await page.setViewportSize({width:390,height:844});await page.waitForTimeout(400);
  await page.mouse.click(340,804);await page.waitForTimeout(600);await page.mouse.click(180,560);await page.waitForTimeout(400);
  await screenshot('appearance-phone');
  assert.deepEqual(errors,[]);
  const report={passed:true,checks:['intermediate color fade','dynamic covers','static covers stay fixed','background-only preserves accent','color selection persists','reload restores mode and color','phone layout','no browser errors']};
  await writeFile(new URL('../../docs/appearance-check.json',import.meta.url),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
} catch(error) { await screenshot('appearance-test-failure');throw error; }
finally {await browser.close();}
