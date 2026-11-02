import { chromium } from 'playwright';
import assert from 'node:assert/strict';

const browser = await chromium.launch({ channel: 'msedge', headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const requested = [];
const waiting = [];
const errors = [];
const cover = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=', 'base64');
const tracks = Array.from({ length: 2 }, (_, i) => ({
  path: `yt://seed${i}`, videoId: `seed${i}`, title: `Seed ${i}`,
  artist: 'Fixture', durationSecs: 30, thumbnailUrl: `http://127.0.0.1:4181/cover/current${i}`,
}));
const recommendations = Array.from({ length: 8 }, (_, i) => ({
  path: `yt://recommendation${i}`, videoId: `recommendation${i}`, title: `Recommendation ${i}`,
  artist: `Artist ${i}`, durationSecs: 30, thumbnailUrl: `http://127.0.0.1:4181/cover/recommendation${i}`,
}));
const wav = Buffer.alloc(44 + 32000 * 30);
wav.write('RIFF'); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8);
wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
wav.writeUInt32LE(16000, 24); wav.writeUInt32LE(32000, 28); wav.writeUInt16LE(2, 32);
wav.writeUInt16LE(16, 34); wav.write('data', 36); wav.writeUInt32LE(wav.length - 44, 40);

page.on('pageerror', error => { if (!error.message.includes('Using exceptions for control flow')) errors.push(error.message); });
await page.addInitScript(tracks => {
  localStorage.setItem('playervf.rust.v1', JSON.stringify({
    version: 1, tracks, history: tracks.map(track => ({ track, playedAt: 1, playCount: 1 })),
    settings: { language: 'en', volume: .5, lyrics_translation: false, lyrics_romanization: false },
  }));
  const AudioElement = window.Audio;
  window.Audio = class extends AudioElement { constructor(...args) { super(...args); window.fixtureAudio = this; } };
}, tracks);
await page.route('**/api/browse?*', route => route.fulfill({ json: recommendations }));
await page.route('**/api/audio/*', route => route.fulfill({ contentType: 'audio/wav', body: wav }));
await page.route('**/api/lyrics*', route => route.fulfill({ json: [] }));
await page.route('**/cover/**', async route => {
  requested.push(new URL(route.request().url()).pathname);
  await new Promise(resolve => waiting.push({ route, resolve }));
  await route.fulfill({ status: 200, contentType: 'image/png', body: cover }).catch(() => {});
});
async function until(condition, label) {
  const deadline = Date.now() + 15000;
  while (!condition()) {
    if (Date.now() > deadline) throw new Error(`Timed out: ${label}; requests=${requested.join(',')}`);
    await new Promise(resolve => setTimeout(resolve, 50));
  }
}
try {
  await page.goto('http://127.0.0.1:4181/');
  await page.locator('iframe').waitFor({ state: 'attached' });
  await until(() => requested.length >= 2, 'two background covers');
  assert.equal(requested.length, 2, 'background requests are bounded');
  await page.mouse.click(99, 855);
  await page.waitForFunction(() => window.fixtureAudio && !window.fixtureAudio.paused);
  await until(() => requested.includes('/cover/current0'), 'selected cover uses reserved slot');
  assert.equal(requested.length, 3, 'selected cover has a single reserved slot');
  await page.mouse.click(150, 855);
  await page.waitForFunction(() => window.fixtureAudio.src.endsWith('/seed1'));
  assert.ok(!requested.includes('/cover/current1'), 'next cover waits while all slots are busy');
  waiting[0].resolve();
  await until(() => requested.includes('/cover/current1'), 'new selected cover starts first');
  assert.equal(requested[3], '/cover/current1');
  assert.deepEqual(errors, []);
  console.log('Artwork queue: bounded requests and current-track priority passed');
} finally {
  for (const item of waiting) item.resolve();
  await browser.close();
}
