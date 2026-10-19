import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const browser = await chromium.launch({channel:'msedge',headless:true});
try {
  const page = await browser.newPage({viewport:{width:720,height:600}});
  await page.route('**/renderer-recovery-test', route => route.fulfill({
    contentType:'text/html',body:'<style>body{margin:0}canvas{width:720px;height:600px}</style><canvas></canvas>'
  }));
  let blocked = true;
  await page.route('**/braccato/widget.js', route => blocked ? route.abort() : route.continue());
  await page.goto('http://127.0.0.1:4181/renderer-recovery-test');
  await page.evaluate(async () => {
    window.bridge = await import('/bridge.js');
    window.fixture = {visible:true,playing:false,position:2,rate:1,offset:0,
      mode:'lyrics',size:48,timed:true,bounds:{x:0,y:0,width:720,height:600},
      windowWidth:720,windowHeight:600,document:JSON.stringify({lyrics:[
        {startTimeMs:0,durationMs:4000,words:'Braccato is ready',parts:[
          {startTimeMs:0,durationMs:1500,words:'Braccato '},
          {startTimeMs:1500,durationMs:2500,words:'is ready'}]}
      ]})};
    window.bridge.updateBraccato(JSON.stringify(window.fixture));
  });
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.bridge.updateBraccato(JSON.stringify(window.fixture))),false);
  blocked = false;
  await page.evaluate(() => {
    window.bridge.resetBraccato();
    window.bridge.updateBraccato(JSON.stringify(window.fixture));
  });
  await page.waitForFunction(() => window.bridge.updateBraccato(JSON.stringify(window.fixture)));
  assert.equal(await page.locator('iframe').count(),1,'retry replaces the failed host');
  const frame = page.frames().find(f => f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(() => window.PlayerVFBraccato?.diagnostics().status === 'rendering');
  assert.ok(await frame.locator('.blyrics--letter').count() > 0,'real Braccato word effects recover');
  console.log('PASS: failed Braccato bootstrap recovers to the real renderer after retry');
} finally { await browser.close(); }
