import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  const page=await browser.newPage({viewport:{width:1440,height:900}});
  await page.route('**/api/**',r=>r.fulfill({contentType:'application/json',body:'[]'}));
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});
  const frame=page.frames().find(f=>f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(()=>!!window.PlayerVFBraccato);
  await page.waitForTimeout(1600);
  await frame.evaluate(()=>{
    window.cameraTargets=[];
    addEventListener('message',e=>{
      if(e.data?.channel==='playervf-braccato-update'&&e.data.state.camera)
        window.cameraTargets.push(e.data.state.camera);
    });
  });
  await page.mouse.move(46,774);await page.mouse.down();
  await page.mouse.move(18,802,{steps:8});await page.mouse.up();
  await page.waitForTimeout(700);
  const cameras=await frame.evaluate(()=>window.cameraTargets);
  assert.ok(cameras.some(c=>c.toX===1&&c.toY===-1),JSON.stringify(cameras));
  console.log('PASS: diagonal gesture stays within the compact navigation pad');
} finally {await browser.close();}
