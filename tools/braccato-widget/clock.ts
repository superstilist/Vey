// Smooth ordinary host-clock jitter; seeks, pause and rate changes stay immediate.
export class PlaybackClock {
  position = 0;
  playing = false;
  rate = 1;
  at = 0;
  private correction = 0;
  private settle = 140;

  read(now: number): number {
    const elapsed = Math.max(0, now - this.at);
    return this.position + (this.playing ? Math.min(1000, elapsed) * this.rate / 1000 : 0)
      + this.correction * Math.max(0, 1 - elapsed / this.settle);
  }

  sample(position: number, playing: boolean, rate: number, now: number) {
    if (!Number.isFinite(position) || !Number.isFinite(rate) || rate <= 0) return;
    const difference = this.read(now) - position;
    this.correction = playing && this.playing && rate === this.rate && Math.abs(difference) < 0.25
      ? difference : 0;
    this.settle = Math.max(140, Math.abs(this.correction) * 1200 / rate);
    this.position = Math.max(0, position);
    this.playing = playing;
    this.rate = rate;
    this.at = now;
  }
}
