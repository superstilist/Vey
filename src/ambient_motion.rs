//! Bounded texture sampling shared by the native backdrop and its regression tests.
pub(crate) fn compose(
    target: &[u8],
    from: &[u8],
    shown: &mut Vec<u8>,
    size: (usize, usize),
    phase: f32,
    warp: f32,
    reduced: bool,
    mix: f32,
) {
    let (width, height) = size;
    shown.resize(target.len(), 0);
    // A normalized square crop covers every angle without transparent corners.
    let angle = if reduced {
        0.
    } else {
        phase * 0.12 * warp.min(1.)
    };
    let (sin, cos) = angle.sin_cos();
    let scale = if reduced || warp == 0. { 1. } else { 0.68 };
    for y in 0..height {
        let v = (y as f32 + 0.5) / height as f32 - 0.5;
        let shift = if reduced {
            0.
        } else {
            (y as f32 / 24. + phase).sin() * warp * 10.
        };
        for x in 0..width {
            let u = (x as f32 + 0.5) / width as f32 - 0.5;
            let sx = ((u * cos - v * sin) * scale + 0.5) * width as f32 - 0.5 + shift;
            let sy = ((u * sin + v * cos) * scale + 0.5) * height as f32 - 0.5;
            let sx = sx.clamp(0., width as f32 - 1.);
            let sy = sy.clamp(0., height as f32 - 1.);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
            let (fx, fy) = (sx.fract(), sy.fract());
            for c in 0..4 {
                let p = |x, y| target[(y * width + x) * 4 + c] as f32;
                let top = p(x0, y0) * (1. - fx) + p(x1, y0) * fx;
                let bottom = p(x0, y1) * (1. - fx) + p(x1, y1) * fx;
                let value = top * (1. - fy) + bottom * fy;
                let i = (y * width + x) * 4 + c;
                shown[i] = if from.len() == target.len() && mix < 1. {
                    (from[i] as f32 + (value - from[i] as f32) * mix).round() as u8
                } else {
                    value.round() as u8
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotating_texture_stays_opaque_and_frozen_motion_is_stable() {
        let source: Vec<u8> = (0..32 * 18)
            .flat_map(|i| [(i % 32 * 8) as u8, (i / 32 * 14) as u8, 80, 255])
            .collect();
        let render = |phase, reduced| {
            let mut result = vec![];
            compose(&source, &[], &mut result, (32, 18), phase, 1., reduced, 1.);
            result
        };
        let first = render(0., false);
        assert_ne!(first, render(8., false));
        assert_eq!(first, render(0., false));
        assert_eq!(render(0., true), render(8., true));
        for phase in [0., 8., 16., 32., 50.] {
            assert!(render(phase, false).chunks_exact(4).all(|p| p[3] == 255));
        }
        let mut mixed = vec![];
        compose(&source, &first, &mut mixed, (32, 18), 8., 1., false, 0.);
        assert_eq!(mixed, first, "artwork changes start at the displayed frame");
    }
}
