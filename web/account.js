import {fileUrl, setCloudFile} from './bridge.js';
let uploadController, loginWindow;
export function prepareAccountLogin(){
  loginWindow=window.open('about:blank','vey-account-login');
  if(loginWindow){loginWindow.opener=null;try{loginWindow.document.title='Vey sign-in';}catch{}}return !!loginWindow;
}
export function openAccountLogin(url){
  if(url===''){prepareAccountLogin();return;}
  if(url==='#close'){closeAccountLogin();return;}
  if(loginWindow&&!loginWindow.closed)loginWindow.location.replace(url);
  else {loginWindow=window.open(url,'vey-account-login');if(loginWindow)loginWindow.opener=null;}
}
export function closeAccountLogin(){try{loginWindow?.close();}catch{}loginWindow=undefined;return true;}

async function json(url, method, token, body, signal) {
  const headers={Authorization:'Bearer '+token};if(body)headers['Content-Type']='application/json';
  const response=await fetch(url,{method,headers,body:body||undefined,signal:signal||AbortSignal.timeout(45000),redirect:'error',credentials:'omit',referrerPolicy:'no-referrer',cache:'no-store'});
  const reader=response.body.getReader(),chunks=[];let size=0;
  while(true){const p=await reader.read();if(p.done)break;size+=p.value.byteLength;if(size>2*1024*1024){await reader.cancel();throw Error('API response is too large');}chunks.push(p.value);}
  const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}
  const result=JSON.parse(new TextDecoder().decode(bytes));
  if(response.status===401&&token)throw Error('Сесія завершилася. Увійдіть знову.');
  if(!response.ok){if(response.status===400&&new URL(url).pathname.endsWith('/vey/oauth/token')&&['authorization_pending','slow_down'].includes(result.error))return result;throw Error(result.error_description||result.error?.message||'Account request failed');}return result;
}
export async function accountRequest(url,method,token,body){try{return JSON.stringify(await json(url,method,token,body));}catch(e){throw String(e.message||e);}}
export function pickAccountAvatar(){
  return new Promise((resolve,reject)=>{
    const input=document.createElement('input');input.type='file';input.accept='image/jpeg,image/png,image/webp';
    input.onchange=async()=>{try{const file=input.files?.[0];if(!file){resolve(null);return;}if(file.size>2*1024*1024)throw Error('Avatar must be 2 MiB or smaller');resolve(new Uint8Array(await file.arrayBuffer()));}catch(e){reject(String(e.message||e));}};
    input.click();
  });
}
export async function accountBinaryRequest(url,method,token,contentType,bytes){
  const response=await fetch(url,{method,headers:{Authorization:'Bearer '+token,'Content-Type':contentType},body:bytes,signal:AbortSignal.timeout(45000),redirect:'error',credentials:'omit',referrerPolicy:'no-referrer',cache:'no-store'});
  const text=await response.text();let result;try{result=JSON.parse(text);}catch{result={};}
  if(response.status===401&&token)throw Error('Сесія завершилася. Увійдіть знову.');
  if(!response.ok)throw Error(result.error_description||result.error?.message||'Avatar upload failed');
  return JSON.stringify(result);
}
export async function accountAvatarRequest(url,token){
  const response=await fetch(url,{headers:{Authorization:'Bearer '+token},signal:AbortSignal.timeout(45000),redirect:'error',credentials:'omit',referrerPolicy:'no-referrer',cache:'no-store'});
  if(response.status===404)return new Uint8Array();
  if(response.status===401)throw Error('Сесія завершилася. Увійдіть знову.');
  if(!response.ok)throw Error('Avatar download failed');
  const declared=Number(response.headers.get('Content-Length'));if(Number.isFinite(declared)&&declared>2*1024*1024)throw Error('Avatar response is too large');
  const reader=response.body?.getReader();if(!reader)return new Uint8Array();const chunks=[];let size=0;
  try{while(true){const part=await reader.read();if(part.done)break;size+=part.value.byteLength;if(size>2*1024*1024){await reader.cancel().catch(()=>{});throw Error('Avatar response is too large');}chunks.push(part.value);}}
  finally{reader.releaseLock();}
  const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.length;}return bytes;
}
export function cancelAccountUpload(){uploadController?.abort();}
export async function uploadAccountFile(api,token,path){
  let pending,complete=false;uploadController=new AbortController();
  try{
    if(!path.startsWith('browser://'))throw Error('Import the local audio file again before uploading');
    const source=fileUrl(path);if(!source)throw Error('Choose the local file again after reloading the app');
    const blob=await(await fetch(source,{signal:uploadController.signal})).blob();
    const name=decodeURIComponent(path.slice('browser://'.length).split(':')[0]);
    pending=await json(api+'/vey/uploads','POST',token,JSON.stringify({name,size:blob.size}),uploadController.signal);
    if(pending.part_size!==5*1024*1024)throw Error('Unsupported upload format');
    for(let offset=0,part=1;offset<blob.size;offset+=pending.part_size,part++){
      const response=await fetch(api+'/vey/uploads/'+pending.id+'/parts/'+part,{method:'PUT',headers:{Authorization:'Bearer '+token,'Content-Type':'application/octet-stream'},body:blob.slice(offset,offset+pending.part_size),signal:uploadController.signal,redirect:'error',credentials:'omit',referrerPolicy:'no-referrer'});
      if(!response.ok){const e=await response.json();throw Error(e.error?.message||'Audio upload failed');}
    }
    const file=await json(api+'/vey/uploads/'+pending.id+'/complete','POST',token,'{}',uploadController.signal);complete=true;
    setCloudFile('cloud://'+file.id,blob);return JSON.stringify(file);
  }catch(e){throw String(e.message||e);}
  finally{if(pending&&!complete)await json(api+'/vey/files/'+pending.id,'DELETE',token,'').catch(()=>{});uploadController=undefined;}
}
export async function downloadAccountFile(api,token,id){
  try{
    if(!/^[a-f0-9-]{36}$/.test(id))throw Error('Invalid cloud file');
    const path='cloud://'+id;if(fileUrl(path))return path;
    uploadController=new AbortController();
    const response=await fetch(api+'/vey/files/'+id,{headers:{Authorization:'Bearer '+token},signal:AbortSignal.any([uploadController.signal,AbortSignal.timeout(300000)]),redirect:'error',credentials:'omit',referrerPolicy:'no-referrer',cache:'no-store'});
    if(!response.ok)throw Error('Audio download failed');
    const declared=Number(response.headers.get('Content-Length'));
    if(!Number.isSafeInteger(declared)||declared<1||declared>512*1024*1024)throw Error('Invalid cloud file size');
    const blob=await response.blob();if(blob.size!==declared)throw Error('Incomplete audio file');
    setCloudFile(path,blob);uploadController=undefined;return path;
  }catch(e){throw String(e.message||e);}
}
