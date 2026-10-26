import {chromium} from 'playwright';
import assert from 'node:assert/strict';
const origin=process.env.VEY_API_URL||'http://127.0.0.1:4879';
const browser=await chromium.launch({channel:'msedge',headless:true});
const errors=[];
try{
 const page=await browser.newPage({viewport:{width:1440,height:1000}});page.on('pageerror',e=>errors.push(e.message));
 const songs=Array.from({length:6},(_,i)=>({id:'fixture-'+i,title:['Night Drive','Soft Focus','Tidal','Afterglow','Open Sky','Still Life'][i],artist:['Aster','Mira','North','Aster','Mira','North'][i],album:'Studio sessions',duration:210,has_line_sync:true}));
 await page.route('**/lyrics/search?**',route=>route.fulfill({json:{results:songs,total:6}}));
 await page.route('**/browse/artists/profile?**',route=>route.fulfill({json:{}}));
 await page.goto(origin);await page.locator('#sg .card--song').first().waitFor();
 const covers=await page.evaluate(()=>['#dc643c','#3a90d8'].map(color=>{const c=document.createElement('canvas');c.width=32;c.height=32;const ctx=c.getContext('2d');ctx.fillStyle=color;ctx.fillRect(0,0,32,32);return c.toDataURL();}));
 await page.evaluate(cover=>BM.player.gpShow({title:'Night Drive',artist:'Aster',cover_art:cover}),covers[0]);
 await page.waitForFunction(()=>{const h=Number(document.documentElement.style.getPropertyValue('--m3-dyn-hue'));return h>0&&h<35;});
 const warm=await page.evaluate(()=>getComputedStyle(document.querySelector('#gp-playbtn')).backgroundColor);
 await page.locator('#q').fill('Night Drive ! 2026');assert.equal(await page.locator('#q2').inputValue(),'Night Drive ! 2026');
 await page.screenshot({path:'target/site-material-desktop.png',fullPage:true});
 await page.evaluate(cover=>BM.player.gpShow({title:'Tidal',artist:'North',cover_art:cover}),covers[1]);
 await page.waitForFunction(()=>Number(document.documentElement.style.getPropertyValue('--m3-dyn-hue'))>190);
 assert.notEqual(await page.evaluate(()=>getComputedStyle(document.querySelector('#gp-playbtn')).backgroundColor),warm);
 for(const width of [390,320]){
   await page.setViewportSize({width,height:844});await page.locator('#q').fill('mobile + search');assert.equal(await page.locator('#q2').inputValue(),'mobile + search');
   const metrics=await page.evaluate(()=>{const a=document.querySelector('.gp-meta').getBoundingClientRect(),b=document.querySelector('.gp-controls').getBoundingClientRect(),p=document.querySelector('#gplayer').getBoundingClientRect(),button=document.querySelector('#gp-playbtn').getBoundingClientRect();return {overflow:document.documentElement.scrollWidth>innerWidth,overlap:a.right>b.left+1,bottom:p.bottom,height:innerHeight,play:button.width};});
   assert.equal(metrics.overflow,false);assert.equal(metrics.overlap,false);assert.ok(metrics.bottom<=metrics.height);assert.ok(metrics.play>=44);
   await page.screenshot({path:'target/site-material-mobile-'+width+'.png',fullPage:true});
 }
 await page.emulateMedia({reducedMotion:'reduce'});assert.ok(await page.evaluate(()=>parseFloat(getComputedStyle(document.querySelector('.card--song')).transitionDuration)<0.01));
 await page.goto(origin+'/account');await page.locator('#vq-start').click();await page.locator('#vq-image').waitFor({state:'visible'});await page.screenshot({path:'target/site-material-qr.png',fullPage:true});await page.locator('#vq-cancel').click();assert.equal(await page.locator('#al-em').isVisible(),true);
 assert.deepEqual(errors,[]);console.log('PASS: desktop/390/320 layouts, actual artwork palette changes, editable synchronized search after player update, unobstructed player controls, 44px play target, reduced motion, QR login/cancellation.');
}finally{await browser.close();}
