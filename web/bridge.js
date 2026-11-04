import { CameraPresentation } from './player-motion.js';
const files = new Map();
export function installMediaSession(handler) {
  const session = navigator.mediaSession;
  if (!session) return false;
  for (const action of ['play','pause','previoustrack','nexttrack','seekto','seekbackward','seekforward']) {
    try { session.setActionHandler(action, details => handler(action, details?.seekTime ?? details?.seekOffset ?? 0)); }
    catch (_) { /* Some browsers omit individual media actions. */ }
  }
  return true;
}
export function setMediaMetadata(title, artist, album, artwork, duration) {
  const session = navigator.mediaSession;
  if (!session || typeof MediaMetadata === 'undefined') return;
  session.metadata = new MediaMetadata({
    title: title || 'Vey', artist: artist || '', album: album || '',
    artwork: artwork ? [{src: artwork, sizes: '512x512'}] : [],
  });
  if (Number.isFinite(duration) && duration > 0) {
    try { session.setPositionState({duration, position: 0, playbackRate: 1}); } catch (_) {}
  }
}
export function setMediaPlayback(playing, position, duration, rate) {
  const session = navigator.mediaSession;
  if (!session) return;
  session.playbackState = playing ? 'playing' : 'paused';
  if (Number.isFinite(duration) && duration > 0 && Number.isFinite(position)) {
    try { session.setPositionState({duration, position: Math.min(duration, Math.max(0, position)), playbackRate: rate || 1}); } catch (_) {}
  }
}
// Flush the latest coalesced Rust state before a hidden page is suspended.
export function onStoreFlush(callback) {
  addEventListener('pagehide', () => callback());
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) callback();
  });
}
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
    // Slint composites the page blur inside its rounded content viewport.
    braccatoMotion = new CameraPresentation(braccatoFrame, null, false);
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
  const cameraMotion = braccatoState.grouped && braccatoState.camera;
  const moving = braccatoState.visible && cameraMotion;
  braccatoMotion.update(cameraMotion ? {...cameraMotion,width:cameraMotion.width*sx,height:cameraMotion.height*sy} : null,!!braccatoState.reduced, cameraMotion?.blur !== false);
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
export function setCloudFile(path, blob) { if (!files.has(path)) files.set(path, URL.createObjectURL(blob)); }
function setFileUrl(path, file) {
  const previous = files.get(path);
  if (previous) URL.revokeObjectURL(previous);
  const url = URL.createObjectURL(file);
  files.set(path, url);
  return url;
}
function decodeId3Text(bytes) {
  if (!bytes.length) return '';
  const encoding = bytes[0];
  const payload = bytes.subarray(1);
  try {
    const decoder = encoding === 0 ? new TextDecoder('iso-8859-1')
      : encoding === 1 ? new TextDecoder('utf-16')
      : encoding === 2 ? new TextDecoder('utf-16be')
      : new TextDecoder('utf-8');
    return decoder.decode(payload).replace(/\0+$/g, '').trim();
  } catch (_) { return ''; }
}
async function readId3(file) {
  if (!/\.mp3$/i.test(file.name) || file.size < 10) return {};
  const bytes = new Uint8Array(await file.slice(0, Math.min(file.size, 16 * 1024 * 1024)).arrayBuffer());
  if (String.fromCharCode(...bytes.subarray(0, 3)) !== 'ID3' || ![3, 4].includes(bytes[3])) return {};
  const version = bytes[3];
  const tagSize = (bytes[6] & 0x7f) * 0x200000 + (bytes[7] & 0x7f) * 0x4000 + (bytes[8] & 0x7f) * 0x80 + (bytes[9] & 0x7f);
  const end = Math.min(bytes.length, 10 + tagSize);
  let cursor = 10;
  const tags = {};
  while (cursor + 10 <= end) {
    const id = String.fromCharCode(...bytes.subarray(cursor, cursor + 4));
    if (!/^[A-Z0-9]{4}$/.test(id)) break;
    const size = version === 4
      ? (bytes[cursor + 4] & 0x7f) * 0x200000 + (bytes[cursor + 5] & 0x7f) * 0x4000 + (bytes[cursor + 6] & 0x7f) * 0x80 + (bytes[cursor + 7] & 0x7f)
      : new DataView(bytes.buffer, bytes.byteOffset + cursor + 4, 4).getUint32(0);
    const start = cursor + 10, frameEnd = start + size;
    if (!size || frameEnd > end) break;
    const frame = bytes.subarray(start, frameEnd);
    if (id === 'TIT2') tags.title = decodeId3Text(frame);
    else if (id === 'TPE1') tags.artist = decodeId3Text(frame);
    else if (id === 'TALB') tags.album = decodeId3Text(frame);
    else if (id === 'APIC' && frame.length > 5) {
      const encoding = frame[0];
      let mimeEnd = 1;
      while (mimeEnd < frame.length && frame[mimeEnd] !== 0) mimeEnd++;
      const mime = new TextDecoder('ascii').decode(frame.subarray(1, mimeEnd)) || 'image/jpeg';
      let description = mimeEnd + 2;
      const wide = encoding === 1 || encoding === 2;
      while (description + (wide ? 1 : 0) < frame.length) {
        if (frame[description] === 0 && (!wide || frame[description + 1] === 0)) {
          description += wide ? 2 : 1;
          break;
        }
        description += wide ? 2 : 1;
      }
      if (description < frame.length) tags.cover = new Blob([frame.slice(description)], {type: mime});
    }
    cursor = frameEnd;
  }
  return tags;
}
function readBe32(bytes, offset) {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0);
}
function readFlacPicture(bytes) {
  try {
    let cursor = 4;
    const mimeLength = readBe32(bytes, cursor); cursor += 4;
    const mime = new TextDecoder('iso-8859-1').decode(bytes.subarray(cursor, cursor + mimeLength)) || 'image/jpeg';
    cursor += mimeLength;
    const descriptionLength = readBe32(bytes, cursor); cursor += 4 + descriptionLength;
    cursor += 16; // width, height, color depth, indexed colors
    const imageLength = readBe32(bytes, cursor); cursor += 4;
    if (!imageLength || cursor + imageLength > bytes.length) return null;
    return new Blob([bytes.subarray(cursor, cursor + imageLength)], {type: mime});
  } catch (_) { return null; }
}
function readVorbisComments(bytes, offset) {
  try {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let cursor = offset;
    const vendorLength = view.getUint32(cursor, true); cursor += 4 + vendorLength;
    const count = view.getUint32(cursor, true); cursor += 4;
    const tags = {};
    for (let index = 0; index < count && cursor + 4 <= bytes.length; index++) {
      const length = view.getUint32(cursor, true); cursor += 4;
      if (cursor + length > bytes.length) break;
      const comment = new TextDecoder().decode(bytes.subarray(cursor, cursor + length)); cursor += length;
      const split = comment.indexOf('=');
      if (split < 1) continue;
      const key = comment.slice(0, split).toUpperCase();
      const value = comment.slice(split + 1);
      if (key === 'TITLE') tags.title ||= value;
      else if (key === 'ARTIST') tags.artist ||= value;
      else if (key === 'ALBUM') tags.album ||= value;
      else if (key === 'METADATA_BLOCK_PICTURE' && !tags.cover) {
        try { tags.cover = readFlacPicture(Uint8Array.from(atob(value), char => char.charCodeAt(0))); } catch (_) {}
      } else if (key === 'COVERART' && !tags.cover) {
        try {
          const image = Uint8Array.from(atob(value), char => char.charCodeAt(0));
          const mime = tags.coverMime || 'image/jpeg';
          tags.cover = new Blob([image], {type: mime});
        } catch (_) {}
      } else if (key === 'COVERARTMIME') tags.coverMime = value;
    }
    return tags;
  } catch (_) { return {}; }
}
async function readFlac(file) {
  if (!/\.flac$/i.test(file.name) || file.size < 4) return {};
  const bytes = new Uint8Array(await file.slice(0, Math.min(file.size, 16 * 1024 * 1024)).arrayBuffer());
  if (String.fromCharCode(...bytes.subarray(0, 4)) !== 'fLaC') return {};
  const tags = {};
  let cursor = 4, last = false;
  while (!last && cursor + 4 <= bytes.length) {
    const header = bytes[cursor];
    last = !!(header & 0x80);
    const type = header & 0x7f;
    const size = (bytes[cursor + 1] << 16) | (bytes[cursor + 2] << 8) | bytes[cursor + 3];
    const start = cursor + 4, end = start + size;
    if (end > bytes.length) break;
    const block = bytes.subarray(start, end);
    if (type === 4) Object.assign(tags, {...readVorbisComments(block, 0), ...tags});
    else if (type === 6 && !tags.cover) tags.cover = readFlacPicture(block);
    cursor = end;
  }
  return tags;
}
async function readOgg(file) {
  if (!/\.(ogg|oga|opus)$/i.test(file.name) || file.size < 27) return {};
  const bytes = new Uint8Array(await file.slice(0, Math.min(file.size, 16 * 1024 * 1024)).arrayBuffer());
  const packets = [];
  let packetLength = 0, cursor = 0;
  while (cursor + 27 <= bytes.length) {
    if (String.fromCharCode(...bytes.subarray(cursor, cursor + 4)) !== 'OggS') break;
    const segmentCount = bytes[cursor + 26];
    const table = cursor + 27, payload = table + segmentCount;
    if (payload > bytes.length) break;
    let at = payload;
    for (let segment = 0; segment < segmentCount; segment++) {
      const length = bytes[table + segment];
      if (at + length > bytes.length) return {};
      packets.push(bytes.subarray(at, at + length)); packetLength += length; at += length;
      if (length < 255) {
        const packet = new Uint8Array(packetLength);
        let write = 0;
        for (const part of packets) { packet.set(part, write); write += part.length; }
        const vorbis = packet.length > 7 && packet[0] === 3 && String.fromCharCode(...packet.subarray(1, 7)) === 'vorbis';
        const opus = packet.length > 8 && String.fromCharCode(...packet.subarray(0, 8)) === 'OpusTags';
        if (vorbis) return readVorbisComments(packet, 7);
        if (opus) return readVorbisComments(packet, 8);
        packets.length = 0; packetLength = 0;
      }
    }
    cursor = at;
  }
  return {};
}
function parseMp4Metadata(bytes, start, end, tags) {
  let cursor = start;
  while (cursor + 8 <= end) {
    let size = readBe32(bytes, cursor);
    const type = String.fromCharCode(...bytes.subarray(cursor + 4, cursor + 8));
    let header = 8;
    if (size === 1) {
      if (cursor + 16 > end) break;
      const large = new DataView(bytes.buffer, bytes.byteOffset + cursor + 8, 8).getBigUint64(0);
      if (large > BigInt(Number.MAX_SAFE_INTEGER)) break;
      size = Number(large); header = 16;
    } else if (size === 0) size = end - cursor;
    if (size < header || cursor + size > end) break;
    const body = cursor + header, atomEnd = cursor + size;
    if (type === 'meta') parseMp4Metadata(bytes, body + 4, atomEnd, tags);
    else if (type === 'moov' || type === 'udta') parseMp4Metadata(bytes, body, atomEnd, tags);
    else if (type === 'ilst') {
      let item = body;
      while (item + 8 <= atomEnd) {
        const itemSize = readBe32(bytes, item);
        const itemType = String.fromCharCode(...bytes.subarray(item + 4, item + 8));
        if (itemSize < 8 || item + itemSize > atomEnd) break;
        let data = item + 8;
        while (data + 16 <= item + itemSize) {
          const dataSize = readBe32(bytes, data);
          const dataType = String.fromCharCode(...bytes.subarray(data + 4, data + 8));
          if (dataSize < 16 || data + dataSize > item + itemSize) break;
          const kind = readBe32(bytes, data + 8);
          const value = bytes.subarray(data + 16, data + dataSize);
          if (dataType === 'data') {
            if (itemType === '©nam') tags.title ||= new TextDecoder().decode(value).replace(/\0+$/g, '').trim();
            else if (itemType === '©ART') tags.artist ||= new TextDecoder().decode(value).replace(/\0+$/g, '').trim();
            else if (itemType === '©alb') tags.album ||= new TextDecoder().decode(value).replace(/\0+$/g, '').trim();
            else if (itemType === 'covr' && !tags.cover && value.length) {
              const mime = kind === 14 ? 'image/png' : 'image/jpeg';
              tags.cover = new Blob([value], {type: mime});
            }
          }
          data += dataSize;
        }
        item += itemSize;
      }
    }
    cursor += size;
  }
}
async function readMp4(file) {
  if (!/\.(m4a|mp4|m4b)$/i.test(file.name) || file.size < 16) return {};
  const windowSize = 16 * 1024 * 1024;
  const ranges = [file.slice(0, Math.min(file.size, windowSize))];
  if (file.size > windowSize) ranges.push(file.slice(Math.max(windowSize, file.size - windowSize), file.size));
  for (const range of ranges) {
    const bytes = new Uint8Array(await range.arrayBuffer());
    const tags = {};
    parseMp4Metadata(bytes, 0, bytes.length, tags);
    if (tags.title || tags.artist || tags.album || tags.cover) return tags;
    for (let index = 4; index + 4 <= bytes.length; index++) {
      if (bytes[index] !== 0x6d || bytes[index + 1] !== 0x6f || bytes[index + 2] !== 0x6f || bytes[index + 3] !== 0x76) continue;
      const offset = index - 4;
      if (offset < 0) continue;
      const size = readBe32(bytes, offset);
      if (size >= 8 && offset + size <= bytes.length) parseMp4Metadata(bytes, offset, offset + size, tags);
      if (tags.title || tags.artist || tags.album || tags.cover) return tags;
    }
  }
  return {};
}
async function readAudioTags(file) {
  if (/\.mp3$/i.test(file.name)) return readId3(file);
  if (/\.flac$/i.test(file.name)) return readFlac(file);
  if (/\.(ogg|oga|opus)$/i.test(file.name)) return readOgg(file);
  if (/\.(m4a|mp4|m4b)$/i.test(file.name)) return readMp4(file);
  return {};
}
async function trackFromFile(file) {
  const path = 'browser://' + encodeURIComponent(file.name) + ':' + file.size + ':' + file.lastModified;
  const url = setFileUrl(path, file);
  const tags = await readAudioTags(file).catch(() => ({}));
  let durationSecs = 0;
  try {
    durationSecs = await new Promise(resolve => {
      const audio = new Audio();
      const timeout = setTimeout(() => finish(0), 2500);
      const finish = value => { clearTimeout(timeout); audio.removeAttribute('src'); audio.load(); resolve(Number.isFinite(value) ? value : 0); };
      audio.preload = 'metadata';
      audio.onloadedmetadata = () => finish(audio.duration);
      audio.onerror = () => finish(0);
      audio.src = url;
    });
  } catch (_) { durationSecs = 0; }
  const thumbnailUrl = tags.cover ? URL.createObjectURL(tags.cover) : '';
  return {
    path,
    title: tags.title || file.name.replace(/\.[^.]+$/, ''),
    artist: tags.artist || 'Невідомий виконавець',
    album: tags.album || '',
    durationSecs,
    thumbnailUrl,
  };
}
// Host-only APIs. All UI, library state, queue and lyric parsing live in Rust.
export async function pickFiles() {
  return new Promise(resolve => {
    const input = document.createElement('input'); input.type = 'file'; input.accept = 'audio/*'; input.multiple = true;
    input.addEventListener('change', async () => resolve(JSON.stringify(await Promise.all([...input.files].map(trackFromFile)))));
    input.addEventListener('cancel', () => resolve('[]')); input.click();
  });
}
export async function pickSingleFile() {
  return new Promise(resolve => {
    const input = document.createElement('input'); input.type = 'file'; input.accept = 'audio/*';
    input.addEventListener('change', async () => resolve(input.files[0] ? JSON.stringify(await trackFromFile(input.files[0])) : ''));
    input.addEventListener('cancel', () => resolve('')); input.click();
  });
}
export function unlinkFile(path) {
  const url = files.get(path);
  if (url) URL.revokeObjectURL(url);
  files.delete(path);
}
const activeDownloads = new Map();
export function cancelDownload(id) { activeDownloads.get(id)?.abort(); }
export async function downloadTrack(id, filename, onProgress) {
  const controller = new AbortController();
  activeDownloads.set(id, controller);
  try {
    const response = await fetch(`/api/audio/${encodeURIComponent(id)}`, {signal: controller.signal});
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const total = Number(response.headers.get('content-length')) || 0;
    let blob;
    if (response.body?.getReader) {
      const reader = response.body.getReader();
      const chunks = [];
      let loaded = 0;
      while (true) {
        const {done, value} = await reader.read();
        if (done) break;
        chunks.push(value); loaded += value.byteLength;
        onProgress?.(total > 0 ? Math.min(100, Math.floor(loaded * 100 / total)) : -1);
      }
      blob = new Blob(chunks, {type: response.headers.get('content-type') || 'audio/mp4'});
    } else {
      onProgress?.(-1);
      blob = await response.blob();
    }
    onProgress?.(100);
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = (filename || 'Vey audio').replace(/[\\/:*?"<>|\u0000-\u001f]/g, '_');
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 30000);
    return true;
  } finally {
    if (activeDownloads.get(id) === controller) activeDownloads.delete(id);
  }
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
