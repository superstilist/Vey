import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const out = fileURLToPath(new URL('../../docs/screenshots', import.meta.url));
const resultPath = fileURLToPath(new URL('../../docs/braccato-widget-check.json', import.meta.url));
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const page = await browser.newPage({ viewport: {width: 720,height: 780},deviceScaleFactor:1 });
const errors = [];
page.on('pageerror', e => errors.push(e.message));
const allRequests = [];
page.on('request', r => allRequests.push(r.url()));
const doc = {
  options: {language:'en',songwriters:['A. Writer','B. Composer']},
  lyrics: [
    { startTimeMs:1000,durationMs:6000,words:'Keep the music moving',agent:'v1',
      translations:{uk:'Нехай музика звучить'},romanization:'Keep the music moving',
      parts:[{startTimeMs:1000,durationMs:700,words:'Keep '},{startTimeMs:1700,durationMs:400,words:'the '},
        {startTimeMs:2100,durationMs:1200,words:'music '},{startTimeMs:3300,durationMs:3700,words:'moving'},
        {startTimeMs:3000,durationMs:3000,words:'(stay with me)',isBackground:true}]},
    {startTimeMs:5000,durationMs:4000,words:'Singing side by side',agent:'v2',parts:[
      {startTimeMs:5000,durationMs:1000,words:'Singing '},{startTimeMs:6000,durationMs:1000,words:'side '},
      {startTimeMs:7000,durationMs:1000,words:'by '},{startTimeMs:8000,durationMs:1000,words:'side'}]},
    {startTimeMs:9000,durationMs:8000,words:'',isInstrumental:true},
    {startTimeMs:17000,durationMs:3000,words:'One more song',parts:null,agent:null,translations:null,romanization:null,timedRomanization:null},
  ],
};
const state = {visible:true,playing:false,position:4.3,rate:1,offset:0,mode:'lyrics',
  timed:true,stage:false,size:42,document:doc,theme:'PlayerVF',css:''};
