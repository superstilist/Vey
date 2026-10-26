import {chromium} from 'playwright';
import jsQR from 'jsqr';
import assert from 'node:assert/strict';
import {randomBytes,randomUUID} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
const api=process.env.VEY_API_URL||'http://127.0.0.1:4879',live=api.startsWith('https:');
if(live){assert.equal(api,'https://blueberry-melody.superstilist.workers.dev');assert.ok(process.env.VEY_BACKEND_ROOT);}
const email='vey-webqr-'+randomUUID()+'@example.invalid',password=randomBytes(32).toString('hex');let account;
const browser=await chromium.launch({channel:'msedge',headless:true});const errors=[];
try{
 const registered=await fetch(api+'/vey/auth/register',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password,display_name:'Web QR fixture'})});assert.ok(registered.ok);account=await registered.json();
 const desktop=await browser.newContext({viewport:{width:1280,height:900}}),pc=await desktop.newPage();
 const mobile=await browser.newContext({viewport:{width:390,height:844},isMobile:true,hasTouch:true}),phone=await mobile.newPage();
 for(const page of [pc,phone])page.on('pageerror',e=>errors.push(e.message));
 await pc.goto(api+'/account');await pc.locator('#vq-start').click();await pc.locator('#vq-image').waitFor({state:'visible'});
 await pc.waitForFunction(()=>document.querySelector('#vq-image').naturalWidth>0);
 const raster=await pc.locator('#vq-image').evaluate(image=>{const canvas=document.createElement('canvas');canvas.width=464;canvas.height=464;const ctx=canvas.getContext('2d');ctx.drawImage(image,0,0,464,464);return [...ctx.getImageData(0,0,464,464).data];});
 const decoded=jsQR(Uint8ClampedArray.from(raster),464,464);assert.ok(decoded);const url=new URL(decoded.data);assert.equal(url.origin,api);assert.equal(url.pathname,'/connect');assert.equal(url.searchParams.size,2);
 const code=url.searchParams.get('vey_code');assert.equal((await pc.locator('#vq-code').innerText()).replace(/\s/g,''),code);
 await pc.screenshot({path:'target/account-web-qr-desktop.png',fullPage:true});
 // A signed-in phone opens its scanner, rather than navigating the QR URL.
 await phone.goto(api+'/connect');await phone.locator('#al-em').fill(email);await phone.locator('#al-pw').fill(password);await phone.locator('#al-btn').click();await phone.locator('#vq-scan').waitFor();
 // Feed an actual QR image through a real video/canvas stream. No hardware claim.
 await phone.evaluate(async code=>{
   const image=new Image();image.src='/vey/oauth/qr?user_code='+code;await image.decode();
   const canvas=document.createElement('canvas');canvas.width=720;canvas.height=720;const ctx=canvas.getContext('2d');ctx.fillStyle='white';ctx.fillRect(0,0,720,720);ctx.drawImage(image,80,80,560,560);
   window.fixtureCanvas=canvas;window.fixtureStream=canvas.captureStream(5);
   Object.defineProperty(navigator.mediaDevices,'getUserMedia',{configurable:true,value:async()=>window.fixtureStream});
 },code);
 await phone.locator('#vq-scan').click();await phone.waitForFunction(code=>document.querySelector('#vey-code').value===code,code);
 assert.equal(await phone.evaluate(()=>window.fixtureStream.getTracks().every(track=>track.readyState==='ended')),true);
 assert.equal(await pc.evaluate(()=>BM.state.U),null,'Scanning alone must not grant access');
 await phone.screenshot({path:'target/account-web-qr-phone.png',fullPage:true});
 await phone.locator('#vey-approve').click();await pc.waitForFunction(id=>BM.state.U?.id===id,account.user.id,{timeout:22000});
 assert.ok(await pc.evaluate(()=>localStorage.getItem('blueberry_melody_tk')));
 assert.equal(await pc.evaluate(()=>JSON.parse(atob(BM.state.TK.split('.')[1])).exp),undefined,'New website sessions have no timed expiry');
 const me=await pc.evaluate(async()=>{const r=await fetch('/auth/me',{headers:{Authorization:'Bearer '+BM.state.TK}});return r.json();});assert.equal(me.id,account.user.id);
 await pc.reload();await pc.waitForFunction(id=>BM.state.U?.id===id,account.user.id);
 // Image fallback on a second PC session, including rejection of unrelated QR links.
 const other=await browser.newPage({viewport:{width:1280,height:900}});await other.goto(api+'/account');await other.locator('#vq-start').click();await other.waitForFunction(()=>document.querySelector('#vq-image').naturalWidth>0&&!document.querySelector('#vq-image').hidden);
 const image=await other.locator('#vq-image').screenshot();const second=(await other.locator('#vq-code').innerText()).replace(/\s/g,'');
 await phone.goto(api+'/connect');await phone.locator('#vq-file').setInputFiles({name:'pc-qr.png',mimeType:'image/png',buffer:image});await phone.waitForFunction(code=>document.querySelector('#vey-code').value===code,second);
 await phone.locator('#vey-deny').click();await other.locator('#vq-login-status').filter({hasText:'cancelled'}).waitFor({timeout:22000});
 assert.equal(await other.evaluate(()=>BM.state.U),null);assert.equal(await other.locator('#vq-image').isVisible(),false);
 await other.locator('#vq-start').click();await other.locator('#vq-cancel').click();assert.equal(await other.locator('#vq-login').isVisible(),false);
 assert.deepEqual(errors,[]);console.log('PASS '+(live?'LIVE':'FIXTURE')+': website QR pixels decode; separate phone camera stream; camera stops; explicit consent; automatic PC website login; authenticated API and reload; image fallback; denial; cancellation.');
}finally{
 await browser.close();
 if(live&&account){assert.match(account.user.id,/^[\w-]{1,80}$/);const root=process.env.VEY_BACKEND_ROOT;const sql="DELETE FROM users WHERE id='"+account.user.id+"' AND email='"+email+"'";const r=spawnSync(process.execPath,[path.join(root,'node_modules/wrangler/bin/wrangler.js'),'d1','execute','lyrico-db','--remote','--command',sql,'--json'],{cwd:root,encoding:'utf8'});assert.equal(r.status,0,'Disposable website QR account cleanup');}
}
