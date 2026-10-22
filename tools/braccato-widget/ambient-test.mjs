import { chromium } from 'playwright';
import assert from 'node:assert/strict';
const browser = await chromium.launch({ channel: 'msedge', headless: true });
try {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  await page.goto('http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(() => !!window.PlayerVFBraccato);
  await page.evaluate(() => {
    window.ambientReadbacks = 0;
    window.ambientRotations = [];
    const rotate = CanvasRenderingContext2D.prototype.rotate;
    CanvasRenderingContext2D.prototype.rotate = function(angle) {
      window.ambientRotations.push(angle);
      return rotate.call(this, angle);
    };
    const original = WebGLRenderingContext.prototype.readPixels;
    WebGLRenderingContext.prototype.readPixels = function (...args) {
      window.ambientReadbacks++;
      return original.apply(this, args);
    };
  });
  const result = await page.evaluate(() => {
    const frames = [];
    addEventListener('playervf:braccato-message', e => {
      if (e.detail.type === 'ambient') frames.push(e.detail.pixels);
    });
    const render = ambient => window.PlayerVFBraccato.update({ ambient });
    render({ time: 0, source: '' });
    render({ time: 2 });
    render({ time: 2 });
    const art = new Uint8Array(64 * 64 * 4);
    for (let i = 0; i < art.length; i += 4) { art[i] = 230; art[i+1] = 50; art[i+2] = 30; art[i+3] = 255; }
    render({ time: 2, source: btoa(String.fromCharCode(...art)), reduced:true });
    return frames;
  });
  assert.equal(result.length, 4, 'every requested frame crosses the bridge');
  const frames = result.map(s => Buffer.from(s, 'base64'));
  assert.ok(frames.every(f => f.length === 256 * 144 * 4));
  assert.ok(frames[0].some((value, i) => i % 4 !== 3 && value > 10), 'nonblack GPU frame');
  assert.notDeepEqual(frames[0], frames[1], 'domain warp moves with time');
  assert.deepEqual(frames[1], frames[2], 'same time gives a frozen frame');
  assert.notDeepEqual(frames[2], frames[3], 'artwork changes the background');
  assert.equal(await page.evaluate(() => window.ambientReadbacks), 3,
    'duplicate frozen frames reuse the GPU target');
  const rotations = await page.evaluate(() => window.ambientRotations);
  assert.deepEqual(rotations.map(angle => angle || 0), [0, -0.24], 'rotation follows the shared clock; frozen/reduced frames do not rotate');
  assert.ok(frames.every(frame => frame.every((value, index) => index % 4 !== 3 || value === 255)), 'rotation covers every texture corner');
  const backdrop = await page.evaluate(() => {
    const geometry = {content:{x:248,y:64,width:1184,height:744},viewport:{x:776,y:210,width:598,height:524}};
    window.PlayerVFBraccato.update({visible:true,mode:'lyrics',nativeBackdrop:geometry});
    const canvas = document.querySelector('#native-backdrop');
    const read = () => Array.from(canvas.getContext('2d').getImageData(0,0,canvas.width,canvas.height).data);
    const initial = read();
    window.PlayerVFBraccato.update({position:5});
    const steady = read();
    const art = new Uint8Array(64 * 64 * 4);
    for(let i=0;i<art.length;i+=4){art[i+2]=230;art[i+3]=255;}
    window.PlayerVFBraccato.update({ambient:{time:2,source:btoa(String.fromCharCode(...art)),reduced:true}});
    const changed = read();
    window.PlayerVFBraccato.update({nativeBackdrop:{...geometry,viewport:{...geometry.viewport,x:850,width:1000,height:800}}});
    return {initial,steady,changed,resized:read(),background:getComputedStyle(document.body).backgroundColor};
  });
  assert.equal(backdrop.background,'rgb(8, 8, 8)');
  assert.ok(backdrop.initial.every((value,i)=>i%4!==3||value===255),'native surface is fully opaque');
  assert.deepEqual(backdrop.initial,backdrop.steady,'clock samples leave the backdrop unchanged');
  assert.notDeepEqual(backdrop.initial,backdrop.changed,'native backdrop follows the current artwork');
  assert.ok(backdrop.resized.every((value,i)=>i%4!==3||value===255),'resizing leaves no transparent gaps');
  const morph = await page.evaluate(async () => {
    const frames = [];
    addEventListener('playervf:braccato-message', e => {
      if(e.detail.type==='ambient') frames.push(Uint8Array.from(atob(e.detail.pixels),c=>c.charCodeAt(0)));
    });
    const source = (r,g,b) => {
      const art=new Uint8Array(64*64*4);
      for(let i=0;i<art.length;i+=4){art[i]=r;art[i+1]=g;art[i+2]=b;art[i+3]=255;}
      return btoa(String.fromCharCode(...art));
    };
    const render = (image,reduced=false) => {
      window.PlayerVFBraccato.update({ambient:{time:2,animate:false,reduced,...(image===undefined?{}:{source:image})}});
      return frames.at(-1);
    };
    const wait=ms=>new Promise(r=>setTimeout(r,ms));
    const delta=(a,b)=>a.reduce((sum,value,i)=>sum+(i%4===3?0:Math.abs(value-b[i])),0)/(a.length/4*3);
    const red=source(230,30,25),blue=source(20,40,230),green=source(20,220,50);
    const start=render(red,true);
    const first=render(blue);
    const readsBeforeBlend = window.ambientReadbacks;
    await wait(300);
    const early=render();
    await wait(400);
    const middle=render();
    const blendReadbacks = window.ambientReadbacks - readsBeforeBlend;
    const interrupted=render(green);
    await wait(1500);
    const finished=render();
    const frozen=render();
    const missing=render('');
    await wait(100);
    const replacement=render(red);
    await wait(1500);
    render();
    const immediate=render(blue,true);
    const immediateSteady=render(undefined,true);
    return {blendReadbacks,first:delta(start,first),early:delta(start,early),middle:delta(start,middle),
      interrupted:delta(middle,interrupted),finished:delta(middle,finished),frozen:delta(finished,frozen),
      missing:delta(finished,missing),replacement:delta(missing,replacement),
      reduced:delta(immediate,immediateSteady)};
  });
  assert.equal(morph.first,0,'new song starts from exactly the displayed colors');
  assert.equal(morph.blendReadbacks,0,'paused morph frames perform no additional GPU readbacks');
  assert.ok(morph.early>0 && morph.middle>morph.early,'paused artwork morph progresses gradually');
  assert.equal(morph.interrupted,0,'rapid skips continue from the displayed mixture');
  assert.ok(morph.finished>10,'interrupted morph reaches the latest artwork');
  assert.equal(morph.frozen,0,'completed paused background is stable');
  assert.equal(morph.missing,0,'loading cover does not flash the default palette');
  assert.equal(morph.replacement,0,'replacement cover continues without a jump');
  assert.equal(morph.reduced,0,'reduced motion applies the final palette immediately');
  const bright = await page.evaluate(() => {
    let result;
    const receive = event => {
      if (event.detail.type === 'ambient') result = Uint8Array.from(atob(event.detail.pixels), c => c.charCodeAt(0));
    };
    addEventListener('playervf:braccato-message', receive);
    const art = new Uint8Array(64 * 64 * 4).fill(255);
    window.PlayerVFBraccato.update({ambient:{source:btoa(String.fromCharCode(...art)),time:2,reduced:true}});
    removeEventListener('playervf:braccato-message', receive);
    const mean = [0,0,0];
    for (let i=0;i<result.length;i+=4) for (let channel=0;channel<3;channel++) mean[channel]+=result[i+channel]/(result.length/4);
    return mean;
  });
  assert.ok(bright.every(channel => channel > 100 && channel < 180), 'white artwork is dimmed for lyric readability');
  assert.ok(Math.max(...bright)-Math.min(...bright)<2, 'neutral artwork has no added color tint');
  const undimmed=await page.evaluate(()=>{
    let pixels;
    const receive=event=>{if(event.detail.type==='ambient')pixels=Uint8Array.from(atob(event.detail.pixels),c=>c.charCodeAt(0));};
    addEventListener('playervf:braccato-message',receive);
    window.PlayerVFBraccato.update({ambient:{time:2,reduced:true,options:{dimBright:false}}});
    removeEventListener('playervf:braccato-message',receive);
    let sum=0;for(let i=0;i<pixels.length;i+=4)sum+=(pixels[i]+pixels[i+1]+pixels[i+2])/3;
    return sum/(pixels.length/4);
  });
  assert.ok(undimmed>230,'turning dimming off updates the same paused artwork without needing a new cover');
  assert.deepEqual(errors, []);
  console.log('PASS: Kawarp output, native backdrop, smooth paused morphs, rapid skips, loading-cover grace and reduced motion',morph);
} finally { await browser.close(); }
