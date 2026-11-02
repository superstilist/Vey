//! Monotone camera segments retain velocity when a cue interrupts a move.
//! Settled lines ease in and out; interrupted moves retain their velocity.
#[derive(Clone, Copy)]
struct Segment {
    from: f64,
    to: f64,
    velocity: f64,
    at: f64,
    duration: f64,
}
impl Segment {
    fn sample(self, now: f64) -> (f64, f64) {
        let t = ((now - self.at) / self.duration).clamp(0., 1.);
        if t >= 1. {
            return (self.to, 0.);
        }
        let d = self.to - self.from;
        let tangent = self.velocity * self.duration;
        let p =
            self.from + d * (3. * t * t - 2. * t * t * t) + tangent * (t * t * t - 2. * t * t + t);
        let v = (d * (6. * t - 6. * t * t) + tangent * (3. * t * t - 4. * t + 1.)) / self.duration;
        (p.clamp(self.from.min(self.to), self.from.max(self.to)), v)
    }
}
#[derive(Default)]
pub(crate) struct Camera {
    segment: Option<Segment>,
    position: f64,
    target: f64,
    duration: f64,
    initialized: bool,
}
impl Camera {
    pub fn reset(&mut self, position: f32) {
        self.position = position as f64;
        self.target = self.position;
        self.segment = None;
        self.initialized = true;
    }
    fn sample(&self, now: f64) -> (f64, f64) {
        self.segment.map_or((self.position, 0.), |s| s.sample(now))
    }
    pub fn advance(&mut self, target: f32, now: f64, duration: f64, snap: bool) -> f32 {
        let target = target.max(0.) as f64;
        if !self.initialized || snap || duration <= 0. {
            self.reset(target as f32);
            self.duration = duration;
            return target as f32;
        }
        let (position, velocity) = self.sample(now);
        if target != self.target || duration != self.duration {
            let distance = target - position;
            let curve_duration = if velocity * distance > 0. {
                duration.min(3. * distance.abs() / velocity.abs())
            } else {
                duration
            };
            // Zero end slope and a normalized start slope in [0,3] keep
            // Hermite interpolation monotone, including rapid direction changes.
            let retained = if distance.abs() < 0.0001 {
                0.
            } else {
                (velocity * curve_duration / distance).clamp(0., 3.)
            };
            self.segment = Some(Segment {
                from: position,
                to: target,
                velocity: retained * distance / curve_duration,
                at: now,
                duration: curve_duration,
            });
            self.target = target;
            self.duration = duration;
        }
        let (position, _) = self.sample(now);
        self.position = position;
        position as f32
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retarget_keeps_position_and_velocity_instead_of_restarting_at_rest() {
        let mut camera = Camera::default();
        camera.advance(0., 0., 750., true);
        camera.advance(100., 0., 750., false);
        let before = camera.sample(400.);
        camera.advance(200., 400., 750., false);
        let after = camera.sample(400.);
        assert!((before.0 - after.0).abs() < 1e-9);
        assert!((before.1 - after.1).abs() < 1e-9);
        assert!(after.1 > 0.);
        assert_eq!(camera.sample(1150.), (200., 0.));
    }
    #[test]
    fn close_forward_target_shortens_settling_without_braking_at_the_cue() {
        let mut camera = Camera::default();
        camera.advance(0., 0., 750., true);
        camera.advance(100., 0., 750., false);
        let before = camera.sample(400.);
        let target = before.0 as f32 + 2.;
        camera.advance(target, 400., 750., false);
        assert!((camera.sample(400.).1 - before.1).abs() < 1e-9);
        assert!(camera.segment.unwrap().duration < 750.);
        assert_eq!(camera.sample(1150.), (target as f64, 0.));
    }
    #[test]
    fn a_settled_line_accelerates_smoothly_without_a_front_loaded_jump() {
        let mut camera = Camera::default();
        camera.advance(0., 0., 750., true);
        camera.advance(200., 0., 750., false);
        assert_eq!(camera.advance(200., 0., 750., false), 0.);
        assert_eq!(camera.sample(0.).1, 0.);
        let first = camera.advance(200., 16., 750., false);
        assert!(first > 0. && first < 1.);
        let quarter = camera.advance(200., 187.5, 750., false);
        assert!(quarter > 25. && quarter < 40.);
        assert_eq!(camera.advance(200., 750., 750., false), 200.);
        let mut last = 0.;
        for now in (0..=750).step_by(10) {
            let y = camera.advance(200., now as f64, 750., false);
            assert!(y >= last && y <= 200.);
            last = y;
        }
    }
    #[test]
    fn dense_cues_never_reverse_overshoot_or_stop_mid_retarget() {
        let mut camera = Camera::default();
        camera.advance(0., 0., 750., true);
        let mut previous = 0.;
        for tick in 1..500 {
            let now = tick as f64 * 8.;
            let target = ((tick - 1) / 20 + 1) as f32 * 40.;
            let y = camera.advance(target, now, 750., false);
            assert!(y >= previous && y <= target);
            previous = y;
            if tick > 40 {
                assert!(camera.sample(now).1 > 0.);
            }
        }
        assert_eq!(camera.advance(1000., 5000., 750., false), 1000.);
    }
    #[test]
    fn seeks_reduced_motion_and_manual_rebase_are_exact() {
        let mut camera = Camera::default();
        assert_eq!(camera.advance(30., 0., 750., true), 30.);
        camera.advance(300., 10., 750., false);
        camera.reset(80.);
        assert_eq!(camera.advance(200., 500., 750., false), 80.);
        assert_eq!(camera.advance(20., 516., 750., true), 20.);
        assert_eq!(camera.advance(120., 532., 0., false), 120.);
        camera.advance(0., 540., 750., false);
        let mut last = 120.;
        for now in (540..1300).step_by(10) {
            let y = camera.advance(0., now as f64, 750., false);
            assert!(y <= last && y >= 0.);
            last = y;
        }
    }
}
