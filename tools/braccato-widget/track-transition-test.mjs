import {chromium} from 'playwright';
import assert from 'node:assert/strict';

const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  const page=await browser.newPage({viewport:{width:1440,height:900}});
  const tracks=['abcdefghijk','lmnopqrstuv'].map((id,i)=>({path:`yt://${id}`,videoId:id,
    title:`${i+1} Color test`,artist:'PlayerVF',durationSecs:30,
    thumbnailUrl:`http://127.0.0.1:4181/cover-${i}.png`}));
  await page.addInitScript(tracks=>localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks,
    history:tracks.map(track=>({track,playedAt:1,playCount:1}))})),tracks);
  const lyrics=[{provider:'Test',sync_type:'RichSync',format:'lrc',lyrics:{offset_ms:0,language:'en',songwriters:[],
    lines:[{start_ms:0,duration_ms:30000,text:'Color test',parts:[{start_ms:0,duration_ms:30000,text:'Color test',is_background:false}]}]}}];
  await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify(lyrics)}));
  const wav=Buffer.alloc(44+16000*2*30);
  wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);
  wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);
  wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
  await page.route('**/api/audio/*',r=>r.fulfill({contentType:'audio/wav',body:wav}));
  await page.goto('http://127.0.0.1:4181/');
  const covers=await page.evaluate(()=>['#e62020','#2040e6'].map(color=>{
    const c=document.createElement('canvas');c.width=c.height=64;
    const ctx=c.getContext('2d');ctx.fillStyle=color;ctx.fillRect(0,0,64,64);
    return c.toDataURL('image/png').split(',')[1];
  }));
  await page.route('**/cover-*.png',async r=>{
    const i=Number(r.request().url().match(/cover-(\d)/)[1]);
    if(i) await new Promise(resolve=>setTimeout(resolve,300));
    await r.fulfill({contentType:'image/png',body:Buffer.from(covers[i],'base64')});
  });
  await page.locator('iframe[title="Braccato lyrics"]').waitFor({state:'attached'});
  const frame=page.frames().find(f=>f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(()=>!!window.PlayerVFBraccato);
  await frame.evaluate(()=>{
    window.framesSent=0;
    addEventListener('message',e=>{
      if(e.data?.channel!=='playervf-braccato-update')return;
      window.hostState={...window.hostState,...e.data.state};
      if(e.data.state.ambient?.source!==undefined)window.artSource=e.data.state.ambient.source;
    });
    addEventListener('playervf:braccato-message',e=>{
      if(e.detail.type==='ambient'){window.framesSent++;window.lastPixels=e.detail.pixels;}
    });
  });
  await page.mouse.click(90,865);
  await frame.waitForFunction(()=>!!window.artSource);
  await page.mouse.click(450,865);
  await frame.waitForFunction(()=>window.hostState.playing===true);
  const firstSource=await frame.evaluate(()=>window.artSource);
  await page.waitForTimeout(2800);
  await page.mouse.click(90,865);
  await frame.waitForFunction(()=>window.hostState.playing===false);
  await page.mouse.click(150,865);
  await frame.waitForFunction(()=>window.hostState.playing===true);
  await page.mouse.click(90,865);
  await frame.waitForFunction(previous=>!!window.artSource && window.artSource!==previous,firstSource);
  await frame.waitForFunction(()=>window.hostState.playing===false);
  const start=await frame.evaluate(()=>({count:window.framesSent,pixels:window.lastPixels}));
  await page.waitForTimeout(800);
  const middle=await frame.evaluate(()=>({count:window.framesSent,pixels:window.lastPixels}));
  assert.ok(middle.count>start.count+5,'paused host continues scheduling the color morph');
  assert.notEqual(start.pixels,middle.pixels,'paused next-song palette progresses');
  await page.waitForTimeout(2200);
  const settled=await frame.evaluate(()=>window.framesSent);
  await page.waitForTimeout(400);
  assert.equal(await frame.evaluate(()=>window.framesSent),settled,'completed paused morph stops GPU/IPC frames');
  console.log('PASS: real Rust WASM next-track artwork morph continues while paused and stops when settled');
} finally {await browser.close();}
