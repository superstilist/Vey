import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const source = await readFile(new URL('../../web/artwork-fetch.js', import.meta.url));
const { fetchArtwork } = await import(`data:text/javascript;base64,${source.toString('base64')}`);

globalThis.fetch = async () => new Response(new Uint8Array([1, 2, 3]));
assert.deepEqual([...await fetchArtwork('https://fixture.invalid/art', 4)], [1, 2, 3]);

globalThis.fetch = async () => new Response(new Uint8Array([1]), {
  headers: { 'content-length': '100' },
});
await assert.rejects(fetchArtwork('https://fixture.invalid/art', 4), /size limit/);

globalThis.fetch = async () => new Response(new ReadableStream({
  start(controller) {
    controller.enqueue(new Uint8Array([1, 2, 3]));
    controller.enqueue(new Uint8Array([4, 5]));
    controller.close();
  },
}));
await assert.rejects(fetchArtwork('https://fixture.invalid/art', 4), /size limit/);

globalThis.fetch = (_url, { signal }) => new Promise((_resolve, reject) => {
  signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true });
});
await assert.rejects(fetchArtwork('https://fixture.invalid/art', 4, 20), /aborted/);
console.log('Artwork fetch: bounded responses and timeout passed');
