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
  const mutations=await page.evaluate(async()=>{
    const api=window.PlayerVFBraccato;
    api.update({presentationOpacity:1,presentationX:0,presentationY:0});
    const scroller=document.querySelector('#scroller');
    let changes=0;
    const observer=new MutationObserver(records=>changes+=records.length);
    observer.observe(scroller,{attributes:true});
    for(let i=0;i<100;i++)api.update({position:2+i/100,playing:false});
    await Promise.resolve();observer.disconnect();
    return changes;
  });
  assert.equal(mutations,0,'steady clock updates do not rewrite scroller styles or visibility');
  const camera = await page.evaluate(async()=>{
    const api=window.PlayerVFBraccato, scroller=document.querySelector('#scroller');
    const position=()=>new DOMMatrixReadOnly(getComputedStyle(scroller).transform).m41;
    const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
    const before=api.diagnostics().builds;
    const path={epoch:1,elapsed:0,duration:380,fromX:0,fromY:0,toX:1,toY:-1,width:1000,height:700};
    api.update({camera:path,presentationX:1000,presentationY:-700});
    const start=position();
    const duration=scroller.getAnimations()[0].effect.getTiming().duration;
    await wait(150);
    const middle=position();
    // Repeated bridge updates must not restart the compositor's animation.
    for(let i=0;i<20;i++)api.update({camera:{...path,elapsed:150},position:2});
    await wait(360);
    const end=position();
    api.update({camera:{...path,epoch:2,fromX:1,fromY:-1,toX:0,toY:0},presentationX:0,presentationY:0});
    await wait(80);
    api.update({camera:null,reduced:true,presentationX:0,presentationY:0});
    const reduced=position();
    return {start,middle,end,reduced,duration,builds:api.diagnostics().builds-before,
      animations:scroller.getAnimations().length};
  });
  assert.ok(camera.start>900 && camera.middle>0 && camera.middle<camera.start-100,
    'camera advances smoothly without bridge samples while playback is paused');
  assert.equal(camera.duration,380,'the lyric host uses the supplied camera duration');
  assert.ok(Math.abs(camera.end)<1,'duplicate samples do not restart camera travel');
  assert.equal(camera.reduced,0,'reduced motion cancels camera travel immediately');
  assert.equal(camera.animations,0,'camera animation is released after cancellation');
  assert.equal(camera.builds,0,'camera movement preserves the lyric renderer');
  const grouped = await page.evaluate(()=>{
    const api=window.PlayerVFBraccato;
    api.update({grouped:true,reduced:false,presentationX:700,presentationY:-400,
      camera:{epoch:3,elapsed:0,fromX:0,fromY:0,toX:1,toY:-1,width:1000,height:700}});
    const scroller=document.querySelector('#scroller');
    return {transform:getComputedStyle(scroller).transform,animations:scroller.getAnimations().length};
  });
  assert.equal(grouped.transform,'none','grouped host movement leaves lyric layout untransformed');
  assert.equal(grouped.animations,0,'grouped movement has no duplicate inner camera animation');
  assert.deepEqual(errors,[]);
  console.log('PASS: delayed samples, preserved lyrics, pause and zero scroller mutations across 100 steady updates');
} finally { await browser.close(); }
