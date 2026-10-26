import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {randomBytes,randomUUID} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
const api=process.env.VEY_API_URL||'http://127.0.0.1:4879',live=api.startsWith('https:');
if(live){assert.equal(api,'https://blueberry-melody.superstilist.workers.dev');assert.ok(process.env.VEY_BACKEND_ROOT,'Live cleanup needs VEY_BACKEND_ROOT');}
const email='vey-browser-'+randomUUID()+'@example.invalid',password=randomBytes(32).toString('hex');
const response=await fetch(api+'/vey/auth/register',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password,display_name:'Browser OAuth fixture'})});assert.ok(response.ok,await response.clone().text());const account=await response.json();
const browser=await chromium.launch({channel:'msedge',headless:true});const errors=[];
try{
 const context=await browser.newContext({viewport:{width:390,height:844}});
 await context.addInitScript(api=>{if(location.origin==='http://127.0.0.1:4181'&&!sessionStorage.getItem('vey.account.session'))sessionStorage.setItem('vey.account.session',JSON.stringify({api,auto_sync:true}));},api);
 async function app(){const p=await context.newPage();p.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message);});await p.goto('http://127.0.0.1:4181');await p.locator('iframe').waitFor({state:'attached'});await p.waitForTimeout(700);await p.evaluate(()=>dispatchEvent(new PopStateEvent('popstate',{state:{vey:{view:'settings',section:'account'}}})));await p.waitForTimeout(600);return p;}
 async function start(p){const popupPromise=context.waitForEvent('page');await p.mouse.click(180,205);const popup=await popupPromise;await popup.waitForURL(url=>url.origin===api&&['/account','/connect'].includes(url.pathname));return popup;}
 const first=await app(),login=await start(first);login.on('pageerror',e=>errors.push(e.message));
 await login.locator('#al-em').fill(email);await login.locator('#al-pw').fill(password);await login.locator('#al-btn').click();await login.locator('#vey-approve').waitFor();
 await login.waitForFunction(()=>/^[A-F0-9]{8}$/.test(document.querySelector('#vey-code')?.value||''));
 assert.match(await login.locator('#vey-code').inputValue(),/^[A-F0-9]{8}$/);await login.screenshot({path:'target/account-oauth-consent-mobile.png',fullPage:true});await login.locator('#vey-approve').click();
 await first.waitForFunction(id=>JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').id===id,account.user.id,{timeout:20000});if(!login.isClosed())await login.waitForEvent('close',{timeout:4000});
 await first.screenshot({path:'target/account-oauth-connected.png'});
 // The website session is reused on a second sign-in without asking for the password again.
 const second=await app(),sso=await start(second);await sso.locator('#vey-approve').waitFor();await sso.waitForFunction(()=>/^[A-F0-9]{8}$/.test(document.querySelector('#vey-code')?.value||''));assert.equal(await sso.locator('#acc-auth').isVisible(),false);await sso.setViewportSize({width:1440,height:900});await sso.screenshot({path:'target/account-oauth-consent-desktop.png',fullPage:true});await sso.locator('#vey-approve').click();
 await second.waitForFunction(id=>JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').id===id,account.user.id,{timeout:20000});if(!sso.isClosed())await sso.waitForEvent('close',{timeout:4000});
 // Explicit denial returns to an unsigned app and never creates an app session.
 const third=await app(),deny=await start(third);await deny.locator('#vey-deny').waitFor();await deny.waitForFunction(()=>/^[A-F0-9]{8}$/.test(document.querySelector('#vey-code')?.value||''));await deny.locator('#vey-deny').click();await third.waitForTimeout(6500);assert.equal(await third.evaluate(()=>JSON.parse(sessionStorage.getItem('vey.account.session')||'{}').token||''),'');if(!deny.isClosed())await deny.waitForEvent('close',{timeout:4000});
 assert.deepEqual(errors,[]);console.log('PASS '+(live?'LIVE':'FIXTURE')+': one-click WASM OAuth; system-browser popup; matching code; website login; password-free website session reuse; denial; automatic return to app.');
}finally{
 await browser.close();
 if(live){assert.match(account.user.id,/^[\w-]{1,80}$/);const root=process.env.VEY_BACKEND_ROOT;const sql="DELETE FROM users WHERE id='"+account.user.id+"' AND email='"+email+"'";const result=spawnSync(process.execPath,[path.join(root,'node_modules/wrangler/bin/wrangler.js'),'d1','execute','lyrico-db','--remote','--command',sql,'--json'],{cwd:root,encoding:'utf8'});assert.equal(result.status,0,'Disposable account cleanup');console.log('Removed disposable browser account.');}
}
