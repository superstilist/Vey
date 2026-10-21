import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  const page=await browser.newPage({viewport:{width:1440,height:900}});
  const errors=[];
  page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
  const covers=await page.evaluate(()=>['#ff0000','#0000ff'].map(color=>{
    const canvas=document.createElement('canvas');canvas.width=canvas.height=64;
    const ctx=canvas.getContext('2d');ctx.fillStyle=color;ctx.fillRect(0,0,64,64);
    ctx.fillStyle='#ffffff30';ctx.beginPath();ctx.arc(50,10,32,0,Math.PI*2);ctx.fill();
    ctx.fillStyle='#00000030';ctx.fillRect(0,42,64,22);
    ctx.fillStyle='#ffffffcc';ctx.font='bold 8px sans-serif';ctx.fillText('VEY',7,56);
    return canvas.toDataURL('image/png').split(',')[1];
  }));
  const tracks=['abcdefghijk','lmnopqrstuv'].map((id,i)=>({path:`yt://${id}`,videoId:id,
    title:i?'Blue Hour':'A Little More Time',artist:'Vey sessions',durationSecs:30,
    thumbnailUrl:`http://127.0.0.1:4181/material-cover-${i}.png`}));
  await page.addInitScript(tracks=>{
    const NativeAudio=window.Audio;
    window.Audio=class extends NativeAudio { constructor(...args){super(...args);window.materialAudio=this;} };
    localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks,
      likes:[tracks[0].path],history:tracks.map(track=>({track,playedAt:1,playCount:1}))}));
  },tracks);
  await page.route('**/material-cover-*.png',async r=>{
    const index=Number(r.request().url().match(/cover-(\d)/)[1]);
    if(index)await new Promise(resolve=>setTimeout(resolve,800));
    await r.fulfill({contentType:'image/png',body:Buffer.from(covers[index],'base64')});
  });
  const lyrics=[{provider:'Vey fixture',sync_type:'Synced',format:'lrc',lyrics:{offset_ms:0,language:'en',songwriters:[],
    lines:[{start_ms:0,duration_ms:12000,text:'A little more time with you',parts:[]},
      {start_ms:12000,duration_ms:18000,text:'Let the music find its way',parts:[]}]}}];
  await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify(lyrics)}));
  await page.route('**/api/browse*',r=>r.fulfill({contentType:'application/json',body:'[]'}));
  const wav=Buffer.alloc(44+16000*2*30);
  wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);
  wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);
  wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
  await page.route('**/api/audio/*',r=>r.fulfill({contentType:'audio/wav',body:wav}));
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});
  const frame=page.frames().find(f=>f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(()=>!!window.PlayerVFBraccato);
  await frame.evaluate(()=>addEventListener('message',e=>{
    if(e.data?.state?.ambient?.source)window.materialSource=e.data.state.ambient.source;
  }));
  const pixel=async(x=1100,y=16)=>{
    const png=await page.screenshot({clip:{x,y,width:1,height:1}});
    return page.evaluate(async data=>{
      const bitmap=await createImageBitmap(new Blob([Uint8Array.from(atob(data),c=>c.charCodeAt(0))],{type:'image/png'}));
      const c=new OffscreenCanvas(1,1);const ctx=c.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();
      return [...ctx.getImageData(0,0,1,1).data].slice(0,3);
    },png.toString('base64'));
  };
  await page.waitForTimeout(1200);
  const shot=name=>page.screenshot({path:fileURLToPath(new URL(`../../docs/screenshots/${name}.png`,import.meta.url))});
  await shot('41-material-home');
  await page.mouse.click(90,865);
  await frame.waitForFunction(()=>!!window.materialSource);
  await page.waitForTimeout(1200);
  const red=await pixel();const first=await frame.evaluate(()=>window.materialSource);
  assert.ok(red[0]>red[2],`red cover produces a warm tonal surface: ${red}`);
  await page.mouse.click(150,865);
  const loading=await pixel();
  assert.deepEqual(loading,red,'delayed artwork retains the previous palette');
  await frame.waitForFunction(source=>window.materialSource!==source,first);
  await page.waitForTimeout(1300);
  const blue=await pixel();
  assert.ok(blue[2]>blue[0],`blue cover produces a cool tonal surface: ${blue}`);
  await page.mouse.click(450,865);
  await page.waitForTimeout(800);
  await frame.waitForFunction(()=>window.PlayerVFBraccato.diagnostics().lineCount>=2);
  const initialVolume=await page.evaluate(()=>window.materialAudio.volume);
  await page.mouse.click(1204,858);
  await page.waitForTimeout(150);
  const clickedVolume=await page.evaluate(()=>window.materialAudio.volume);
  await shot('42-material-player');
  console.log({initialVolume,clickedVolume});
  assert.ok(clickedVolume<initialVolume,'tonal volume slider controls the actual audio element');
  await page.keyboard.press('ArrowRight');await page.waitForTimeout(100);
  assert.ok(await page.evaluate(v=>window.materialAudio.volume>v,clickedVolume),'tonal slider retains keyboard interaction');
  await shot('42-material-player');
  await page.mouse.click(90,98);
  await page.waitForTimeout(650);
  assert.deepEqual(await pixel(),blue,'navigation preserves the cover palette');
  await page.setViewportSize({width:390,height:844});await page.waitForTimeout(500);
  await shot('43-material-mobile-home');
  await page.mouse.click(180,728);await page.waitForTimeout(700);
  await shot('44-material-mobile-player');
  const phoneBackground=await pixel(20,600);
  assert.ok(phoneBackground[2]>phoneBackground[0]+12,`mobile player retains the blue artwork background: ${phoneBackground}`);
  assert.deepEqual(errors,[]);
  const report={passed:true,checks:['warm/cool cover-derived surfaces','no loading palette flash','palette retained across navigation','volume slider input and keyboard','desktop/mobile screenshots','no browser errors'],red,loading,blue};
  await writeFile(new URL('../../docs/material-check.json',import.meta.url),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
} finally {await browser.close();}
