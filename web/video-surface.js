// Video is a muted visual slave. The existing song audio owns transport/time.
export class VideoSurface {
  constructor(parent = document.body) {
    this.video = document.createElement('video');
    this.video.muted = true;
    this.video.defaultMuted = true;
    this.video.playsInline = true;
    this.video.preload = 'metadata';
    this.video.setAttribute('aria-hidden', 'true');
    this.video.style.cssText = 'position:fixed;display:none;object-fit:contain;background:#000;border-radius:24px;pointer-events:none;z-index:2';
    parent.append(this.video);
    this.failed = false;
    this.url = '';
    this.video.addEventListener('error', () => { this.failed = true; this.video.style.display = 'none'; });
    for (const event of ['loadedmetadata','loadeddata','canplay','seeked'])
      this.video.addEventListener(event, () => this.update(this.state));
  }
  update(state) {
    if (!state) return false;
    this.state = state;
    const v = this.video;
    if (this.url !== state.url) {
      v.pause(); this.failed = false; this.url = state.url || '';
      if (this.url) v.src = this.url;
      else { v.removeAttribute('src'); v.load(); }
    }
    const visible = !!state.visible && !!this.url && !this.failed && !document.hidden;
    if (!visible) { v.pause(); v.style.display = 'none'; return false; }
    if (Number.isFinite(v.duration) && v.duration > 0 && state.position >= v.duration) {
      v.pause(); v.style.display='none'; return false;
    }
    let b = state.bounds;
    if (!state.native) {
      const canvas = document.querySelector('canvas')?.getBoundingClientRect();
      if (!canvas) return false;
      const sx = canvas.width / state.windowWidth, sy = canvas.height / state.windowHeight;
      b = {x:canvas.left+b.x*sx,y:canvas.top+b.y*sy,width:b.width*sx,height:b.height*sy};
    }
    Object.assign(v.style,{left:`${b.x}px`,top:`${b.y}px`,width:`${b.width}px`,height:`${b.height}px`});
    const target = Math.max(0, Math.min(state.position, Number.isFinite(v.duration) ? v.duration : state.position));
    if (v.readyState >= 1 && !v.seeking && Math.abs(v.currentTime-target) > (state.playing ? 0.25 : 0.04)) v.currentTime = target;
    v.playbackRate = Math.max(0.25,Math.min(4,state.rate || 1));
    v.muted = true;
    if (state.playing && v.paused && !v.ended) v.play().catch(() => { v.style.display = 'none'; });
    if (!state.playing) v.pause();
    const ready = v.readyState >= 2 && !v.ended;
    v.style.display = ready ? 'block' : 'none';
    return ready;
  }
  dispose() { this.video.pause(); this.video.removeAttribute('src'); this.video.load(); this.video.remove(); }
}
let surface;
export function updateVideoSurface(json) {
  surface ||= new VideoSurface();
  return surface.update(JSON.parse(json));
}
export function videoSurfaceDiagnostics() {
  if (!surface) return null;
  const {visible,playing,position,rate,bounds}=surface.state || {};
  const v=surface.video;
  return {visible,playing,position,rate,bounds,currentTime:v.currentTime,paused:v.paused,ended:v.ended,ready:v.readyState,failed:surface.failed,display:v.style.display};
}
document.addEventListener('visibilitychange', () => { if (surface) surface.update(surface.state); });
