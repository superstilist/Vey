// The host supplies the same world-space camera path used by Slint. Let the
// browser compositor run that path between bridge samples, including on pause.
export class CameraPresentation {
  animation;
  key = '';

  constructor(element) { this.element = element; }

  update(camera, reduced) {
    if (!camera || reduced) {
      this.cancel();
      return false;
    }
    const duration = Number.isFinite(camera.duration) ? Math.min(520,Math.max(1,camera.duration)) : 380;
    const values = [camera.epoch,camera.fromX,camera.fromY,camera.toX,camera.toY,camera.width,camera.height,duration];
    if (!values.every(Number.isFinite) || camera.width <= 0 || camera.height <= 0) {
      this.cancel();
      return false;
    }
    const key = JSON.stringify(values);
    if (key !== this.key) {
      this.cancel();
      this.key = key;
      this.element.style.transition = 'none';
      const transform = (x,y) =>
        `translate(${(1-x)*camera.width}px, ${(-1-y)*camera.height}px)`;
      this.animation = this.element.animate([
        {transform:transform(camera.fromX,camera.fromY)},
        {transform:transform(camera.toX,camera.toY)},
      ],{duration,easing:'cubic-bezier(0.45, 0, 0.55, 1)',fill:'both'});
      // Join an already-started host animation without replaying its beginning.
      this.animation.currentTime = Math.min(duration,Math.max(0,camera.elapsed || 0));
    }
    return true;
  }

  cancel() {
    this.animation?.cancel();
    this.animation = undefined;
    this.key = '';
  }
}
