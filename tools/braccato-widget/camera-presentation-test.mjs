import {chromium} from 'playwright';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

const source = await readFile(new URL('../../web/player-motion.js', import.meta.url), 'utf8');
const browser = await chromium.launch({channel:'msedge',headless:true});
try {
  const page = await browser.newPage();
  await page.setContent('<div id="stage"><div id="lyrics">Timed lyrics</div></div>');
  const result = await page.evaluate(async source => {
    const {CameraPresentation} = await import(URL.createObjectURL(new Blob([source], {type:'text/javascript'})));
    const element = document.querySelector('#lyrics');
    const stage = document.querySelector('#stage');
    const motion = new CameraPresentation(element, stage);
    const camera = {epoch:1,fromX:0,fromY:0,toX:1,toY:-1,width:800,height:600,duration:380,elapsed:0};
    const started = motion.update(camera, false, true, false);
    motion.animation.pause(); motion.stageAnimation.pause();
    motion.animation.currentTime = 182.4; motion.stageAnimation.currentTime = 182.4;
    const peak = {filter:getComputedStyle(element).filter, stageFilter:getComputedStyle(stage).filter};
    const original = motion.animation;
    motion.update({...camera,elapsed:200}, false, true, false);
    const retained = motion.animation === original;
    motion.update({...camera,epoch:2,fromX:0.4,fromY:-0.4,toX:-1,toY:1}, false, false, true);
    motion.animation.pause(); motion.animation.currentTime = 190;
    const disabled = {filter:getComputedStyle(element).filter,stage:!!motion.stageAnimation};
    motion.update(camera, true);
    const reduced = element.getAnimations().length === 0 && stage.getAnimations().length === 0;
    const invalid = motion.update({...camera,width:NaN}, false);
    return {started, peak, retained, disabled, reduced, invalid};
  }, source);
  assert.equal(result.started,true,'boolean flags must not reject valid camera coordinates');
  assert.match(result.peak.filter,/blur\([1-9]/,'lyric content blurs during travel');
  assert.match(result.peak.stageFilter,/blur\([1-9]/);
  assert.equal(result.retained,true,'duplicate bridge samples keep the running animation');
  assert.equal(result.disabled.filter,'blur(0px)');
  assert.equal(result.disabled.stage,false);
  assert.equal(result.reduced,true,'reduced motion cancels both layers');
  assert.equal(result.invalid,false,'invalid geometry is rejected');
  console.log('PASS camera presentation: blur, boolean flags, interruption, duplicate samples, reduced motion');
} finally { await browser.close(); }
