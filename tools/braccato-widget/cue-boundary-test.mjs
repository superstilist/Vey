import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const browser=await chromium.launch({channel:'msedge',headless:true});
try {
  const page=await browser.newPage();
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(()=>!!window.PlayerVFBraccato);
  const update=state=>page.evaluate(state=>window.PlayerVFBraccato.update(state),state);
  const active=()=>page.locator('.blyrics--line').evaluateAll(lines=>lines.map(line=>line.classList.contains('blyrics--active')));
  await update({visible:true,playing:false,position:2.8,timed:true,stage:false,
    scrollOffset:0,lookahead:0,endingThreshold:0,document:{options:{syncType:'richsync'},lyrics:[
      {startTimeMs:1000,durationMs:2000,words:'Finish this',parts:[
        {startTimeMs:1000,durationMs:1000,words:'Finish '},{startTimeMs:2000,durationMs:1000,words:'this'}]},
      {startTimeMs:3000,durationMs:2000,words:'Next line',parts:[
        {startTimeMs:3000,durationMs:1000,words:'Next '},{startTimeMs:4000,durationMs:1000,words:'line'}]}
    ]}});
  await page.waitForTimeout(120);
  assert.deepEqual(await active(),[true,false],'default camera must retain the unfinished source');
  assert.equal(await page.locator('.blyrics-container').evaluate(el=>getComputedStyle(el).getPropertyValue('--blyrics-scroll-timing-offset').trim()),'0ms');
  await update({position:3.01});await page.waitForTimeout(50);
  assert.deepEqual(await active(),[false,true],'next source starts at its supplied cue');
  await update({position:2.8,scrollOffset:500});await page.waitForTimeout(50);
  assert.deepEqual(await active(),[false,true],'explicit camera offset remains supported');
  assert.deepEqual(errors,[]);
  console.log('PASS: browser camera waits for cue by default and retains explicit offset support');
} finally {await browser.close();}
