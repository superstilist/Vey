import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';

const browser = await chromium.launch({channel:'msedge',headless:true});
try {
  const page = await browser.newPage({viewport:{width:1100,height:760}});
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  await page.addInitScript(() => {
    window.navigationReadbacks = []; window.navigationUploads = [];
    for (const type of [window.WebGLRenderingContext, window.WebGL2RenderingContext]) {
      if (!type) continue;
      const read = type.prototype.readPixels;
      type.prototype.readPixels = function(x,y,width,height,...args) {
        window.navigationReadbacks.push([width,height]);
        return read.call(this,x,y,width,height,...args);
      };
      const upload = type.prototype.texImage2D;
      type.prototype.texImage2D = function(...args) {
        if (typeof args[3] === 'number' && typeof args[4] === 'number')
          window.navigationUploads.push([args[3],args[4]]);
        return upload.apply(this,args);
      };
    }
  });
  await page.route('**/api/**', r => r.fulfill({contentType:'application/json',body:'[]'}));
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe[title="Braccato lyrics"]').waitFor({state:'attached'});
  const frame = page.frames().find(f => f.url().includes('/braccato/index.html'));
  await frame.waitForFunction(() => !!window.PlayerVFBraccato);
  await frame.evaluate(() => {
    window.cameras = [];
    addEventListener('message', event => {
      if(event.data?.channel !== 'playervf-braccato-update') return;
      window.hostState = {...window.hostState,...event.data.state};
      if(event.data.state.camera) window.cameras.push(event.data.state.camera);
    });
  });
  // The initial host message may precede this listener; settled duplicate
  // samples are intentionally suppressed. Read the already-applied DOM layout.
  await page.waitForFunction(() => document.querySelector('iframe[title="Braccato lyrics"]')?.parentElement.style.borderRadius === '28px');
  const radius = () => page.locator('iframe[title="Braccato lyrics"]').evaluate(el => getComputedStyle(el.parentElement).borderRadius);
  assert.equal(await radius(), '28px', 'desktop host shares the Slint content radius');
  await page.evaluate(() => { window.navigationReadbacks = []; window.navigationUploads = []; });
  await page.mouse.click(90,194);
  await frame.waitForFunction(() => window.cameras.some(c => c.toX===1 && c.toY===1));
  await page.waitForFunction(() => window.navigationUploads.filter(([w,h]) => w===768 && h<=640).length >= 2);
  assert.ok(await page.evaluate(() => window.navigationReadbacks.filter(([w,h]) => w===1100 && h===760).length >= 2), 'WASM captures both page textures');
  assert.equal(await page.locator('canvas').evaluate(el => getComputedStyle(el).filter), 'none', 'chrome stays sharp');
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/navigation-web-diagonal.png',import.meta.url))});
  await page.mouse.click(90,290);
  await frame.waitForFunction(() => window.hostState?.navigation?.view === 'settings' && !window.hostState.camera);
  assert.ok((await frame.evaluate(() => window.cameras)).every(c => [c.fromX,c.fromY,c.toX,c.toY,c.elapsed].every(Number.isFinite)));
  await page.setViewportSize({width:390,height:760});
  await frame.waitForFunction(() => window.hostState?.mobile && window.hostState.stageBounds.radius === 20);
  assert.equal(await radius(), '20px', 'mobile host shares the Slint content radius');
  await page.screenshot({path:fileURLToPath(new URL('../../docs/screenshots/navigation-web-mobile.png',import.meta.url))});
  assert.deepEqual(errors.filter(e => !e.includes('Using exceptions for control flow')), []);
  console.log('PASS WASM navigation: diagonal camera, retarget, sharp chrome, desktop/mobile clipping, no runtime errors');
} finally { await browser.close(); }
