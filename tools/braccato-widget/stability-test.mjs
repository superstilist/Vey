import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const browser = await chromium.launch({channel:'msedge',headless:true});
try {
  const page = await browser.newPage();
  const errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.goto('http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(()=>!!window.PlayerVFBraccato);
  await page.evaluate(()=>{
    window.messages=[];
    addEventListener('playervf:braccato-message',e=>window.messages.push(e.detail));
    window.PlayerVFBraccato.update({visible:true,playing:true,position:1.1,rate:1,offset:0,
      document:{lyrics:[{startTimeMs:0,durationMs:5000,words:'The music keeps moving',parts:[
        {startTimeMs:0,durationMs:2000,words:'The music '},
        {startTimeMs:2000,durationMs:3000,words:'keeps moving'}]}]}});
  });
  await page.waitForTimeout(350);
  assert.ok(await page.evaluate(()=>window.PlayerVFBraccato.diagnostics().currentTime>1.36),
    'a late audio sample does not freeze animations after 250ms');
  const result=await page.evaluate(()=>{
    const api=window.PlayerVFBraccato;
    const before=api.diagnostics();
    api.update({document:'{broken'});
    api.update({document:'{broken'});
    const after=api.diagnostics();
    api.update({playing:false,position:2});
    return {before:before.builds,after:after.builds,lines:after.lineCount,
      errors:window.messages.filter(m=>m.type==='error').length};
  });
  assert.equal(result.before,result.after,'malformed documents keep the current renderer');
  assert.equal(result.lines,1);
  assert.equal(result.errors,2,'failed input remains retryable');
  await page.waitForTimeout(150);
  assert.equal(await page.evaluate(()=>window.PlayerVFBraccato.diagnostics().currentTime),2);
  assert.deepEqual(errors,[]);
  console.log('PASS: delayed samples, malformed-document recovery, preserved lyrics and pause');
} finally { await browser.close(); }
