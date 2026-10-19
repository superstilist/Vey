import { Kawarp } from '@kawarp/core';

// Slint owns the composition. Only this small, decorative texture crosses IPC.
const width = 256, height = 144;
let renderer: Kawarp | undefined;
let canvas: HTMLCanvasElement;
let failed = false;
let lastSource = '';
let pixels = new Uint8Array(width * height * 4);
let upright = new Uint8Array(pixels.length);
const warped = new Uint8Array(pixels.length);
let renderedTime = NaN;
let encodedFrame = '';
let frameSettled = false;
let revision = 0;
let backdropKey = '';
let backdropCanvas: HTMLCanvasElement | undefined;
let sourceCanvas: HTMLCanvasElement | undefined;
let sourceRevision = -1;
const morphDuration = 1400;
const artworkGrace = 900;
const morphFrom = new Uint8Array(width * height * 4);
let morphStarted = -Infinity;
let pendingSource: string | undefined;
let pendingAt = 0;

export function renderAmbient(state: any, send: (message: object) => void) {
  if (!state || failed) return;
  try {
    if (!renderer) {
      canvas = document.createElement('canvas');
      canvas.width = width; canvas.height = height;
      renderer = new Kawarp(canvas, { warpIntensity: 0.8, blurPasses: 12,
        saturation: 1.25, tintIntensity: 0.18, transitionDuration: 0, scale: 1.15 });
      renderer.loadGradient(['#273c59', '#73516d', '#ae785b', '#243848']);
    }
    const now = performance.now();
    if (state.source !== undefined && state.source !== lastSource) {
      lastSource = state.source;
      pendingSource = lastSource;
      // A missing image often means the next cover is still downloading.
      pendingAt = now + (!lastSource && revision > 0 && !state.reduced ? artworkGrace : 0);
    }
    if (pendingSource !== undefined && (now >= pendingAt || state.reduced)) {
      // Snapshot the displayed mixture, including an interrupted transition.
      // Kawarp still blurs only when an artwork source actually changes.
      morphFrom.set(upright);
      morphStarted = revision > 0 && !state.reduced ? now : -Infinity;
      if (pendingSource) {
        const bytes = Uint8Array.from(atob(pendingSource), c => c.charCodeAt(0));
        renderer.loadImageData(bytes, 64, 64);
      } else renderer.loadGradient(['#273c59', '#73516d', '#ae785b', '#243848']);
      pendingSource = undefined;
      renderedTime = NaN;
    }
    if (state.reduced) morphStarted = -Infinity;
    // Paused palette morphs share one GPU target. Keep it separate from the
    // displayed blend so intermediate frames never require another readback.
    const targetChanged = renderedTime !== state.time;
    if (targetChanged) {
      renderer.renderFrame(state.time);
      const gl = canvas.getContext('webgl')!;
      gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      const stride = width * 4;
      for (let y = 0; y < height; y++) {
        warped.set(pixels.subarray(y * stride, (y + 1) * stride), (height - y - 1) * stride);
      }
      renderedTime = state.time;
    }
    const t = Math.min(1, Math.max(0, (now - morphStarted) / morphDuration));
    if (!targetChanged && t === 1 && frameSettled) {
      send({ type: 'ambient', pixels: encodedFrame });
      return;
    }
    if (t < 1) {
      // Smootherstep has zero velocity and acceleration at both ends. Blend
      // the small IPC texture, keeping the GPU warp and global clock intact.
      const mix = t * t * t * (t * (t * 6 - 15) + 10);
      for (let i = 0; i < upright.length; i++)
        upright[i] = Math.round(morphFrom[i] + (warped[i] - morphFrom[i]) * mix);
    } else upright.set(warped);
    frameSettled = t === 1;
    revision++;
    let encoded: string;
    if (typeof (upright as any).toBase64 === 'function') encoded = (upright as any).toBase64();
    else {
      let binary = '';
      for (let i = 0; i < upright.length; i += 8192)
        binary += String.fromCharCode(...upright.subarray(i, i + 8192));
      encoded = btoa(binary);
    }
    encodedFrame = encoded;
    send({ type: 'ambient', pixels: encoded });
  } catch (error) {
    failed = true;
    renderer?.dispose();
    console.warn('Kawarp unavailable; using the static ambient background.', error);
  }
}

// Opaque Windows child: sample the same global background at this viewport,
// rather than asking DWM to blend a transparent HWND with an OpenGL swap chain.
export function paintNativeBackdrop(backdrop: any) {
  if (!backdrop) return;
  const key = JSON.stringify([backdrop, revision]);
  if (key === backdropKey) return;
  backdropKey = key;
  if (!backdropCanvas) {
    backdropCanvas = document.createElement('canvas');
    backdropCanvas.id = 'native-backdrop';
    backdropCanvas.width = width; backdropCanvas.height = height;
    backdropCanvas.style.cssText = 'position:fixed;inset:0;width:100%;height:100%;pointer-events:none;z-index:0';
    document.body.style.background = '#080808';
    document.body.prepend(backdropCanvas);
    sourceCanvas = document.createElement('canvas');
    sourceCanvas.width = width; sourceCanvas.height = height;
  }
  const c = backdrop.content, v = backdrop.viewport;
  if (c.width <= 0 || c.height <= 0 || v.width <= 0 || v.height <= 0) return;
  const ctx = backdropCanvas.getContext('2d')!;
  ctx.setTransform(width / v.width, 0, 0, height / v.height,
    -(v.x - c.x) * width / v.width, -(v.y - c.y) * height / v.height);
  ctx.fillStyle = /^#[0-9a-f]{6}$/i.test(backdrop.surface) ? backdrop.surface : '#171c15';
  ctx.fillRect(0,0,c.width,c.height);
  ctx.globalAlpha = 0.06;
  if (revision > 0) {
    if (sourceRevision !== revision) {
      sourceCanvas!.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(upright.buffer),width,height),0,0);
      sourceRevision = revision;
    }
    ctx.drawImage(sourceCanvas!,0,0,c.width,c.height);
  }
  ctx.globalAlpha = 1;
}

export function disposeAmbient() {
  renderer?.dispose(); renderer = undefined;
  backdropCanvas?.remove(); backdropCanvas = undefined; sourceCanvas = undefined;
  revision = 0; sourceRevision = -1; backdropKey = ''; lastSource = ''; failed = false;
  morphStarted = -Infinity; pendingSource = undefined; pendingAt = 0;
  renderedTime = NaN; encodedFrame = ''; frameSettled = false;
}
