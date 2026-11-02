import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';

const output=resolve(process.argv[2]||'');
if(!process.argv[2])throw new Error('Usage: node lyric-appearance-test.mjs NEW_OUTPUT_DIRECTORY');
await mkdir(output); // Never overwrite previous screenshots.
const browser=await chromium.launch({channel:'msedge',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900}});
const errors=[];
page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
await page.route('**/*',r=>new URL(r.request().url()).hostname==='127.0.0.1'?r.continue():r.abort());
const track={path:'yt://abcdefghijk',title:'Appearance fixture',artist:'Vey',durationSecs:30,videoId:'abcdefghijk',thumbnailUrl:''};
await page.addInitScript(track=>{
  if(!localStorage.getItem('playervf.rust.v1'))localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks:[track],history:[{track,playedAt:1,playCount:1}],settings:{language:'en',lyrics_translation:false,lyrics_romanization:false}}));
},track);
const lyrics={provider:'Fixture',sync_type:'RichSync',format:'lrc',lyrics:{offset_ms:0,language:'en',songwriters:[],lines:[{start_ms:0,duration_ms:8000,text:'Feel the music',parts:[{start_ms:0,duration_ms:4000,text:'Feel ',is_background:false},{start_ms:4000,duration_ms:4000,text:'the music',is_background:false}]}]}};
await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify([lyrics])}));
const wav=Buffer.alloc(44+32000*30);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
await page.route('**/api/audio/*',r=>r.fulfill({contentType:'audio/wav',body:wav}));
const shot=name=>page.screenshot({path:resolve(output,`${name}.png`)});
const settings=()=>page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings);
async function pixel(x,y){
  const buffer=await page.screenshot({clip:{x,y,width:1,height:1}});
  return page.evaluate(async base64=>{const bitmap=await createImageBitmap(new Blob([Uint8Array.from(atob(base64),c=>c.charCodeAt(0))],{type:'image/png'}));const c=new OffscreenCanvas(1,1);const ctx=c.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();return [...ctx.getImageData(0,0,1,1).data].slice(0,3);},buffer.toString('base64'));
}
async function openAppearance(){
  const mobile=page.viewportSize().width<600;
  await frame.waitForFunction(()=>window.latestHostState?.navigation?.view==='lyrics'&&!window.latestHostState.camera);
  await page.waitForTimeout(250);
  await page.mouse.click(mobile?page.viewportSize().width-46:1374,mobile?72:100);
  await page.waitForTimeout(400);
  const overlay=await frame.evaluate(()=>window.latestHostState?.overlay);
  assert(overlay?.height>0,JSON.stringify({overlay,point:await page.evaluate(({x,y})=>({tag:document.elementFromPoint(x,y)?.tagName,box:document.querySelector('iframe')?.getBoundingClientRect().toJSON()}),{x:mobile?318:1374,y:mobile?38:100})}));
  await shot(mobile?'phone-tools':'desktop-tools');
  // With translation disabled, the Appearance action precedes Advanced.
  await page.mouse.click(overlay.x+overlay.width/2,overlay.y+310);
  await page.waitForTimeout(400);
}
let frame;
try{
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe[title="Braccato lyrics"]').waitFor({state:'attached'});
  frame=page.frames().find(f=>f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(()=>!!window.PlayerVFBraccato);
  await frame.evaluate(()=>addEventListener('message',e=>{if(e.data?.channel==='playervf-braccato-update')window.latestHostState={...window.latestHostState,...e.data.state};}));
  await page.waitForTimeout(700);
  await page.mouse.click(90,865);await page.waitForTimeout(300);
  await page.mouse.click(450,865);await page.waitForTimeout(500);
  await openAppearance();await shot('desktop-panel');
  assert.equal((await settings()).lyrics_theme_css,'');
  // Choose the next preset using the real combo box keyboard path.
  await page.mouse.click(700,158);await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');
  await page.waitForTimeout(250);await shot('desktop-preset-draft');
  assert.equal((await settings()).lyrics_theme_css,'','selection must remain a draft');
  await page.mouse.click(890,832);await page.waitForTimeout(200);
  assert.equal((await settings()).lyrics_theme_css,'');
  await openAppearance();
  await page.mouse.click(700,158);await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');await page.waitForTimeout(250);
  await page.mouse.click(982,832);await page.waitForTimeout(300);
  assert((await settings()).lyrics_theme_css.includes('glow'),'Apply must save the selected preset');
  assert.equal((await settings()).theme_entire_player,true,'full-player scope is saved');
  await openAppearance();
  await page.mouse.click(470,669);await page.waitForTimeout(250);
  await shot('custom-player-draft');
  assert(!(await settings()).lyrics_theme_css.includes('--vey-accent'),'color edits remain a draft');
  await page.mouse.click(982,832);await page.waitForTimeout(1100);
  assert((await settings()).lyrics_theme_css.includes('--vey-accent: #9dcfff'));
  assert.deepEqual(await pixel(99,838),[157,207,255],'custom accent reaches the actual player transport');
  await openAppearance();await page.mouse.click(980,267);await page.waitForTimeout(200);
  assert.equal((await settings()).theme_entire_player,true,'scope toggle is a draft');
  await page.mouse.click(982,832);await page.waitForTimeout(1100);
  assert.equal((await settings()).theme_entire_player,false);
  assert.notDeepEqual(await pixel(99,838),[157,207,255],'lyrics-only mode restores the application palette');
  await openAppearance();await page.mouse.move(800,400);await page.mouse.wheel(0,1100);await page.waitForTimeout(400);
  await shot('desktop-effects');
  await page.keyboard.press('Escape');await page.waitForTimeout(200);
  await page.setViewportSize({width:360,height:780});await page.waitForTimeout(500);
  await openAppearance();await shot('phone-panel');
  await page.mouse.move(300,380);await page.mouse.wheel(0,1300);await page.waitForTimeout(400);
  await shot('phone-effects');
  await page.mouse.click(176,730);await page.waitForTimeout(200);
  assert((await settings()).lyrics_theme_css.includes('glow'));
  assert.deepEqual(errors,[]);
  const report={passed:true,checks:['real dialog navigation','combo keyboard selection','draft isolation','Cancel','Apply/persistence','whole-player custom accent pixels','scope persistence and opt-out','scroll body','fixed footer','desktop/360px captures','no browser errors']};
  await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
}catch(error){await shot('failure');throw error;}finally{await browser.close();}
