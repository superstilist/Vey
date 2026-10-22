import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const browser = await chromium.launch({channel:'msedge',headless:true});
const track = {path:'yt://abcdefghijk',title:'Search input fixture',artist:'Vey',durationSecs:120,videoId:'abcdefghijk'};
const lyrics = {provider:'fixture',sync_type:'Synced',format:'lrc',lyrics:{offset_ms:0,lines:[
  {start_ms:0,duration_ms:60000,text:'Search remains editable',parts:[]},
  {start_ms:60000,duration_ms:60000,text:'After focusing lyrics',parts:[]},
]}};
const wav=Buffer.alloc(44+32000*120);
wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
const results=[];
try {
  for(const [width,height] of [[1440,900],[320,640],[390,844],[430,932],[844,390]]) {
    const mobile=width<800||height<500;
    const context=await browser.newContext({viewport:{width,height},isMobile:mobile,hasTouch:mobile});
    const page=await context.newPage();
    const errors=[];const queries=[];
    page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
    await page.addInitScript(track=>{
      localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,settings:{language:'en'},tracks:[track],history:[{track,playedAt:1,playCount:1}]}));
      const NativeAudio=window.Audio;
      window.Audio=class extends NativeAudio {constructor(...args){super(...args);window.fixtureAudio=this;}};
    },track);
    await page.route('**/api/search*',r=>{queries.push(new URL(r.request().url()).searchParams.get('q'));return r.fulfill({contentType:'application/json',body:JSON.stringify([track])});});
    await page.route('**/api/browse*',r=>r.fulfill({contentType:'application/json',body:'[]'}));
    await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify([lyrics])}));
    await page.route('**/api/audio/*',r=>r.fulfill({contentType:'audio/wav',body:wav}));
    await page.goto(process.env.BRACCATO_APP_URL || 'http://127.0.0.1:4181/');
    await page.locator('iframe').waitFor({state:'attached'});
    await page.waitForTimeout(1000);
    const submit=async query=>{
      const count=queries.length;
      await page.mouse.click(mobile?180:450,32);
      await page.keyboard.press('ControlOrMeta+a');
      await page.keyboard.press('Backspace');
      await page.keyboard.type(query,{delay:15});
      // Text edits must neither navigate nor launch a request before Enter.
      await page.waitForTimeout(150);
      assert.equal(queries.length,count,'editing does not submit or rebuild results');
      const response=page.waitForResponse(r=>r.url().includes('/api/search?'));
      await page.keyboard.press('Enter');await response;
      await page.waitForTimeout(600);
      assert.equal(queries.at(-1),query,'search receives the complete typed query');
    };
    await submit('before song');
    await page.mouse.click(mobile?36:99,height-(mobile?(height<500?84:92):45));
    await page.waitForFunction(()=>window.fixtureAudio&&!window.fixtureAudio.paused);
    await submit('after song');
    if(mobile){
      const count=queries.length;
      const response=page.waitForResponse(r=>r.url().includes('/api/search?'));
      await page.mouse.click(34,height<500?26:32);await response;
      await page.waitForTimeout(600);
      assert.equal(queries.length,count+1,'phone search icon submits the visible query');
      assert.equal(queries.at(-1),'after song');
    }
    if(height>=500){
    await page.mouse.click(mobile?190:450,height-(mobile?(height<500?84:92):45));
    const iframe=page.locator('iframe[title="Braccato lyrics"]');
    await iframe.waitFor({state:'visible'});
    const frame=page.frames().find(f=>f.url().includes('/braccato/index.html'));
    await frame.locator('.blyrics--line').first().click();
    if(mobile){await page.mouse.click(44,68);await iframe.waitFor({state:'hidden'});}
    }
    await submit('Пісня ї-東京');
    for(const query of ['symbols !? 123','another search']) {
      await page.mouse.click(mobile?60:99,height-(mobile?(height<500?84:92):45));
      await page.waitForFunction(()=>window.fixtureAudio.paused);
      await page.mouse.click(mobile?60:99,height-(mobile?(height<500?84:92):45));
      await page.waitForFunction(()=>!window.fixtureAudio.paused);
      await submit(query);
    }
    await page.screenshot({path:fileURLToPath(new URL(`../../target/search-compact-${width}.png`,import.meta.url))});
    assert.deepEqual(errors,[]);
    results.push({width,height,passed:true,queries});
    await context.close();
  }
  await writeFile(new URL('../../target/search-input-check.json',import.meta.url),JSON.stringify(results,null,2));
  console.log(JSON.stringify(results,null,2));
} finally {await browser.close();}
