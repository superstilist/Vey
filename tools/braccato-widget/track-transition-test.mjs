import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  const page=await browser.newPage({viewport:{width:1440,height:900}});
  const tracks=['abcdefghijk','lmnopqrstuv'].map((id,i)=>({path:`yt://${id}`,videoId:id,
    title:`${i+1} Color test`,artist:'PlayerVF',durationSecs:30,
    thumbnailUrl:`http://127.0.0.1:4181/cover-${i}.png`}));
  await page.addInitScript(tracks=>localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,tracks,
    history:tracks.map(track=>({track,playedAt:1,playCount:1})),playlists:[{id:'motion',name:'Motion playlist',tracks:tracks.map(track=>track.path)}]})),tracks);
  const lyrics=[{provider:'Test',sync_type:'RichSync',format:'lrc',lyrics:{offset_ms:0,language:'en',songwriters:[],
    lines:[{start_ms:0,duration_ms:30000,text:'Color test',parts:[{start_ms:0,duration_ms:30000,text:'Color test',is_background:false}]}]}}];
  await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify(lyrics)}));
  await page.route('**/api/search*',r=>r.fulfill({contentType:'application/json',body:'[]'}));
  await page.route('**/api/browse*',r=>r.fulfill({contentType:'application/json',body:'[]'}));
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
  const pixel={x:1408,y:780,width:1,height:1};
  const background=await page.screenshot({clip:pixel});
  for(const [name,y] of [['liked',242],['library',194],['settings',290],['home',98],['explore',146]]) {
    await page.mouse.click(90,y);
    await page.waitForTimeout(450);
    let routePixel=await page.screenshot({clip:pixel});
    for(let attempt=0;!routePixel.equals(background)&&attempt<20;attempt++){
      await page.waitForTimeout(100);routePixel=await page.screenshot({clip:pixel});
    }
    assert.deepEqual(routePixel,background,`${name} uses the same background and shading as the player`);
    assert.equal(await frame.evaluate(()=>window.framesSent),settled,`${name} does not restart a paused background`);
  }
  const brightCorner=async()=>{
    const png=await page.screenshot({clip:{x:250,y:65,width:800,height:160}});
    return page.evaluate(async data=>{
      const bytes=Uint8Array.from(atob(data),c=>c.charCodeAt(0));
      const bitmap=await createImageBitmap(new Blob([bytes],{type:'image/png'}));
      const canvas=new OffscreenCanvas(bitmap.width,bitmap.height);
      const ctx=canvas.getContext('2d');ctx.drawImage(bitmap,0,0);bitmap.close();
      const pixels=ctx.getImageData(0,0,canvas.width,canvas.height).data;
      let left=canvas.width,top=canvas.height,count=0;
      for(let y=0;y<canvas.height;y++)for(let x=0;x<canvas.width;x++){
        const i=(y*canvas.width+x)*4;
        if(Math.min(pixels[i],pixels[i+1],pixels[i+2])>110){left=Math.min(left,x);top=Math.min(top,y);count++;}
      }
      return {x:left,y:top,count};
    },png.toString('base64'));
  };
  const click=(x,y)=>()=>page.mouse.click(x,y);
  const cases=[
    ['east',1,0,click(90,146)],['southeast',1,1,click(90,194)],
    ['south',0,1,click(90,242)],['southwest',-1,1,click(90,410)],
    ['west',-1,0,click(1046,865)],
    ['northwest',-1,-1,async()=>{await page.mouse.click(450,31);await page.keyboard.type('Color');await page.keyboard.press('Enter');}],
    ['north',0,-1,click(90,290)],
  ];
  const directions=[];
  await frame.evaluate(()=>{
    window.stageCameras=[];
    addEventListener('message',e=>{
      const camera=e.data?.channel==='playervf-braccato-update'&&e.data.state.camera;
      if(camera)window.stageCameras.push(camera);
    });
  });
  for(const [name,x,y,open] of cases){
    await page.mouse.click(90,98);await page.waitForTimeout(1000);
    const before=await brightCorner();
    await frame.evaluate(()=>{window.stageCameras=[];});
    await open();
    const samples=[];
    for(let attempt=0;attempt<10;attempt++){
      await page.waitForTimeout(16);
      samples.push(await brightCorner());
    }
    await page.waitForTimeout(1000);
    const target=await brightCorner();
    const follows=(sample,origin,sign)=>sample.count>150
      &&(x===0?Math.abs(sample.x-origin.x)<4:(sample.x-origin.x)*x*sign>1.5)
      &&(y===0?Math.abs(sample.y-origin.y)<4:(sample.y-origin.y)*y*sign>1.5);
    // Neighboring pages now enter the crop too. Verify the world camera and
    // real intermediate paint rather than assuming only one page has text.
    const cameras=await frame.evaluate(()=>window.stageCameras);
    assert.ok(cameras.some(c=>c.toX===x&&c.toY===y&&c.elapsed<c.duration),`${name} camera targets the correct world cell: ${JSON.stringify(cameras)}`);
    const intermediate=samples.some(sample=>sample.count>150&&
      (Math.abs(sample.x-before.x)>4||Math.abs(sample.y-before.y)>4||Math.abs(sample.count-before.count)>300)&&
      (Math.abs(sample.x-target.x)>4||Math.abs(sample.y-target.y)>4||Math.abs(sample.count-target.count)>300));
    const moved=intermediate||samples.some(sample=>follows(sample,before,-1)||follows(sample,target,1));
    assert.ok(moved,`${name} navigation follows its compass vector: ${JSON.stringify({before,target,samples})}`);
    directions.push(name);
  }
  await page.mouse.click(90,98);await page.waitForTimeout(1000);
  await frame.evaluate(()=>{window.playerEntries=[];addEventListener('message',e=>{
    if(e.data?.channel==='playervf-braccato-update'&&e.data.state.visible)
      window.playerEntries.push({x:e.data.state.presentationX,y:e.data.state.presentationY,
        grouped:e.data.state.grouped,innerTransform:getComputedStyle(document.querySelector('#scroller')).transform});
  });});
  // Sample while the camera runs; querying only after a cross-frame wait can
  // miss the entire animation on a busy compilation host.
  await page.locator('iframe[title="Braccato lyrics"]').evaluate(element=>{
    window.playerMotionSamples=[];window.capturePlayerMotion=true;
    const sample=()=>{
      const animations=element.getAnimations();
      window.playerMotionSamples.push({animations:animations.length,easing:animations[0]?.effect.getTiming().easing});
      if(window.capturePlayerMotion)requestAnimationFrame(sample);
    };
    requestAnimationFrame(sample);
  });
  await page.mouse.click(450,865);
  await frame.waitForFunction(()=>window.playerEntries.some(p=>p.x>0&&p.y<0));
  const movingHost = await page.locator('iframe[title="Braccato lyrics"]').evaluate(element=>{
    window.capturePlayerMotion=false;
    const samples=window.playerMotionSamples;
    return {animations:Math.max(0,...samples.map(s=>s.animations)),
      easing:samples.find(s=>s.animations)?.easing,
      clipped:getComputedStyle(element.parentElement).overflow};
  });
  assert.equal(movingHost.animations,1,'one camera animation moves the complete lyric host');
  assert.equal(movingHost.easing,'cubic-bezier(0.42, 0, 0.58, 1)','the browser host uses the shared ease-in-out curve');
  assert.equal(movingHost.clipped,'hidden','the shared stage clips the moving host');
  directions.push('northeast');
  await page.waitForTimeout(500);
  const cameraSamples = await frame.evaluate(()=>window.playerEntries);
  assert.ok(cameraSamples.some(p=>p.x>100&&p.y < -100),
    'player travels through world space rather than a short entrance offset');
  assert.ok(cameraSamples.every(p=>p.grouped && p.innerTransform==='none'),
    'the lyric viewport moves as a group without translating its scrolling contents twice');
  assert.ok(Math.abs(cameraSamples.at(-1).x)<1&&Math.abs(cameraSamples.at(-1).y)<1,
    'camera arrives exactly at the player world position');
  assert.deepEqual(await page.locator('iframe[title="Braccato lyrics"]').boundingBox(),{x:776,y:210,width:598,height:524},'diagonal player motion leaves embedded bounds fixed');
  assert.equal(new Set(directions).size,8);
  for(const y of [290,242,194,98]){await page.mouse.click(90,y);await page.waitForTimeout(35);}
  await page.waitForTimeout(900);
  assert.deepEqual(await page.screenshot({clip:pixel}),background,'rapid navigation keeps the background stable');
  await page.mouse.click(90,865);
  await frame.waitForFunction(previous=>window.framesSent>previous+3,settled);
  const exploring=await frame.evaluate(()=>window.framesSent);
  await page.mouse.click(90,290);
  await frame.waitForFunction(previous=>window.framesSent>previous+3,exploring);
  await page.evaluate(()=>{
    window.navigationFrameGaps=[];window.measureNavigation=true;
    let previous;
    const sample=time=>{
      if(previous!==undefined)window.navigationFrameGaps.push(time-previous);
      previous=time;
      if(window.measureNavigation)requestAnimationFrame(sample);
    };
    requestAnimationFrame(sample);
  });
  for(const y of [98,194,98,290,98,242,98,146]){
    await page.mouse.click(90,y);await page.waitForTimeout(240);
  }
  await page.waitForTimeout(600);
  const performance=await page.evaluate(()=>{
    window.measureNavigation=false;
    const gaps=window.navigationFrameGaps.sort((a,b)=>a-b);
    return {samples:gaps.length,medianMs:gaps[Math.floor(gaps.length/2)],p95Ms:gaps[Math.floor(gaps.length*.95)],maxMs:gaps.at(-1)};
  });
  assert.ok(performance.samples>10,'animation frames continue through rapid page changes');
  await writeFile(new URL('../../docs/navigation-performance.json',import.meta.url),JSON.stringify({environment:'headless Edge, Rust WASM app',performance},null,2)+'\n');
  console.log('PASS: next-song morphs, shared background, all eight slide directions, stable player viewport, rapid navigation and continuous background animation',directions);
  console.log('Navigation frame timings:',performance);
  // The small 3x3 pad maps directly to the nine fixed stage cells.
  for(const [column,row] of [[-1,-1],[0,-1],[1,-1],[-1,0],[0,0],[1,0],[-1,1],[0,1],[1,1]]){
    await frame.evaluate(()=>{window.stageCameras=[];});
    await page.mouse.click(20+(column+1)*16,776+(row+1)*16);
    await page.waitForTimeout(700);
    assert.ok((await frame.evaluate(()=>window.stageCameras)).some(c=>c.toX===column&&c.toY===row),`stage pad reaches ${column},${row}`);
  }
  await page.mouse.click(36,792);await page.waitForTimeout(650);
  await frame.evaluate(()=>{window.stageCameras=[];});
  await page.mouse.move(36,792);await page.mouse.down();
  await page.mouse.move(48,804,{steps:4});await page.mouse.up();
  await page.waitForTimeout(500);
  assert.equal((await frame.evaluate(()=>window.stageCameras)).length,0,'small diagonal movement stays within the current cell');
  await frame.evaluate(()=>{window.stageCameras=[];});
  await page.mouse.move(36,792);await page.mouse.down();
  await page.mouse.move(16,792,{steps:5});await page.mouse.up();
  await page.waitForTimeout(650);
  assert.ok((await frame.evaluate(()=>window.stageCameras)).some(c=>c.toX===1&&c.toY===0),'a horizontal swipe moves one page east');
  await page.mouse.click(36,792);await page.waitForTimeout(650);
  await frame.evaluate(()=>{window.stageCameras=[];});
  await page.mouse.move(36,792);await page.mouse.down();
  await page.mouse.move(36,772,{steps:5});await page.mouse.up();
  await page.waitForTimeout(650);
  assert.ok((await frame.evaluate(()=>window.stageCameras)).some(c=>c.toX===0&&c.toY===1),'a vertical swipe moves one page south');
  await page.mouse.click(36,792);await page.waitForTimeout(650);
  await frame.evaluate(()=>{window.stageCameras=[];});
  await page.mouse.move(46,774);await page.mouse.down();
  await page.mouse.move(18,802,{steps:8});await page.mouse.up();
  await page.waitForTimeout(650);
  assert.ok((await frame.evaluate(()=>window.stageCameras)).some(c=>c.toX===1&&c.toY===-1),'a short diagonal gesture navigates to the player');
  await page.mouse.click(90,290);await page.waitForTimeout(650);
  await page.mouse.move(1000,550);await page.mouse.wheel(0,650);await page.waitForTimeout(200);
  const scrolled=await brightCorner();
  await page.mouse.click(90,98);await page.waitForTimeout(650);
  await page.mouse.click(90,290);await page.waitForTimeout(650);
  assert.deepEqual(await brightCorner(),scrolled,'mounted pages preserve their scroll state');
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/40-nine-page-stage.png',import.meta.url))});
  console.log('PASS: nine-cell navigator, diagonal mini gesture, preserved page scroll state');
} finally {await browser.close();}
