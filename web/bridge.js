import { CameraPresentation } from './player-motion.js';
const files = new Map();
// DOM host for the original, locally bundled Braccato renderer. Playback and
// authenticated provider traffic remain in Rust.
let braccatoFrame;
let braccatoStage;
let braccatoMotion;
let braccatoPresentationKey = '';
let braccatoScaleX = 1, braccatoScaleY = 1;
let braccatoReady = false;
let braccatoMessageHandler;
let braccatoState = {};
const braccatoMessages = [];
let braccatoGeometryKey = '';
let braccatoGeometryDirty = true;
addEventListener('resize', () => { braccatoGeometryDirty = true; });
addEventListener('scroll', () => { braccatoGeometryDirty = true; }, true);
document.addEventListener('visibilitychange', () => {
  if (!document.hidden) { braccatoGeometryDirty = true; sendBraccato(); }
});
function sendBraccato() {
  if (braccatoReady && !document.hidden) {
    braccatoFrame.contentWindow.postMessage({channel:'playervf-braccato-update',state:braccatoState},location.origin);
    delete braccatoState.ambient;
    delete braccatoState.document;
    delete braccatoState.editorText;
  }
}
export function resetBraccato() {
  if (braccatoMessageHandler) removeEventListener('message', braccatoMessageHandler);
  braccatoFrame?.remove();
  braccatoStage?.remove();
  braccatoMotion?.cancel();
  braccatoStage = braccatoMotion = undefined;
  braccatoPresentationKey = '';
  braccatoFrame = undefined;
  braccatoReady = false;
  braccatoState = {};
  braccatoMessages.length = 0;
  braccatoGeometryKey = '';
  braccatoGeometryDirty = true;
}
export function updateBraccato(json) {
  const next = JSON.parse(json);
  if (next.navigation) syncNavigationHistory(next.navigation);
  // Keep an unsent artwork change when clock samples arrive in a hidden tab.
  if (next.ambient && braccatoState.ambient)
    next.ambient = {...braccatoState.ambient,...next.ambient};
  braccatoState = {...braccatoState,...next};
  if (!braccatoFrame) {
    braccatoFrame = document.createElement('iframe');
    braccatoFrame.title = 'Braccato lyrics';
    braccatoFrame.style.cssText = 'position:absolute;border:0;border-radius:20px;background:transparent;display:none;pointer-events:auto;will-change:transform';
    braccatoStage = document.createElement('div');
    braccatoStage.style.cssText = 'position:fixed;overflow:hidden;pointer-events:none;z-index:2';
    braccatoStage.appendChild(braccatoFrame);
    braccatoMotion = new CameraPresentation(braccatoFrame);
    braccatoMessageHandler = event => {
      if (event.source !== braccatoFrame.contentWindow || event.origin !== location.origin || event.data?.channel !== 'playervf-braccato') return;
      if (event.data.type === 'ready') { braccatoReady = true; sendBraccato(); }
      else if (['seek','error','edit','ambient'].includes(event.data.type)) {
        if (event.data.type === 'ambient') {
          const previous = braccatoMessages.findIndex(message => message.type === 'ambient');
          if (previous >= 0) braccatoMessages.splice(previous,1);
        }
        braccatoMessages.push(event.data);
        if (braccatoMessages.length > 100) braccatoMessages.shift();
      }
    };
    addEventListener('message', braccatoMessageHandler);
    braccatoFrame.src = new URL('./braccato/index.html', location.href).href;
    document.body.appendChild(braccatoStage);
  }
  if (document.hidden) return braccatoReady;
  const bounds = braccatoState.bounds;
  const stage = braccatoState.stageBounds || {x:0,y:0,width:braccatoState.windowWidth,height:braccatoState.windowHeight,radius:0};
  const geometryKey = JSON.stringify([bounds,stage,braccatoState.overlay,braccatoState.windowWidth,braccatoState.windowHeight]);
  if (braccatoGeometryDirty || geometryKey !== braccatoGeometryKey) {
    const canvas = document.querySelector('canvas').getBoundingClientRect();
    const sx = canvas.width / (braccatoState.windowWidth || canvas.width);
    const sy = canvas.height / (braccatoState.windowHeight || canvas.height);
    braccatoScaleX = sx; braccatoScaleY = sy;
    Object.assign(braccatoFrame.style, {
      left: `${(bounds.x-stage.x) * sx}px`, top: `${(bounds.y-stage.y) * sy}px`,
      width: `${Math.max(1,bounds.width * sx)}px`, height: `${Math.max(1,bounds.height * sy)}px`,
    });
    Object.assign(braccatoStage.style, {
      left:`${canvas.left+stage.x*sx}px`,top:`${canvas.top+stage.y*sy}px`,
      width:`${stage.width*sx}px`,height:`${stage.height*sy}px`,
      borderRadius:`${stage.radius*Math.min(sx,sy)}px`,
    });
    const overlay = braccatoState.overlay;
    if (overlay) {
      const w = Math.max(1,bounds.width*sx), h = Math.max(1,bounds.height*sy);
      const x = (overlay.x-bounds.x)*sx, y = (overlay.y-bounds.y)*sy;
      const right = x+overlay.width*sx, bottom = y+overlay.height*sy;
      const r = 20*Math.min(sx,sy);
      // A hole also routes pointer events to the Slint controls underneath.
      braccatoFrame.style.clipPath = `path(evenodd, "M0 0H${w}V${h}H0Z M${x+r} ${y}H${right-r}Q${right} ${y} ${right} ${y+r}V${bottom-r}Q${right} ${bottom} ${right-r} ${bottom}H${x+r}Q${x} ${bottom} ${x} ${bottom-r}V${y+r}Q${x} ${y} ${x+r} ${y}Z")`;
    } else braccatoFrame.style.clipPath = '';
    braccatoGeometryKey = geometryKey;
    braccatoGeometryDirty = false;
  }
  const sx = braccatoScaleX, sy = braccatoScaleY;
  const moving = braccatoState.visible && braccatoState.grouped && braccatoState.camera;
  braccatoMotion.update(moving ? {...moving,width:moving.width*sx,height:moving.height*sy} : null,!!braccatoState.reduced);
  const presentationKey = braccatoState.visible ? JSON.stringify([!!moving,moving ? 0 : braccatoState.presentationX,moving ? 0 : braccatoState.presentationY,braccatoState.presentationOpacity]) : 'hidden';
  if (braccatoState.visible && presentationKey !== braccatoPresentationKey) {
    if (!moving) braccatoFrame.style.transform = braccatoState.grouped ? `translate(${(braccatoState.presentationX||0)*sx}px,${(braccatoState.presentationY||0)*sy}px)` : 'none';
    braccatoFrame.style.opacity = String(braccatoState.presentationOpacity ?? 1);
    braccatoFrame.style.pointerEvents = moving ? 'none' : 'auto';
  }
  braccatoPresentationKey = presentationKey;
  const display = braccatoState.visible && braccatoReady && bounds.width > 8 && bounds.height > 8 ? 'block' : 'none';
  if (braccatoFrame.style.display !== display) braccatoFrame.style.display = display;
  sendBraccato();
  return braccatoReady;
}
export function drainBraccatoMessages() { return JSON.stringify(braccatoMessages.splice(0)); }
export function fileUrl(path) { return files.get(path) || ""; }
// Host-only APIs. All UI, library state, queue and lyric parsing live in Rust.
export async function pickFiles() {
  return new Promise(resolve => {
    const input = document.createElement('input'); input.type = 'file'; input.accept = 'audio/*'; input.multiple = true;
    input.addEventListener('change', () => resolve(JSON.stringify([...input.files].map(f => (() => { const path='browser://'+encodeURIComponent(f.name)+':'+f.size+':'+f.lastModified; files.set(path,URL.createObjectURL(f)); return {path, title: f.name.replace(/\.[^.]+$/, ''), artist: 'Невідомий виконавець', album: '', durationSecs: 0, thumbnailUrl: ''}; })()))));
    input.addEventListener('cancel', () => resolve('[]')); input.click();
  });
}
export async function readJsonFile() {
  return new Promise((resolve, reject) => {
    const input=document.createElement('input');input.type='file';input.accept='.json';
    input.addEventListener('change',async()=>{try{resolve(await input.files[0].text());}catch(e){reject(e);}});
    input.addEventListener('cancel',()=>resolve(''));input.click();
  });
}
export function downloadJson(text) {
  const url=URL.createObjectURL(new Blob([text],{type:'application/json'}));
  const link=document.createElement('a');link.href=url;link.download='playervf-backup.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
}
export function challenge() {
  return new Promise((resolve,reject)=>{
    const modal=document.createElement('dialog');const frame=document.createElement('iframe');
    frame.src='https://lyrics.api.dacubeking.com/challenge';frame.style.cssText='width:450px;max-width:80vw;height:300px;border:0';modal.append(frame);document.body.append(modal);modal.showModal();
    const cleanup=()=>{clearTimeout(timeout);removeEventListener('message',onMessage);modal.remove();};
    const onMessage=e=>{if(e.origin!=='https://lyrics.api.dacubeking.com'||e.source!==frame.contentWindow)return;if(e.data?.type==='turnstile-token'&&typeof e.data.token==='string'){const token=e.data.token;cleanup();resolve(token);}};
    const timeout=setTimeout(()=>{cleanup();reject(Error('Час перевірки вичерпано'));},120000);
    modal.addEventListener('cancel',()=>{cleanup();reject(Error('Перевірку закрито'));});addEventListener('message',onMessage);
  });
}

