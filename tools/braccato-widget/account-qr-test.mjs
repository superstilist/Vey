import {chromium} from 'playwright';
import jsQR from 'jsqr';
import assert from 'node:assert/strict';
import {randomBytes,randomUUID} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
const api=process.env.VEY_API_URL||'http://127.0.0.1:4879',live=api.startsWith('https:');
if(live){assert.equal(api,'https://blueberry-melody.superstilist.workers.dev');assert.ok(process.env.VEY_BACKEND_ROOT);}
const accounts=[];
async function register(){const email='vey-qr-'+randomUUID()+'@example.invalid',password=randomBytes(32).toString('hex');const r=await fetch(api+'/vey/auth/register',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password,display_name:'QR fixture'})});assert.ok(r.ok,await r.clone().text());const signed=await r.json();const account={...signed,email,password};accounts.push(account);return account;}
const browser=await chromium.launch({channel:'msedge',headless:true});const errors=[];
try{
 const first=await register(),other=await register();
 const context=await browser.newContext({viewport:{width:390,height:844}});
 await context.addInitScript(api=>{if(location.origin==='http://127.0.0.1:4181')sessionStorage.setItem('vey.account.session',JSON.stringify({api,auto_sync:true}));},api);
 async function app(){const p=await context.newPage();p.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});await p.goto('http://127.0.0.1:4181');await p.locator('iframe').waitFor({state:'attached'});await p.waitForTimeout(700);await p.evaluate(()=>dispatchEvent(new PopStateEvent('popstate',{state:{vey:{view:'settings',section:'account'}}})));await p.waitForTimeout(600);return p;}
 const main=await app(),deviceResponse=main.waitForResponse(r=>r.url().endsWith('/vey/oauth/device_authorization'));
 await main.mouse.click(180,260);const issued=await(await deviceResponse).json();await main.waitForTimeout(700);
 assert.equal(context.pages().length,1,'QR sign-in must not launch a browser tab');
 const screenshot=await main.screenshot({path:'target/account-qr-phone.png'});
 const raster=await main.evaluate(async base64=>{const blob=await(await fetch('data:image/png;base64,'+base64)).blob();const bitmap=await createImageBitmap(blob);const canvas=new OffscreenCanvas(bitmap.width,bitmap.height),ctx=canvas.getContext('2d');ctx.drawImage(bitmap,0,0);return {width:canvas.width,height:canvas.height,rgba:[...ctx.getImageData(0,0,canvas.width,canvas.height).data]};},screenshot.toString('base64'));
 const qr=jsQR(Uint8ClampedArray.from(raster.rgba),raster.width,raster.height);assert.ok(qr,'the actual rendered QR must decode');
 assert.equal(qr.data,issued.verification_uri_complete);assert.equal(new URL(qr.data).pathname,'/connect');assert.ok(!qr.data.includes(issued.device_code));
 const phoneContext=await browser.newContext({viewport:{width:390,height:844},isMobile:true,hasTouch:true}),phone=await phoneContext.newPage();
 await phone.goto(qr.data);await phone.locator('#al-em').fill(first.email);await phone.locator('#al-pw').fill(first.password);await phone.locator('#al-btn').click();await phone.waitForFunction(()=>/^[A-F0-9]{8}$/.test(document.querySelector('#vey-code')?.value||''));
 assert.equal(await phone.locator('#vey-code').inputValue(),issued.user_code);
 await phone.locator('#vey-switch').click();await phone.locator('#al-em').waitFor({state:'visible'});assert.equal(await phone.locator('#al-pw').inputValue(),'');
 await phone.locator('#al-em').fill(other.email);await phone.locator('#al-pw').fill(other.password);await phone.locator('#al-btn').click();await phone.locator('#vey-plan').filter({hasText:other.email}).waitFor();
 assert.equal(await phone.locator('#vey-code').inputValue(),issued.user_code);await phone.screenshot({path:'target/account-qr-consent.png',fullPage:true});await phone.locator('#vey-approve').click();
 await main.waitForFunction(id=>JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').id===id,other.user.id,{timeout:22000});
 // Manual short-code entry uses the same flow without a QR scanner.
 const manual=await app();await manual.waitForTimeout(500);const manualResponse=manual.waitForResponse(r=>r.url().endsWith('/vey/oauth/device_authorization'));await manual.mouse.click(180,260);const second=await(await manualResponse).json();
 await phone.goto(api+'/connect');await phone.locator('#vey-code').waitFor();await phone.locator('#vey-code').fill(second.user_code.slice(0,4).toLowerCase()+'-'+second.user_code.slice(4).toLowerCase());await phone.locator('#vey-approve').click();
 await manual.waitForFunction(id=>JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').id===id,other.user.id,{timeout:22000});
 assert.deepEqual(errors,[]);console.log('PASS '+(live?'LIVE':'FIXTURE')+': decoded actual QR pixels; no secret in QR; independent phone login; switch account; automatic app sign-in; manual grouped/lowercase code.');
}finally{
 await browser.close();
 if(live)for(const account of accounts){assert.match(account.user.id,/^[\w-]{1,80}$/);const root=process.env.VEY_BACKEND_ROOT;const sql="DELETE FROM users WHERE id='"+account.user.id+"' AND email='"+account.email+"'";const r=spawnSync(process.execPath,[path.join(root,'node_modules/wrangler/bin/wrangler.js'),'d1','execute','lyrico-db','--remote','--command',sql,'--json'],{cwd:root,encoding:'utf8'});assert.equal(r.status,0,'Disposable QR account cleanup');}
}
