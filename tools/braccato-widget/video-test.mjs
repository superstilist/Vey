import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {readFile, mkdir, writeFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const fixture=fileURLToPath(new URL('../../target/video-surface-fixture.mp4',import.meta.url));
execFileSync('ffmpeg',['-y','-v','error','-f','lavfi','-i','testsrc2=size=160x90:rate=24','-t','4','-c:v','libx264','-pix_fmt','yuv420p','-an','-movflags','+faststart',fixture]);
const body=await readFile(fixture);
const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  const page=await browser.newPage({viewport:{width:800,height:600}});
  await page.setContent('<canvas width="800" height="600"></canvas>');
  await page.addScriptTag({content:(await readFile(new URL('../../web/video-surface.js',import.meta.url),'utf8')).replaceAll('export ','')+';window.testSurface=new VideoSurface();'});
  const src='data:video/mp4;base64,'+body.toString('base64');
  await page.evaluate(url=>{
    window.state={url,position:0,rate:1,playing:false,visible:true,native:true,bounds:{x:10,y:20,width:320,height:180}};
    testSurface.update(state);
  },src);
  await page.waitForFunction(()=>testSurface.video.readyState>=2);
  await page.evaluate(()=>testSurface.update(state));
  assert.equal(await page.locator('video').evaluate(v=>v.muted),true);
  assert.equal(await page.locator('video').evaluate(v=>v.paused),true);
  assert.equal(await page.locator('video').evaluate(v=>v.style.display),'block');
  await page.evaluate(()=>{state.position=1.5;testSurface.update(state);});
  await page.waitForFunction(()=>Math.abs(testSurface.video.currentTime-1.5)<0.06);
  await page.evaluate(()=>{state.playing=true;state.rate=1.25;testSurface.update(state);});
  await page.waitForFunction(()=>!testSurface.video.paused&&testSurface.video.currentTime>1.6);
  assert.equal(await page.locator('video').evaluate(v=>v.playbackRate),1.25);
  await page.evaluate(()=>{state.position=2;state.playing=false;testSurface.update(state);});
  await page.waitForFunction(()=>testSurface.video.paused&&Math.abs(testSurface.video.currentTime-2)<0.06);
  await page.evaluate(()=>{state.visible=false;testSurface.update(state);});
  assert.equal(await page.locator('video').evaluate(v=>v.style.display),'none');
  await page.evaluate(()=>{state.visible=true;state.url='data:video/mp4;base64,invalid';testSurface.update(state);});
  await page.waitForFunction(()=>testSurface.failed);
  assert.equal(await page.locator('video').evaluate(v=>v.style.display),'none');
  await page.evaluate(()=>{testSurface.dispose();});
  assert.equal(await page.locator('video').count(),0);
  const report={passed:true,scenarios:['muted','paused frame','seek','play','rate','pause','hidden suspension','error fallback','resource disposal']};
  await writeFile(new URL('../../docs/video-surface-check.json',import.meta.url),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report));
} finally {await browser.close();}

