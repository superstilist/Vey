const ART_CACHE = 'vey-artwork-v1';
const ART_CACHE_LIMIT = 128;
const ART_CACHE_ITEM_BYTES = 2 * 1024 * 1024;
const ART_CACHE_TTL_MS = 7 * 24 * 60 * 60 * 1000;

async function cachedArtwork(url, maxBytes) {
  if (!globalThis.caches) return null;
  try {
    const cache = await caches.open(ART_CACHE);
    const response = await cache.match(url);
    if (!response) return null;
    const saved = Number(response.headers.get('X-Vey-Cached-At'));
    if (!saved || Date.now() - saved > ART_CACHE_TTL_MS) {
      await cache.delete(url);
      return null;
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.length <= maxBytes) return bytes;
    await cache.delete(url);
  } catch (_) { /* Private mode and full/disabled caches are normal. */ }
  return null;
}

async function rememberArtwork(url, bytes) {
  if (!globalThis.caches || bytes.length > ART_CACHE_ITEM_BYTES) return;
  try {
    const blob = new Blob([bytes]);
    const cache = await caches.open(ART_CACHE);
    await cache.put(url, new Response(blob, { headers: { 'X-Vey-Cached-At': String(Date.now()) } }));
    const keys = await cache.keys();
    await Promise.all(keys.slice(0, Math.max(0, keys.length - ART_CACHE_LIMIT)).map(key => cache.delete(key)));
  } catch (_) { /* Cache failures must never block artwork display. */ }
}

// Decode through the browser's asynchronous image pipeline and hand Slint a
// bounded RGBA buffer. This avoids running the full image decoder in WASM on
// the UI thread for every cover during Home startup.
export async function decodeArtwork(bytes, maxEdge = 768) {
  const bitmap = await createImageBitmap(new Blob([bytes]));
  try {
    const scale = Math.min(1, maxEdge / Math.max(bitmap.width, bitmap.height));
    const width = Math.max(1, Math.round(bitmap.width * scale));
    const height = Math.max(1, Math.round(bitmap.height * scale));
    const canvas = typeof OffscreenCanvas !== 'undefined'
      ? new OffscreenCanvas(width, height)
      : Object.assign(document.createElement('canvas'), { width, height });
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context) throw new Error('Artwork canvas is unavailable');
    context.drawImage(bitmap, 0, 0, width, height);
    const { data } = context.getImageData(0, 0, width, height);
    return { width, height, data };
  } finally {
    bitmap.close();
  }
}

// Bound network work even when a server omits Content-Length or stalls.
export async function fetchArtwork(url, maxBytes, timeoutMs = 15000) {
  const cached = await cachedArtwork(url, maxBytes);
  if (cached) return cached;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { signal: controller.signal });
    if (!response.ok) throw new Error(`Artwork HTTP ${response.status}`);
    const length = Number(response.headers.get('content-length'));
    if (Number.isFinite(length) && length > maxBytes) throw new Error('Artwork exceeds size limit');
    if (!response.body) {
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.length > maxBytes) throw new Error('Artwork exceeds size limit');
      void rememberArtwork(url, bytes);
      return bytes;
    }
    const reader = response.body.getReader();
    const chunks = [];
    let size = 0;
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        size += value.length;
        if (size > maxBytes) {
          controller.abort();
          throw new Error('Artwork exceeds size limit');
        }
        chunks.push(value);
      }
    } finally {
      reader.releaseLock();
    }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.length;
    }
    void rememberArtwork(url, bytes);
    return bytes;
  } finally {
    clearTimeout(timeout);
  }
}
