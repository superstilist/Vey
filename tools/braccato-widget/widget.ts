// Host adapter only. The renderer, effects, parsers, highlighter, provider chain
// and RICS compiler below are the unmodified upstream implementations.
import * as element from '../../third-party/braccato/packages/core/src/element';
import * as core from '../../third-party/braccato/packages/core/src/index';
import * as constants from '../../third-party/braccato/packages/core/src/constants';
import * as text from '../../third-party/braccato/packages/core/src/text';
import * as themeSettings from '../../third-party/braccato/packages/core/src/themeSettings';
import * as util from '../../third-party/braccato/packages/core/src/util';
import * as parsers from '../../third-party/braccato/packages/parsers/src/index';
import * as format from '../../third-party/braccato/packages/parsers/src/format';
import * as highlight from '../../third-party/braccato/packages/highlight/src/index';
import * as providers from '../../third-party/braccato/packages/provider-blyrics/src/index';
import '../../third-party/braccato/packages/core/src/styles/variables.css';
import '../../third-party/braccato/packages/core/src/styles/lyrics.css';
import '../../third-party/braccato/packages/core/src/styles/instrumental.css';
import '../../third-party/braccato/packages/core/src/styles/stage.css';
import '../../third-party/braccato/packages/highlight/src/highlight.css';
import './widget.css';
import { renderAmbient, paintNativeBackdrop, disposeAmbient } from './ambient';
import { PlaybackClock } from './clock';
import { CameraPresentation } from '../../web/player-motion.js';
import catalog from '../../locales/catalog.tsv';
const locales = ['uk','en','cs','pl','de','es','fr','ja'];
const messages = new Map(catalog.trimEnd().split('\n').map((line:string) => {const fields=line.replace(/\r$/, '').split('\t');return [fields[0],fields];}));
const tr = (text:string) => messages.get(text)?.[Math.max(0,locales.indexOf(state.language || 'uk'))] || text;

export const upstream = { core, element, constants, text, themeSettings, util, parsers, format, highlight, providers };
export const revision = '1e6b6954b1c18136274681a9bf4985efb2332b01';
const scroller = document.querySelector('#scroller') as HTMLElement;
const resume = document.querySelector('#resume') as HTMLButtonElement;
const editorPane = document.querySelector('#editor-pane') as HTMLElement;
const editorInput = document.querySelector('#editor-input') as HTMLTextAreaElement;
const editor = highlight.attachEditor(editorInput);
const view = document.createElement('braccato-lyrics') as any;
let state: any = { visible: false, position: 0, playing: false, rate: 1, offset: 0 };
const clock = new PlaybackClock();
let documentKey: string | null = null;
let themeKey = '';
let frame = 0;
let themeCompiles = 0;
let builds = 0;
let lyrics: any[] = [];
let ticks = 0;
let tickOptionsUpdates = 0;
let tickConfig = '';
let lastTickTime = NaN;
let lastTickPlaying = false;
let visibilityAnimation: Animation | undefined;
let presentationKey = '';
const cameraPresentation = new CameraPresentation(scroller);
scroller.style.willChange = 'transform, opacity';

editorInput.addEventListener('input', () => send({type:'edit',text:editorInput.value}));
document.querySelector('#pretty')!.addEventListener('click', () => {
  editorInput.value = highlight.prettyTtml(editorInput.value);
  editor.refresh();
  send({type:'edit',text:editorInput.value});
});

function send(message: object) {
  const host = (window as any).ipc;
  if (host?.postMessage) host.postMessage(JSON.stringify(message));
  else if (parent !== window) parent.postMessage({ channel: 'playervf-braccato', ...message }, location.origin);
  window.dispatchEvent(new CustomEvent('playervf:braccato-message', { detail: message }));
}
view.host = {
  isViewVisible: () => state.visible && !document.hidden,
  getScrollElement: () => scroller,
  seek: (seconds: number) => send({ type: 'seek', seconds: Math.max(0, seconds - state.offset / 1000) }),
  setResumeAffordanceVisible: (visible: boolean) => { resume.hidden = !visible; },
  log: (...args: any[]) => console.debug('[braccato]', ...args),
};
view.addEventListener('braccato:error', (event: any) => send({ type: 'error', message: `${event.detail.phase}: ${event.detail.error.message}`.slice(0, 1000) }));
view.addEventListener('braccato:lyrics-loaded', () => {
  builds++;
  const renderer = view.renderer;
  if (!renderer) return;
  renderer.lines.forEach((line: any, index: number) => {
    const item = lyrics[index];
    if (!item) return;
    const translations = item.translations || {};
    const language = ['uk','translation','en'].find(lang => translations[lang]) || Object.keys(translations)[0];
    const translation = translations[language] || item.translation?.text;
    if (translation) core.injectTranslation(document,line.lyricElement,translation,language || item.translation?.lang);
    const romanization = item.romanization || item.timedRomanization?.map((p:any)=>p.words).join('');
    if (romanization) core.injectRomanization(document,line.lyricElement,line,romanization,item.timedRomanization);
  });
  renderer.scheduleLyricPositionUpdate(() => state.visible && state.mode !== 'editor',tick);
});
scroller.appendChild(view);
// Pause autoscroll only after the lyrics actually move. A touch/pointer down
// also happens on taps, which used to suspend following playback for 25 sec.
scroller.addEventListener('scroll', () => view.renderer?.noteUserScroll(), { passive: true });
resume.addEventListener('click', () => view.renderer?.resumeAutoscroll());

