// The host supplies the same world-space camera path used by Slint. Let the
// browser compositor run that path between bridge samples, including on pause.
export class CameraPresentation {
  animation;
  stageAnimation;
  key = '';

  constructor(element, stageElement, elementBlur = true) {
    this.element = element;
    this.stageElement = stageElement;
    this.elementBlur = elementBlur;
  }

  update(camera, reduced, blurEnabled = true, applyTransform = true) {
    if (!camera || reduced) {
      this.cancel();
      return false;
    }
    const duration = Number.isFinite(camera.duration) ? Math.min(520,Math.max(1,camera.duration)) : 380;
    const blurred = blurEnabled && camera.blur !== false;
    const values = [camera.epoch,camera.fromX,camera.fromY,camera.toX,camera.toY,camera.width,camera.height,duration];
    if (!values.every(Number.isFinite) || camera.width <= 0 || camera.height <= 0) {
      this.cancel();
      return false;
    }
    const key = JSON.stringify([...values,blurred,applyTransform]);
    if (key !== this.key) {
      this.cancel();
      this.key = key;
      this.element.style.transition = 'none';
      const transform = (x,y) =>
        `translate(${(1-x)*camera.width}px, ${(-1-y)*camera.height}px)`;
      const frame = (x,y,peak=false) => ({
        ...(applyTransform ? {transform:transform(x,y)} : {}),
        filter:peak && blurred && this.elementBlur ? 'blur(5.5px)' : 'blur(0px)',
      });
      const frames = [
        frame(camera.fromX,camera.fromY),
        {offset:0.48,...frame(
          (camera.fromX+camera.toX)/2,
          (camera.fromY+camera.toY)/2,
          true,
        )},
        frame(camera.toX,camera.toY),
      ];
      const options = {duration,easing:'cubic-bezier(0.45, 0, 0.55, 1)',fill:'both'};
      this.animation = this.element.animate(frames,options);
      if (blurred && this.stageElement) this.stageAnimation = this.stageElement.animate([
        {filter:'blur(0px)'},
        {offset:0.48,filter:'blur(3.5px)'},
        {filter:'blur(0px)'},
      ],options);
      // Join an already-started host animation without replaying its beginning.
      const elapsed = Math.min(duration,Math.max(0,camera.elapsed || 0));
      this.animation.currentTime = elapsed;
      if (this.stageAnimation) this.stageAnimation.currentTime = elapsed;
    }
    return true;
  }

  cancel() {
    this.animation?.cancel();
    this.stageAnimation?.cancel();
    this.animation = undefined;
    this.stageAnimation = undefined;
    this.key = '';
  }
}
