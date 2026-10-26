import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const api='http://127.0.0.1:4879',password='Fixture-only-password-123',email='wasm-'+Date.now()+'@example.test';
const r=await fetch(api+'/vey/auth/register',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password,display_name:'Wasm fixture'})});assert.ok(r.ok);const account=await r.json();
const b=await chromium.launch({channel:'msedge',headless:true}),p=await b.newPage({viewport:{width:390,height:844}});const errors=[];
p.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});
const track={path:'yt://abcdefghijk',videoId:'abcdefghijk',title:'Wasm saved fixture',artist:'Vey',durationSecs:30};
await p.addInitScript(track=>localStorage.setItem('playervf.rust.v1',JSON.stringify({version:1,settings:{language:'uk',volume:0.4},tracks:[track],likes:[track.path],history:[{track,playedAt:1,playCount:3}]})),track);
try{
 await p.goto('http://127.0.0.1:4181');await p.locator('iframe').waitFor({state:'attached'});await p.waitForTimeout(800);
 await p.evaluate(()=>dispatchEvent(new PopStateEvent('popstate',{state:{vey:{view:'settings',section:'account'}}})));await p.waitForTimeout(700);
 const fill=async(y,value)=>{await p.mouse.click(150,y);await p.keyboard.press('Control+a');await p.keyboard.type(value);};
 await p.mouse.click(180,314);await p.waitForTimeout(200);await fill(396,api);await fill(503,email);await fill(557,password);await p.mouse.click(105,611);
 await p.waitForFunction(()=>{const s=JSON.parse(sessionStorage.getItem('vey.account.session')||'{}');return !!s.token;});
 await p.waitForTimeout(1800);await p.mouse.click(180,305);
 await p.waitForFunction(()=>JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').revision===1);
 const headers={Authorization:'Bearer '+account.token,'Content-Type':'application/json'};
 const get=async()=>{const r=await fetch(api+'/vey/library',{headers});assert.ok(r.ok);return r.json();};
 const put=async(snapshot)=>{const r=await fetch(api+'/vey/library',{method:'PUT',headers,body:JSON.stringify(snapshot)});assert.ok(r.ok,await r.clone().text());};
 const saved=await get();assert.equal(saved.library.history[0].playCount,3);assert.ok(Math.abs(saved.library.settings.volume-0.4)<0.001);assert.deepEqual(saved.library.likes,[track.path]);
 saved.library.settings.volume=0.77;saved.library.likes=[];await put(saved);
 await p.mouse.click(180,358);await p.waitForFunction(()=>Math.abs(JSON.parse(localStorage.getItem('playervf.rust.v1')).settings.volume-0.77)<0.001);
 const remote=await get();remote.library.settings.volume=0.33;await put(remote);
 await p.waitForFunction(()=>Math.abs(JSON.parse(localStorage.getItem('playervf.rust.v1')).settings.volume-0.33)<0.001,undefined,{timeout:22000});
 await p.screenshot({path:'target/account-wasm-basic.png'});
 const conflict=await get();conflict.library.settings.volume=0.55;await put(conflict);
 await p.mouse.click(180,305);await p.waitForFunction(()=>JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').auto_sync===false);
 assert.equal((await get()).library.settings.volume,0.55,'stale local push cannot overwrite remote edits');
 // Logout remains available after a conflict, clears the local token, and revokes it.
 await p.mouse.move(250,630);await p.mouse.wheel(0,600);await p.waitForTimeout(250);await p.screenshot({path:'target/account-wasm-conflict.png'});

 const uiToken=await p.evaluate(()=>JSON.parse(sessionStorage.getItem('vey.account.session')).token);
 await p.mouse.click(180,616);await p.waitForFunction(()=>!JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').token);await p.waitForTimeout(300);assert.equal((await fetch(api+'/vey/account',{headers:{Authorization:'Bearer '+uiToken}})).status,401);
 assert.deepEqual(errors,[]);console.log('PASS: actual WASM login; full metadata push; restore; automatic remote pull; revision conflict pauses sync; logout clears and revokes the UI session.');
}finally{await b.close();}
