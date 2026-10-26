import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const browser = await chromium.launch({channel:'msedge',headless:true});
try {
  const page = await browser.newPage();
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(()=>!!window.PlayerVFBraccato);
  await page.evaluate(()=>window.PlayerVFBraccato.update({visible:true,playing:false,position:2.4,timed:true,size:36,
    customTheme:':root{--blyrics-font-size:36px;--blyrics-glow-color:#66ccff;}',fontFamily:'Roboto',
    document:{lyrics:[{startTimeMs:1000,durationMs:5000,words:'Illuminate slowly',parts:[
      {startTimeMs:1000,durationMs:4000,words:'Illuminate'},{startTimeMs:5000,durationMs:1000,words:' slowly'}]}]}}));
  await page.waitForFunction(()=>window.PlayerVFBraccato.diagnostics().lineCount===1);
  const result=await page.evaluate(()=>{
    const view=document.querySelector('braccato-lyrics');
    const api=window.PlayerVFBraccato;const before=api.diagnostics();
    for(let i=0;i<100;i++)api.update({position:2.4});
    const word=view.querySelector('.blyrics--word');
    const style=getComputedStyle(word);
    return {theme:view.theme,font:style.fontFamily,size:style.fontSize,before,after:api.diagnostics()};
  });
  assert.match(result.theme,/36px/);assert.match(result.theme,/66ccff/);assert.match(result.theme,/Roboto/);
  assert.match(result.font,/Roboto/);assert.equal(result.size,'36px');
  assert.equal(result.after.themeCompiles,result.before.themeCompiles,'steady samples must reuse custom theme');
  assert.equal(result.after.builds,result.before.builds,'steady samples must retain lyric models');
  assert.deepEqual(errors,[]);
  console.log('PASS: custom font/theme forwarded and retained across 100 steady updates');
} finally {await browser.close();}
