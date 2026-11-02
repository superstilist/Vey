import {gsap} from 'gsap';
import {CustomEase} from 'gsap/CustomEase';

gsap.registerPlugin(CustomEase);
gsap.config({autoSleep:12});
gsap.ticker.lagSmoothing(0);
const cameraEase=CustomEase.create('vey-camera','0.2,0,0,1');

// The host's clock and camera endpoints stay authoritative. GSAP presents that
// same path between bridge samples and never changes lyric cue/playback time.
export class CameraPresentation {
  animation;
  stageAnimation;
  key='';
  engine='gsap';
  constructor(element,stageElement,elementBlur=true){
    this.element=element;this.stageElement=stageElement;this.elementBlur=elementBlur;
  }
  update(camera,reduced,blurEnabled=true,applyTransform=true){
    if(!camera||reduced){this.cancel();return false;}
    const duration=Number.isFinite(camera.duration)?Math.min(520,Math.max(1,camera.duration)):160;
    const blurred=blurEnabled&&camera.blur!==false;
    const values=[camera.epoch,camera.fromX,camera.fromY,camera.toX,camera.toY,camera.width,camera.height,duration];
    if(!values.every(Number.isFinite)||camera.width<=0||camera.height<=0){this.cancel();return false;}
    const key=JSON.stringify([...values,blurred,applyTransform]);
    if(key===this.key)return true;
    this.cancel();this.key=key;this.element.style.transition='none';
    const element=this.element,stage=blurred?this.stageElement:null;
    const original={transform:element.style.transform,filter:element.style.filter,stageFilter:stage?.style.filter};
    const phase={value:0};
    const render=()=>{
      const p=phase.value;
      if(applyTransform){
        const x=camera.fromX+(camera.toX-camera.fromX)*p;
        const y=camera.fromY+(camera.toY-camera.fromY)*p;
        element.style.transform=`translate(${(1-x)*camera.width}px, ${(-1-y)*camera.height}px)`;
      }
      const peak=p<0.48?p/0.48:(1-p)/0.52;
      element.style.filter=`blur(${blurred&&this.elementBlur?Math.max(0,peak)*5.5:0}px)`;
      if(stage)stage.style.filter=`blur(${Math.max(0,peak)*3.5}px)`;
    };
    const timeline=gsap.timeline({paused:true}).to(phase,{value:1,duration:duration/1000,ease:cameraEase,onUpdate:render});
    let cancelled=false;
    const handle={
      timeline,
      pause(){timeline.pause();},
      cancel(){
        if(cancelled)return;cancelled=true;timeline.kill();
        element.style.transform=original.transform;element.style.filter=original.filter;
        if(stage)stage.style.filter=original.stageFilter;
      },
      get currentTime(){return timeline.time()*1000;},
      set currentTime(ms){timeline.time(Math.min(duration,Math.max(0,ms))/1000,false);},
    };
    this.animation=handle;this.stageAnimation=stage?handle:undefined;
    handle.currentTime=Number.isFinite(camera.elapsed)?camera.elapsed:0;render();timeline.play();
    return true;
  }
  cancel(){
    this.animation?.cancel();this.animation=undefined;this.stageAnimation=undefined;this.key='';
  }
}
