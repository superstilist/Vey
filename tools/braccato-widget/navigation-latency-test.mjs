import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
if(!process.argv[2]) throw new Error('NEW_OUTPUT_DIRECTORY required');
const output=resolve(process.argv[2]);await mkdir(output);
const browser=await chromium.launch({channel:'msedge',headless:true});
const page=await browser.newPage({viewport:{width:1100,height:760}});
const errors=[];
page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
await page.addInitScript(()=>{
  window.pageReadbacks=0;
  for(const type of [window.WebGLRenderingContext,window.WebGL2RenderingContext]) {
    if(!type)continue;
    const read=type.prototype.readPixels;
    type.prototype.readPixels=function(x,y,w,h,...args){
      if(w===1100&&h===760)window.pageReadbacks++;
      return read.call(this,x,y,w,h,...args);
    };
  }
});
await page.route('**/*',r=>new URL(r.request().url()).hostname==='127.0.0.1'?r.continue():r.abort());
await page.route('**/api/**',r=>r.fulfill({contentType:'application/json',body:'[]'}));
try {
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});
  await page.waitForTimeout(500);
  await page.evaluate(()=>{
    const frame=document.querySelector('iframe');
    const send=frame.contentWindow.postMessage.bind(frame.contentWindow);
    frame.contentWindow.postMessage=function(data,...args){
      if(data?.channel==='playervf-braccato-update'){
        window.hostState={...window.hostState,...data.state};
        const r=window.navRequest;
        if(r&&window.hostState.navigation?.view===r.view){
          if(window.hostState.camera&&!r.first){r.first=performance.now()-r.start;r.duration=window.hostState.camera.duration;}
          if(!window.hostState.camera&&r.first&&!r.settled)r.settled=performance.now()-r.start;
        }
      }
      return send(data,...args);
    };
    window.pageReadbacks=0;
  });
  const samples=[];
  for(const [view,y] of [['library',194],['settings',290],['home',98],['liked',242],['settings',290],['home',98]]){
    await page.evaluate(view=>window.navRequest={view,start:performance.now()},view);
    await page.mouse.click(90,y);
    await page.waitForFunction(()=>window.navRequest?.settled,undefined,{timeout:5000});
    const sample=await page.evaluate(()=>window.navRequest);
    assert.ok(sample.duration<=180,JSON.stringify(sample));
    samples.push(sample);
  }
  assert.equal(await page.evaluate(()=>window.pageReadbacks),0,'navigation performs no full-window readbacks');
  // Retargeting must finish on the latest route.
  await page.mouse.click(90,194);
  await page.waitForTimeout(40);
  await page.mouse.click(90,290);
  await page.waitForFunction(()=>window.hostState.navigation?.view==='settings'&&!window.hostState.camera);
  await page.screenshot({path:resolve(output,'desktop-settings.png')});
  const sectionStart=await page.evaluate(()=>performance.now());
  await page.mouse.click(650,328); // Appearance in the 1100x760 centered column
  await page.waitForFunction(()=>window.hostState.navigation?.section==='appearance',undefined,{timeout:1500});
  const settingsObservedMs=await page.evaluate(start=>performance.now()-start,sectionStart);
  await page.screenshot({path:resolve(output,'settings-section.png')});
  await page.setViewportSize({width:390,height:760});
  await page.waitForTimeout(350);
  await page.mouse.click(28,724); // retained page -> Home
  await page.waitForTimeout(250);
  await page.screenshot({path:resolve(output,'mobile.png')});
  assert.deepEqual(errors,[]);
  const report={passed:true,samples,settingsObservedMs,readbacks:await page.evaluate(()=>window.pageReadbacks),
    checks:['short live camera','no page readbacks','rapid retarget','settings section','mobile layout','no browser errors']};
  await writeFile(resolve(output,'report.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
}catch(error){await page.screenshot({path:resolve(output,'failure.png')});throw error;}
finally{await browser.close();}