function tick() {
  if (!state.visible || state.mode === 'editor' || document.hidden) return;
  const time = clock.read(performance.now());
  if (time === lastTickTime && lastTickPlaying === !!state.playing) return;
  lastTickTime = time;
  lastTickPlaying = !!state.playing;
  if (view.playing !== lastTickPlaying) view.playing = lastTickPlaying;
  view.currentTime = time;
  ticks++;
}
function animate() {
  frame = 0;
  tick();
  if (state.visible && state.mode !== 'editor' && state.playing && !document.hidden) frame = requestAnimationFrame(animate);
}
function requestTick() {
  if (!frame && state.visible && state.mode !== 'editor' && !document.hidden) frame = requestAnimationFrame(animate);
}

export function update(next: any) {
  renderAmbient(next.ambient, send);
  const wasVisible = state.visible;
  const wasEditing = state.mode === 'editor';
  const oldLanguage = state.language;
  state = { ...state, ...next };
  if (state.language !== oldLanguage) {
    document.documentElement.lang = state.language || 'uk';
    resume.textContent = tr('Повернутися до слів');
    editorInput.setAttribute('aria-label',tr('Текст пісні'));
    document.querySelector('#pretty')!.textContent = tr('Форматувати TTML');
  }
  const cameraMoving = cameraPresentation.update(!state.grouped && state.visible && state.mode !== 'editor' ? state.camera : null, !!state.reduced);
  const presentation = JSON.stringify([!!state.grouped,state.presentationOpacity ?? 1,state.grouped ? 0 : cameraMoving ? 'camera' : state.presentationX ?? 0,state.grouped ? 0 : cameraMoving ? 'camera' : state.presentationY ?? 0,!!state.reduced]);
  if (presentation !== presentationKey) {
    presentationKey = presentation;
    scroller.style.opacity = String(state.presentationOpacity ?? 1);
    // Move the composited content, keeping native child/iframe bounds stable.
    scroller.style.transition = state.grouped || state.reduced || cameraMoving ? 'none' : 'transform 32ms linear, opacity 32ms linear';
    if (!cameraMoving) scroller.style.transform = state.grouped ? 'none' : `translate(${state.presentationX ?? 0}px, ${state.presentationY ?? 0}px)`;
    scroller.style.pointerEvents = state.presentationOpacity === 0 || cameraMoving ? 'none' : '';
  }
  if (state.visible && state.mode !== 'editor') paintNativeBackdrop(state.nativeBackdrop);
  const editing = state.mode === 'editor';
  const optionsKey = JSON.stringify([state.rate, state.offset, state.passive, state.reduced]);
  if (optionsKey !== tickConfig) {
    tickConfig = optionsKey;
    view.tickOptions = { playbackRate: state.rate, lyricOffset: -state.offset / 1000,
      passiveScrollEnabled: !!state.passive, eventCreationTime: -1, smoothScroll: !state.reduced };
    tickOptionsUpdates++;
    lastTickTime = NaN;
  }
  if (editorPane.hidden !== !editing) editorPane.hidden = !editing;
  if (scroller.hidden !== editing) scroller.hidden = editing;
  if (editing) {
    resume.hidden = true;
    if (next.editorText !== undefined && editorInput.value !== next.editorText) {
      const start = editorInput.selectionStart, end = editorInput.selectionEnd;
      editorInput.value = next.editorText;
      editor.refresh();
      if (document.activeElement === editorInput) editorInput.setSelectionRange(start,end);
    }
    if (!wasEditing) editor.refresh();
  }
  if (next.position !== undefined && (next.position !== clock.position || state.playing !== clock.playing || state.rate !== clock.rate)) {
    clock.sample(state.position, state.playing, state.rate, performance.now());
  }
  const layout = state.stage && state.timed !== false ? 'stage' : 'scroll';
  if (view.layout !== layout) view.layout = layout;
  const requestedSize = Number.isFinite(state.size) ? state.size : 48;
  const size = state.mobile ? Math.min(Math.max(requestedSize, 28), 32) : Math.min(Math.max(requestedSize, 40), 56);
  const disableRichsync = !!state.disableRichsync;
  const config = JSON.stringify([size, state.reduced, state.passiveSeconds, state.scrollRatio, state.lookahead, state.scrollDuration, disableRichsync, state.richsyncOffset, state.lineOffset]);
  if (config !== themeKey) {
    themeKey = config;
    themeCompiles++;
    const motion = state.reduced ? `:root,.blyrics-container{--blyrics-animate-line-scale:0;--blyrics-animate-word-wobble:0;--blyrics-animate-instrumental:0;--blyrics-animate-decoration-entry:0;--blyrics-scale:1;--blyrics-active-scale:1;}` : '';
    const settings = `/* blyrics-target-scroll-pos-ratio=${state.scrollRatio ?? 0.37}; blyrics-early-scroll-consider-s=${(state.lookahead ?? 540) / 1000}; blyrics-line-scroll-duration=${state.scrollDuration ?? 750}ms; blyrics-passive-scroll-seconds-per-line=${state.passiveSeconds || 3.5}; blyrics-line-synced-animation-delay=0ms; blyrics-swipe-lead-ratio=0; blyrics-swipe-duration-ratio=1; ${disableRichsync ? 'blyrics-disable-richsync=true;' : ''} */`;
    // The upstream default adds 150 ms to rich-sync cues. PlayerVF's native
    // renderer uses the configured value directly (0 ms by default), so pass
    // the same offset into Braccato to keep word highlights aligned with audio.
    const timing = `.blyrics-container{--blyrics-richsync-timing-offset:${state.richsyncOffset ?? 0}ms;--blyrics-timing-offset:${state.lineOffset ?? 0}ms;}`;
    const sizeStyle = `.blyrics-container{--blyrics-font-size:${size}px;--blyrics-translated-font-size:calc(var(--blyrics-font-size) * 0.667);}`;
    view.theme = `${settings}\n${sizeStyle}\n${motion}\n${timing}`;
  }
  if (next.document !== undefined) {
    const key = typeof next.document === 'string' ? next.document : JSON.stringify(next.document);
    if (key !== documentKey) {
      try {
        const data = typeof next.document === 'string' ? JSON.parse(next.document) : next.document;
        if (!data || !Array.isArray(data.lyrics)) throw Error('Invalid lyric document');
        lyrics = data.lyrics;
        view.lyricsOptions = { loaderVisible: false, noLyrics: false, ...(data.options || {}) };
        view.lyrics = lyrics;
        documentKey = key;
        lastTickTime = NaN;
        scroller.scrollTop = 0;
      } catch (error) {
        send({type:'error',message:`Lyrics: ${String(error).slice(0,900)}`});
      }
    }
  }
  if (wasVisible !== state.visible) {
    visibilityAnimation?.cancel();
    if (state.visible && !state.reduced && state.presentationOpacity === undefined) {
      visibilityAnimation = view.animate([{opacity:0},{opacity:1}],{
        duration:180,easing:'cubic-bezier(0.2, 0.8, 0.2, 1)'
      });
    }
    lastTickTime = NaN;
    view.renderer?.noteVisibilityChange();
    if (state.visible) view.renderer?.relayout();
    else if (frame) { cancelAnimationFrame(frame); frame = 0; }
  }
  const playing = state.visible && !editing && state.playing && !document.hidden;
  if (!playing) {
    if (frame) { cancelAnimationFrame(frame); frame = 0; }
    if (view.playing) view.playing = false;
  }
  if (!frame) tick();
  if (playing) requestTick();
}

export function diagnostics() {
  return { revision, status: view.status, currentTime: view.currentTime, syncType: view.renderer?.syncType,
    lineCount: view.renderer?.lines.length || 0, layout: view.layout,
    letters: view.querySelectorAll('.blyrics--letter').length,
    animations: document.getAnimations().length, themeCompiles, builds, ticks, tickOptionsUpdates,
    scrollTop: scroller.scrollTop, visible: state.visible,
    packages: Object.keys(upstream), mode: state.mode, html: view.innerHTML };
}
export function destroy() {
  cameraPresentation.cancel();
  visibilityAnimation?.cancel();
  disposeAmbient();
  state.visible = false;
  if (frame) cancelAnimationFrame(frame);
  editor.destroy();
  view.remove();
}
document.addEventListener('visibilitychange', () => {
  view.renderer?.noteVisibilityChange();
  lastTickTime = NaN;
  if (document.hidden) {
    if (frame) { cancelAnimationFrame(frame); frame = 0; }
    if (view.playing) view.playing = false;
  } else if (state.playing) requestTick();
});
addEventListener('message', event => {
  if (event.source === parent && event.origin === location.origin && event.data?.channel === 'playervf-braccato-update') update(event.data.state);
});
send({ type: 'ready', revision });
