import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';

if(!process.argv[2]||!process.argv[3])throw new Error('Usage: npm run test:home -- NATIVE_FIXTURE_DIRECTORY NEW_OUTPUT_DIRECTORY');
const fixture=resolve(process.argv[2]),out=resolve(process.argv[3]);await mkdir(out);
const seed=JSON.parse(await readFile(join(fixture,'store.json'),'utf8'));
const trackA={path:'yt://HomeSong001',videoId:'HomeSong001',title:'Collection song one',artist:'Fixture artist',durationSecs:30};
const trackB={...trackA,path:'yt://HomeSong002',videoId:'HomeSong002',title:'Collection song two'};
const wav=Buffer.alloc(44+32000*30);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(16000,24);wav.writeUInt32LE(32000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
const browser=await chromium.launch({channel:'msedge',headless:true});let page;
const errors=[],requests={home:0,collections:[],search:[],video:[]};let offline=false;
async function open(width,height){
  const context=await browser.newContext({viewport:{width,height},isMobile:width<800||height<500,hasTouch:width<800||height<500});
  const page=await context.newPage();page.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
  await page.addInitScript(seed=>{
    seed.settings={...seed.settings,language:'en',home_layout:0,home_rotate:false,lyrics_reduced_motion:true};
    seed.home_catalog.updatedAt=Date.now();seed.recommendations.updatedAt=Date.now();
    localStorage.setItem('playervf.rust.v1',JSON.stringify(seed));window.audioSources=[];
    const NativeAudio=window.Audio;window.Audio=class extends NativeAudio{constructor(...a){super(...a);window.homeAudio=this;}set src(value){window.audioSources.push(value);super.src=value;}get src(){return super.src;}};
  },structuredClone(seed));
  await page.route('https://fixtures.vey.test/**',async r=>{const name=new URL(r.request().url()).pathname.split('/').at(-1);await r.fulfill({contentType:'image/png',body:await readFile(join(fixture,name))});});
  await page.route('**/api/home*',r=>{requests.home++;return offline?r.fulfill({status:503,body:'offline fixture'}):r.fulfill({contentType:'application/json',body:JSON.stringify({...seed.home_catalog,updatedAt:Date.now()})});});
  await page.route('**/api/browse*',r=>{const id=new URL(r.request().url()).searchParams.get('id');if(id)requests.collections.push(id);return r.fulfill({contentType:'application/json',body:JSON.stringify(id?.startsWith('VL')?[trackA,trackA,trackB]:[trackA,trackB])});});
  await page.route('**/api/search*',r=>{requests.search.push(new URL(r.request().url()).searchParams.get('q'));return r.fulfill({contentType:'application/json',body:JSON.stringify([trackA,trackB])});});
  await page.route('**/api/audio/*',r=>r.fulfill({contentType:'audio/wav',body:wav}));
  await page.route('**/api/lyrics*',r=>r.fulfill({contentType:'application/json',body:'[]'}));
  await page.route('**/api/video',r=>{requests.video.push(JSON.parse(r.request().postData()));return r.fulfill({contentType:'application/json',body:'null'});});
  await page.goto('http://127.0.0.1:4181/');await page.locator('iframe').waitFor({state:'attached'});await page.waitForTimeout(1300);return {context,page};
}
const nav=async(view)=>{await page.waitForFunction(view=>history.state?.vey?.view===view,view,{timeout:7000});await page.waitForTimeout(350);};
const shot=name=>page.screenshot({path:join(out,`${name}.png`)});
try{
  const desktop=await open(1440,900);page=desktop.page;await shot('desktop-home');
  const initialRequests=requests.home;assert.equal(initialRequests,0,'fresh mixed-media cache opens without a catalog request');
  await page.mouse.click(680,112);await page.waitForTimeout(250);await shot('desktop-playlists');
  const playlist=page.waitForResponse(r=>r.url().includes('/api/browse?id=VLfixture0'),{timeout:7000});await page.mouse.click(400,330);await playlist;await nav('explore');await shot('public-playlist');
  await page.mouse.click(99,855);await page.waitForFunction(()=>window.homeAudio.src.endsWith('/HomeSong001'));
  const before=await page.evaluate(()=>window.audioSources.length);await page.mouse.click(150,855);
  await page.waitForFunction(count=>window.audioSources.length>count,before);assert.ok((await page.evaluate(()=>window.audioSources.at(-1))).endsWith('/HomeSong001'),'intentional playlist repeat is retained');
  await page.mouse.click(150,855);await page.waitForFunction(()=>window.homeAudio.src.endsWith('/HomeSong002'));
  await page.goBack();await nav('home');
  await page.mouse.click(520,112);await page.waitForTimeout(200);await shot('desktop-albums');
  const album=page.waitForResponse(r=>r.url().includes('/api/browse?id=MPREfixture0'),{timeout:7000});await page.mouse.click(400,330);await album;await nav('explore');await shot('album-tracks');
  await page.goBack();await nav('home');
  await page.mouse.click(600,112);await page.waitForTimeout(200);await shot('desktop-artists');
  const artist=page.waitForResponse(r=>r.url().includes('/api/search?'),{timeout:7000});await page.mouse.click(400,330);await artist;await nav('search');assert.equal(requests.search.at(-1),'Artist 1');
  await page.goBack();await nav('home');
  await page.mouse.click(442,112);await page.waitForTimeout(200);await shot('desktop-videos');
  const first=await page.screenshot({clip:{x:325,y:245,width:950,height:240}});await page.mouse.click(1334,208);await page.waitForTimeout(250);
  assert.ok(!first.equals(await page.screenshot({clip:{x:325,y:245,width:950,height:240}})),'shelf arrow changes the visible video cards');
  await page.mouse.click(1290,208);await page.waitForTimeout(200);await page.mouse.click(400,330);await nav('lyrics');
  await page.waitForFunction(()=>window.homeAudio.src.includes('/fixture0000'));assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).settings.art_mode),'clip','a video duplicated on Listen again still opens from its own shelf in clip mode');
  await page.goBack();await nav('home');
  await page.mouse.click(1340,112);await page.waitForTimeout(900);assert.equal(requests.home,initialRequests+1,'refresh explicitly requests fresh Home data');
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).home_catalog);assert.ok(saved.sections.some(s=>s.kind==='video'));
  offline=true;await page.mouse.click(1340,112);await page.waitForTimeout(700);assert.deepEqual(await page.evaluate(()=>JSON.parse(localStorage.getItem('playervf.rust.v1')).home_catalog),saved,'failed refresh preserves the mixed-media cache');
  await shot('cached-offline');await desktop.context.close();offline=false;
  for(const [width,height] of [[390,844],[320,640],[844,390]]){
    const mobile=await open(width,height);page=mobile.page;await shot(`home-${width}x${height}`);
    await page.mouse.click(144,height<500?90:102);await page.waitForTimeout(250);await shot(`videos-${width}x${height}`);
    if(width===390){await page.mouse.click(130,300);await nav('lyrics');await page.waitForFunction(()=>window.homeAudio.src.includes('/fixture0000'));}
    await mobile.context.close();
  }
  assert.deepEqual(errors,[]);const report={passed:true,requests,checks:['cached startup','square album/playlist cards','circular artist cards','wide video shelf','shelf paging','public playlist repeats','album navigation','artist search','video identity and clip-mode handoff','explicit refresh','offline cache','390/320px and landscape layout','no browser errors']};
  await writeFile(join(out,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
}catch(error){if(page&&!page.isClosed())await shot('failure');throw error;}finally{await browser.close();}