// CSS owns platform safe-area values. Read only on startup and viewport changes.
let insetProbe;
export function viewportInsets() {
  if (!insetProbe) {
    insetProbe=document.createElement('div');
    insetProbe.style.cssText='position:fixed;visibility:hidden;pointer-events:none;padding:env(safe-area-inset-top,0px) env(safe-area-inset-right,0px) env(safe-area-inset-bottom,0px) env(safe-area-inset-left,0px)';
    document.body.append(insetProbe);
  }
  const style=getComputedStyle(insetProbe);
  return JSON.stringify([style.paddingTop,style.paddingBottom,style.paddingLeft,style.paddingRight].map(v=>Math.max(0,parseFloat(v)||0)));
}

// Back/forward gestures use the same Rust navigation and settings transitions.
let navigationKey='';
let historyTarget='';
let historyTargetUntil=0;
const routeKey=nav=>`${nav.view}:${nav.view==='settings' ? nav.section||'' : ''}`;
function syncNavigationHistory(nav) {
  const key=routeKey(nav);
  if(historyTarget) {
    if(key===historyTarget) { navigationKey=key;historyTarget='';return; }
    if(performance.now()<historyTargetUntil)return;
    historyTarget='';
  }
  if(key===navigationKey)return;
  const state={...(typeof history.state==='object' ? history.state : {}),vey:{view:nav.view,section:nav.section||''}};
  if(navigationKey)history.pushState(state,'');else history.replaceState(state,'');
  navigationKey=key;
}
addEventListener('popstate',event=>{
  const nav=event.state?.vey;
  if(!nav)return;
  historyTarget=routeKey(nav);historyTargetUntil=performance.now()+1000;
  braccatoMessages.push({type:'navigation',view:nav.view,section:nav.section||''});
});
