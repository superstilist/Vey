const files = new Map();
// DOM host for the original, locally bundled Braccato renderer. Playback and
// authenticated provider traffic remain in Rust.
let braccatoFrame;
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
  braccatoFrame = undefined;
  braccatoReady = false;
  braccatoState = {};
  braccatoMessages.length = 0;
  braccatoGeometryKey = '';
  braccatoGeometryDirty = true;
}
export function updateBraccato(json) {
  const next = JSON.parse(json);
  // Keep an unsent artwork change when clock samples arrive in a hidden tab.
  if (next.ambient && braccatoState.ambient)
    next.ambient = {...braccatoState.ambient,...next.ambient};
  braccatoState = {...braccatoState,...next};
  if (!braccatoFrame) {
    braccatoFrame = document.createElement('iframe');
    braccatoFrame.title = 'Braccato lyrics';
    braccatoFrame.style.cssText = 'position:fixed;border:0;border-radius:20px;background:transparent;display:none;z-index:2';
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
    document.body.appendChild(braccatoFrame);
  }
  if (document.hidden) return braccatoReady;
  const bounds = braccatoState.bounds;
  const geometryKey = JSON.stringify([bounds,braccatoState.overlay,braccatoState.windowWidth,braccatoState.windowHeight]);
  if (braccatoGeometryDirty || geometryKey !== braccatoGeometryKey) {
    const canvas = document.querySelector('canvas').getBoundingClientRect();
    const sx = canvas.width / (braccatoState.windowWidth || canvas.width);
    const sy = canvas.height / (braccatoState.windowHeight || canvas.height);
    Object.assign(braccatoFrame.style, {
      left: `${canvas.left + bounds.x * sx}px`, top: `${canvas.top + bounds.y * sy}px`,
      width: `${Math.max(1,bounds.width * sx)}px`, height: `${Math.max(1,bounds.height * sy)}px`,
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
  braccatoFrame.style.display = braccatoState.visible && braccatoReady && bounds.width > 8 && bounds.height > 8 ? 'block' : 'none';
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
