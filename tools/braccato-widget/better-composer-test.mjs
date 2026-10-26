import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {randomUUID,randomBytes} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
const api=process.env.VEY_API_URL||'http://127.0.0.1:4892',live=api.startsWith('https:');
if(live){assert.equal(api,'https://blueberry-melody.superstilist.workers.dev');assert.ok(process.env.VEY_BACKEND_ROOT);}
const id=live?'composer-'+randomUUID():'composer-fixture',fixturePath=api+'/lyrics/'+id,email='composer-'+randomUUID()+'@example.invalid';let owner;
function sql(command){const root=process.env.VEY_BACKEND_ROOT;const result=spawnSync(process.execPath,[path.join(root,'node_modules/wrangler/bin/wrangler.js'),'d1','execute','lyrico-db','--remote','--command',command,'--json'],{cwd:root,encoding:'utf8'});assert.equal(result.status,0,'Disposable Composer fixture database operation');}
const child='https://blueberry-composer.superstilist.workers.dev';
const browser=await chromium.launch({channel:'msedge',headless:true});
try{
 const response=await fetch(api+'/auth/register',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password:randomBytes(32).toString('hex'),display_name:'Composer fixture'})});assert.ok(response.ok,await response.clone().text());const signed=await response.json();owner=signed.user.id;assert.match(owner,/^[\w-]{1,80}$/);if(live)sql("INSERT INTO songs(id,artist,title,album) VALUES('"+id+"','Fixture Artist','Composer Roundtrip','Fixture Album');INSERT INTO lyrics(id,song_id,language,plain_text) VALUES('"+id+"-lyrics','"+id+"','en','Hello world');INSERT INTO line_sync(id,lyrics_id,line_index,start_time,end_time,text) VALUES('"+id+"-line','"+id+"-lyrics',0,1,4,'Hello world')");
 const context=await browser.newContext({viewport:{width:1440,height:1000}});
 await context.addInitScript(({api,token})=>{if(location.origin===api)localStorage.setItem('blueberry_melody_tk',token);},{api,token:signed.token});
 if(!live)await context.route(child+'/**',async route=>{const u=new URL(route.request().url());const result=await route.fetch({url:'http://127.0.0.1:4891'+u.pathname+u.search});await route.fulfill({response:result});});
 await context.addInitScript(child=>{if(location.origin===child){const play=HTMLMediaElement.prototype.play;HTMLMediaElement.prototype.play=function(...args){window.fixtureMedia=this;return play.apply(this,args);};}},child);
 const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto(api+'/lyrics/create?song='+id);await page.locator('#bc-status').filter({hasText:'Editing Fixture Artist'}).waitFor({timeout:45000});
 assert.equal(await page.locator('#st-create').isVisible(),false);assert.equal(await page.locator('#bc-panel').isVisible(),true);
 const frame=page.frameLocator('#bc-frame');await frame.getByRole('button',{name:'Close',exact:true}).first().click({timeout:3000}).catch(()=>{});
 assert.ok(await frame.locator('body').innerText().then(t=>t.includes('Powered by Better Lyrics Composer')));
 // A forged response from the host window cannot impersonate the editor iframe.
 await page.evaluate(()=>dispatchEvent(new MessageEvent('message',{origin:'https://evil.example',source:window,data:{channel:'blueberry-composer',type:'exported',id:'forged',value:{plain:'corrupted'}}})));
 const audio=Buffer.alloc(44+8000*2*6);audio.write('RIFF');audio.writeUInt32LE(audio.length-8,4);audio.write('WAVEfmt ',8);audio.writeUInt32LE(16,16);audio.writeUInt16LE(1,20);audio.writeUInt16LE(1,22);audio.writeUInt32LE(8000,24);audio.writeUInt32LE(16000,28);audio.writeUInt16LE(2,32);audio.writeUInt16LE(16,34);audio.write('data',36);audio.writeUInt32LE(audio.length-44,40);
 await page.locator('#bc-audio').setInputFiles({name:'fixture.wav',mimeType:'audio/wav',buffer:audio});await page.locator('#bc-status').filter({hasText:'Audio linked locally'}).waitFor({timeout:30000});
 await page.locator('#bc-preview').click();await frame.locator('[data-tour=preview-panel]').waitFor({state:'visible'});await frame.locator('[data-tour=preview-panel] braccato-lyrics').waitFor({state:'attached'});
 await page.locator('#bc-save').click();await page.locator('#bc-status').filter({hasText:'Saved to blueberry_melody'}).waitFor({timeout:30000});
 const saved=await(await fetch(fixturePath)).json();assert.match(saved.lyrics.plain,/Hello world/);assert.equal(saved.lyrics.line_sync[0].start,1);assert.equal(saved.lyrics.line_sync[0].end,4);
 const raw='<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata"><body><div><p begin="1s" end="4s" ttm:agent="v2"><span begin="1s" end="2s">Hello</span> <span begin="2s" end="4s">world</span><span ttm:role="x-bg"><span begin="1.5s" end="2.5s">oh</span></span></p></div></body></tt>';
 const written=await fetch(fixturePath,{method:'PUT',headers:{'Content-Type':'application/json',Authorization:'Bearer '+signed.token},body:JSON.stringify({lyrics_text:raw,lyrics_format:'ttml'})});assert.ok(written.ok);const background=await(await fetch(fixturePath)).json();assert.ok(background.lyrics.word_sync.some(word=>word.is_bg===1));assert.equal(background.lyrics.line_sync[0].agent,'v2');
 await page.screenshot({path:'target/better-composer-desktop.png',fullPage:true});
 await page.setViewportSize({width:390,height:844});assert.equal(await page.frames().find(f=>f.url().startsWith(child)).evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.screenshot({path:'target/better-composer-mobile.png',fullPage:true});
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 await frame.locator('[data-tour=preview-panel]').getByRole('button',{name:'Play',exact:true}).click();const mediaFrame=page.frames().find(f=>f.url().startsWith(child));await mediaFrame.waitForFunction(()=>window.fixtureMedia&&!window.fixtureMedia.paused);
 await page.evaluate(()=>BM.studio.open('add'));await mediaFrame.waitForFunction(()=>window.fixtureMedia.paused);assert.equal(await page.locator('#bc-panel').isVisible(),false);
 await page.evaluate(()=>BM.studio.open('create'));assert.equal(await page.locator('#bc-panel').isVisible(),true);
 await page.reload();await page.locator('#bc-status').filter({hasText:'Editing Fixture Artist'}).waitFor({timeout:30000});
 assert.ok(await page.frameLocator('#bc-frame').locator('body').innerText().then(text=>text.includes('Composer Roundtrip')));
 const childFrame=page.frames().find(f=>f.url().startsWith(child));assert.equal(await childFrame.evaluate(()=>Object.keys(localStorage).filter(k=>k.startsWith('blueberry.song.')).length),1,'Reload must resume the existing local project');
 assert.deepEqual(errors,[]);console.log('PASS '+(live?'LIVE':'FIXTURE')+': separate Composer page and attribution, old editor hidden, catalog import, original live preview, API publish roundtrip, forged message rejection, mobile layout, Add compatibility, project persistence.');
}finally{await browser.close();if(live&&owner){sql("DELETE FROM songs WHERE id='"+id+"' AND title='Composer Roundtrip';DELETE FROM users WHERE id='"+owner+"' AND email='"+email+"'");console.log('Removed disposable Composer song and account.');}}
