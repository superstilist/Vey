import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const api='http://127.0.0.1:4879',password='Fixture-only-password-123';
const login=async email=>{const r=await fetch(api+'/vey/auth/login',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({email,password})});assert.ok(r.ok);return r.json();};
const basic=await login('basic@example.test'),admin=await login('admin@example.test');
const grant=await fetch(api+'/vey/admin/entitlements/'+basic.user.id,{method:'PUT',headers:{Authorization:'Bearer '+admin.token,'Content-Type':'application/json'},body:JSON.stringify({premium:true,reason:'tester'})});assert.ok(grant.ok);
const browser=await chromium.launch({channel:'msedge',headless:true});
try{
 const page=await browser.newPage();await page.goto('http://127.0.0.1:4181');
 const result=await page.evaluate(async({api,token})=>{
  const source=await(await fetch('/pkg/playervf.js')).text();const match=source.match(/'(.\/snippets\/[^']+\/web\/account.js)'/);if(!match)throw Error('compiled account import missing');
  const url=new URL(match[1],location.origin+'/pkg/playervf.js');const transfer=await import(url.href),bridge=await import(new URL('bridge.js',url).href);
  const bytes=new Uint8Array(5*1024*1024+2048);bytes.set([73,68,51]);bridge.setCloudFile('browser://fixture.mp3:0',new Blob([bytes]));
  const file=JSON.parse(await transfer.uploadAccountFile(api,token,'browser://fixture.mp3:0'));
  await transfer.downloadAccountFile(api,token,file.id);
  const downloaded=new Uint8Array(await(await fetch(bridge.fileUrl('cloud://'+file.id))).arrayBuffer());
  const deletion=await fetch(api+'/vey/files/'+file.id,{method:'DELETE',headers:{Authorization:'Bearer '+token}});if(!deletion.ok)throw Error('cleanup failed');
  return {size:downloaded.length,head:[...downloaded.slice(0,3)]};
 },{api,token:basic.token});
 assert.equal(result.size,5*1024*1024+2048);assert.deepEqual(result.head,[73,68,51]);console.log('PASS: compiled WASM browser bridge, multipart Blob upload, authenticated download, shared playback URL.');
}finally{await browser.close();}