const update = next => page.evaluate(next => window.PlayerVFBraccato.update(next), next);
const inspect = () => page.evaluate(() => {
  const d=window.PlayerVFBraccato.diagnostics();delete d.html;return d;
});
try {
  await page.goto(process.env.BRACCATO_TEST_URL || 'http://127.0.0.1:4181/braccato/index.html');
  await page.waitForFunction(() => !!window.PlayerVFBraccato);
  await page.evaluate(() => { window.events=[];addEventListener('playervf:braccato-message',e=>window.events.push(e.detail)); });
  await update(state);
  await page.waitForTimeout(150);
  let d=await inspect();
  assert.equal(d.lineCount,4);
  assert.equal(d.status,'rendering');
  assert.equal(d.syncType,'richsync');
  assert.ok(d.letters>10,'original letter wave spans are built');
  assert.ok(d.animations>0,'original Web Animations effects are present');
  assert.equal(await page.locator('.blyrics--translated').textContent(),'Нехай музика звучить');
  assert.equal(await page.locator('.blyrics--romanized').textContent(),'Keep the music moving');
  assert.equal(errors.length,0,errors.join('\n'));
  await mkdir(out,{recursive:true});
  await page.screenshot({path:resolve(out,'30-braccato-original-scroll.png')});
  const initial={...d};

  await update({stage:true,position:5.8});
  await page.waitForTimeout(150);
  d=await inspect();
  assert.equal(d.layout,'stage');
  const agents=await page.locator('[data-stage-visible]').evaluateAll(nodes=>nodes.map(n=>n.dataset.agent));
  assert.ok(agents.includes('v1') && agents.includes('v2'),'overlapping duet remains on stage');
  await page.screenshot({path:resolve(out,'31-braccato-original-stage.png')});

  await update({stage:false,position:11});
  await page.waitForTimeout(100);
  assert.ok(await page.locator('.blyrics--instrumental svg').count()>0,'original instrumental SVG is rendered');
  await page.screenshot({path:resolve(out,'32-braccato-original-instrumental.png')});

  await update({position:4.3,offset:250});
  await page.locator('.blyrics--line').first().click({force:true});
  const seek=await page.evaluate(()=>window.events.find(e=>e.type==='seek'));
  assert.ok(seek && seek.seconds>=0,'line click reaches the Rust seek bridge');
  const before=await inspect();
  await page.evaluate(()=>{for(let n=0;n<200;n++)window.PlayerVFBraccato.update({position:4.3});});
  const after=await inspect();
  assert.equal(after.builds,before.builds,'clock samples do not rebuild lyrics');
  assert.equal(after.themeCompiles,before.themeCompiles,'clock samples do not recompile themes');
  assert.equal(after.ticks,before.ticks,'identical paused samples do not render lyrics again');
  assert.equal(after.tickOptionsUpdates,before.tickOptionsUpdates,'clock samples reuse playback options');
  await update({theme:'Braccato'});
  const standardTheme = await page.locator('.blyrics-container').evaluate(el => {
    const local = getComputedStyle(el), defaults = getComputedStyle(document.documentElement);
    return ['--blyrics-glow-color','--blyrics-lyric-active-color','--blyrics-lyric-inactive-color']
      .every(name => local.getPropertyValue(name).trim() === defaults.getPropertyValue(name).trim());
  });
  assert.ok(standardTheme,'standard Braccato inherits the upstream color and glow defaults');
  const hiddenBefore = (await inspect()).ticks;
  await update({visible:false,playing:true,position:5});
  await page.waitForTimeout(100);
  assert.equal((await inspect()).ticks,hiddenBefore,'hidden lyrics do not tick');
  assert.equal(await page.locator('braccato-lyrics').evaluate(el => el.playing),false,'hidden lyric animations pause');
  await update({visible:true,playing:false,position:4.3});

  const fixedBefore = await inspect();
  for (const theme of ['Власна','Караоке','Контрастна','PlayerVF']) {
    await update({theme,css:'.blyrics-container { --blyrics-glow-color: red; --blyrics-font-size: 8px; }'});
  }
  assert.equal((await inspect()).themeCompiles,fixedBefore.themeCompiles,'legacy themes and custom CSS cannot change or recompile the standard theme');
  assert.equal(await page.locator('.blyrics-container').evaluate(el=>getComputedStyle(el).fontSize),'48px');
  await update({reduced:true});
  const reduced=await page.locator('.blyrics-container').evaluate(el=>getComputedStyle(el).getPropertyValue('--blyrics-animate-word-wobble').trim());
  assert.equal(reduced,'0');

  await update({mode:'editor',editorText:'[00:01.00]<00:01.00>Hello <00:02.00>world'});
  await page.locator('#editor-input').fill('[00:03.00]Edited lyric');
  assert.ok(await page.evaluate(()=>window.events.some(e=>e.type==='edit'&&e.text.includes('Edited lyric'))));
  assert.ok(await page.locator('.bh-edit .bh-line').count()>0,'upstream highlighted editor is attached');
  await page.screenshot({path:resolve(out,'33-braccato-original-editor.png')});
  await update({mode:'lyrics',mobile:true,position:4.3,css:'',reduced:false});
  await page.setViewportSize({width:390,height:844});
  await page.waitForTimeout(100);
  await page.screenshot({path:resolve(out,'34-braccato-original-mobile.png')});

  const eventErrors=await page.evaluate(()=>window.events.filter(e=>e.type==='error'));
  assert.deepEqual(eventErrors,[]);
  assert.deepEqual(errors,[]);
  const remoteRequests=allRequests.filter(url=>!url.startsWith('http://127.0.0.1:4181/')&&!url.startsWith('data:'));
  assert.deepEqual(remoteRequests,[],'the renderer and its font load without a CDN');
  const report={passed:true,initial,final:await inspect(),optimization:{stationarySamples:200,additionalTicks:after.ticks-before.ticks,additionalOptionWrites:after.tickOptionsUpdates-before.tickOptionsUpdates},checks:['standard Braccato colors and glow','upstream packages','letter waves','Web Animations','duet stage','instrumental SVG','seek bridge','no per-tick rebuilds','no duplicate paused ticks','hidden animations pause','custom themes ignored','reduced motion','highlighted editing','mobile resize','no external asset requests']};
  await writeFile(resultPath,JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
} finally { await browser.close(); }
