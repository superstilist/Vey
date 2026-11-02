import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
const out=resolve(process.argv[2]||'../../target/library-import-browser-01');
await mkdir(out,{recursive:false});
const browser=await chromium.launch({channel:'msedge',headless:true});
const track={path:'yt://abcdefghijk',videoId:'abcdefghijk',title:'Public import fixture',artist:'Vey',durationSecs:120};
const link='https://music.youtube.com/playlist?list=PL1234567890';
const playlist={id:'youtube-PL1234567890',name:'Imported fixture',tracks:[track]};
try {
  const context=await browser.newContext({viewport:{width:1440,height:900}});
  const page=await context.newPage();const errors=[];const requests=[];
  page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
  await page.addInitScript(track=>{if(!localStorage.getItem('playervf.rust.v1'))localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,settings:{language:'en'},tracks:[track]}));},track);
  await page.route('**/api/playlists/import',async route=>{
    requests.push(JSON.parse(route.request().postData()));
    await route.fulfill({contentType:'application/json',body:JSON.stringify(playlist)});
  });
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(1000);
  await page.mouse.click(1370,176);await page.waitForTimeout(200);
  await page.screenshot({path:resolve(out,'import-dialog.png')});
  // Dialog uses a centered 430px form, with the playlist field after its intro.
  await page.mouse.click(700,344);
  await page.keyboard.type(link,{delay:4});await page.keyboard.press('Enter');
  await page.waitForFunction(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')||'{}').playlists?.some(p=>p.id==='youtube-PL1234567890'),null,{timeout:8000});
  assert.deepEqual(requests,[link]);
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')));
  assert.equal(saved.playlists.find(p=>p.id===playlist.id).tracks[0],track.path);
  assert.ok(!JSON.stringify(saved).includes('access_token'));
  await page.screenshot({path:resolve(out,'import-complete.png')});
  // Imported content survives a reload without signing in to either account.
  await page.reload();await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(700);
  assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).playlists[0].id),playlist.id);
  await page.mouse.click(1330,32);await page.waitForTimeout(180);
  await page.screenshot({path:resolve(out,'connections-vey.png')});
  await page.mouse.click(700,230);await page.keyboard.press('ArrowDown');await page.keyboard.press('Enter');await page.waitForTimeout(180);
  await page.screenshot({path:resolve(out,'connections-youtube.png')});
  await page.mouse.click(100,100);await page.waitForTimeout(100);
  await page.mouse.click(1326,176);await page.waitForTimeout(150);
  await page.mouse.click(600,380);
  await page.waitForFunction(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings.home_layout===2,null,{timeout:5000});
  await page.screenshot({path:resolve(out,'layout-selected.png')});
  assert.deepEqual(errors,[]);
  await writeFile(resolve(out,'report.json'),JSON.stringify({passed:true,requests,accountRequired:false,layoutSelected:2},null,2));
  console.log('Public playlist import, persistence and no-account flow passed.');
} finally {await browser.close();}
