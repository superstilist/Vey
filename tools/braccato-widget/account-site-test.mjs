import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const api='http://127.0.0.1:4879',password='Fixture-only-password-123';
async function request(route,body,token){const r=await fetch(api+'/vey/'+route,{method:body?'POST':'GET',headers:{'Content-Type':'application/json',...(token?{Authorization:'Bearer '+token}:{})},...(body?{body:JSON.stringify(body)}:{})});assert.ok(r.ok,await r.clone().text());return r.json();}
const basic=await request('auth/register',{email:'site-'+Date.now()+'@example.test',password,display_name:'Site fixture'}),admin=await request('auth/login',{email:'admin@example.test',password});
const device=await request('auth/device',{});
const browser=await chromium.launch({channel:'msedge',headless:true});
try{
 const page=await browser.newPage({viewport:{width:390,height:844}}),errors=[];
 page.on('pageerror',e=>errors.push(e.message));
 await page.goto(api+'/account?vey_code='+device.user_code);
 await page.locator('#al-em').fill(basic.user.email);await page.locator('#al-pw').fill(password);await page.locator('#al-btn').click();
 await page.locator('#vey-panel').waitFor();await page.locator('#vey-plan').filter({hasText:'Basic'}).waitFor();
 assert.equal(await page.locator('#al-pw').inputValue(),'');
 assert.equal(await page.locator('#vey-upload').isVisible(),false);
 assert.equal(await page.locator('#acc-profile > :first-child').getAttribute('id'),'vey-panel');
 await page.locator('#vey-approve').click();await page.locator('#vey-status').filter({hasText:'Device connected'}).waitFor();
 assert.equal((await request('auth/device/poll',{device_code:device.device_code})).user.id,basic.user.id);
 const grant=await fetch(api+'/vey/admin/entitlements/'+basic.user.id,{method:'PUT',headers:{Authorization:'Bearer '+admin.token,'Content-Type':'application/json'},body:JSON.stringify({premium:true,reason:'tester'})});assert.ok(grant.ok);
 await page.locator('#vey-refresh').click();await page.locator('#vey-upload').waitFor({state:'visible'});
 const audio=Buffer.alloc(8192);audio.write('ID3');await page.locator('#vey-files').setInputFiles({name:'browser-fixture.mp3',mimeType:'audio/mpeg',buffer:audio});await page.locator('#vey-send').click();
 await page.locator('#vey-status').filter({hasText:'Files saved'}).waitFor();await page.locator('#vey-list').filter({hasText:'browser-fixture.mp3'}).waitFor();
 const downloaded=page.waitForEvent('download');await page.locator('#vey-list button').filter({hasText:'Download'}).click();assert.equal((await downloaded).suggestedFilename(),'browser-fixture.mp3');
 await page.screenshot({path:'target/account-site-mobile.png',fullPage:true});
 page.on('dialog',d=>d.accept());await page.locator('#vey-list button').filter({hasText:'Delete'}).click();await page.waitForFunction(()=>!document.querySelector('#vey-list').textContent.includes('browser-fixture.mp3'));
 await page.locator('#vey-auto').check();await page.waitForTimeout(300);await page.locator('#vey-files').setInputFiles({name:'automatic.mp3',mimeType:'audio/mpeg',buffer:audio});await page.locator('#vey-status').filter({hasText:'Files saved'}).waitFor();
 assert.equal((await request('library',undefined,basic.token)).library.autoUpload,true);
 await page.setViewportSize({width:1440,height:900});await page.screenshot({path:'target/account-site-desktop.png',fullPage:true});
 assert.deepEqual(errors,[]);console.log('PASS: mobile/desktop account panel; password cleared; Basic gate; device approval; Premium upload/download/delete; automatic selected-file upload.');
}finally{await browser.close();}
