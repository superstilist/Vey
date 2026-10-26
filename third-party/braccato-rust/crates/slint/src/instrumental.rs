//! Shared native note masks. Playback selects atlas cells; no SVG parsing or rasterization per frame.
use crate::InstrumentalFrames;
use resvg::{tiny_skia, usvg};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::cell::OnceCell;

const TILE: usize = 72;
const COLUMNS: usize = 32;
const LEVELS: usize = 65;
const PHASES: usize = 8;
const WIDTH: usize = TILE * COLUMNS;
const HEIGHT: usize = TILE * (LEVELS * PHASES).div_ceil(COLUMNS);

thread_local! {
    static FRAMES: OnceCell<InstrumentalFrames> = const { OnceCell::new() };
}

/// Obtain shared alpha atlases for the original Braccato note silhouette, rising water and bloom.
/// Initialize once on the UI thread, then pass the returned model to `LyricsView.instrumental-frames`.
pub fn instrumental_frames() -> InstrumentalFrames {
    FRAMES.with(|frames| frames.get_or_init(build_frames).clone())
}

fn image(alpha: &[u8], width: usize, height: usize) -> Image {
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width as u32, height as u32);
    for (pixel, &a) in buffer.make_mut_slice().iter_mut().zip(alpha) {
        *pixel = Rgba8Pixel::new(255, 255, 255, a);
    }
    Image::from_rgba8(buffer)
}

fn build_frames() -> InstrumentalFrames {
    let tree = usvg::Tree::from_str(
        include_str!("../ui/instrumental-note.svg"),
        &usvg::Options::default(),
    )
    .expect("embedded upstream instrumental silhouette");
    let mut note = tiny_skia::Pixmap::new(TILE as u32, TILE as u32).unwrap();
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut note.as_mut());
    let silhouette: Vec<u8> = note.pixels().iter().map(|pixel| pixel.alpha()).collect();
    let mut alpha = vec![0u8; WIDTH * HEIGHT];
    for level in 0..LEVELS {
        let progress = level as f32 / (LEVELS - 1) as f32;
        for phase in 0..PHASES {
            let frame = level * PHASES + phase;
            let origin_x = (frame % COLUMNS) * TILE;
            let origin_y = (frame / COLUMNS) * TILE;
            for y in 0..TILE {
                for x in 0..TILE {
                    // Upstream wave travels from translateY(78%) to -4%, then flattens.
                    // The padded tile maps its 24-unit viewBox to a 48px content square.
                    let local_x = (x as f32 - 12.0) / 2.0;
                    let local_y = (y as f32 - 12.0) / 2.0;
                    let wave = (local_x * std::f32::consts::TAU / 18.0
                        + phase as f32 * std::f32::consts::TAU / PHASES as f32)
                        .sin()
                        * 0.6
                        * (1.0 - progress * progress);
                    let surface = 3.0 + 24.0 * (0.78 - 0.82 * progress) + wave;
                    let coverage = if level == 0 {
                        0.0
                    } else if level == LEVELS - 1 {
                        1.0
                    } else {
                        ((local_y - surface) * 2.0 + 0.5).clamp(0.0, 1.0)
                    };
                    alpha[(origin_y + y) * WIDTH + origin_x + x] =
                        (silhouette[y * TILE + x] as f32 * coverage).round() as u8;
                }
            }
        }
    }
    let mut halo = alpha.clone();
    let mut scratch = vec![0u8; halo.len()];
    // Three separable box passes approximate a Gaussian without per-frame filter work.
    // Each note has enough empty padding to prevent bleeding into adjacent atlas cells.
    for _ in 0..3 {
        box_blur(&halo, &mut scratch, WIDTH, HEIGHT, true);
        box_blur(&scratch, &mut halo, WIDTH, HEIGHT, false);
    }
    InstrumentalFrames {
        silhouette: image(&silhouette, TILE, TILE),
        fill: image(&alpha, WIDTH, HEIGHT),
        glow: image(&halo, WIDTH, HEIGHT),
        tile_size: TILE as i32,
        columns: COLUMNS as i32,
        levels: LEVELS as i32,
        phases: PHASES as i32,
    }
}

fn box_blur(source: &[u8], target: &mut [u8], width: usize, height: usize, horizontal: bool) {
    let (lines, length, stride) = if horizontal {
        (height, width, 1)
    } else {
        (width, height, width)
    };
    const RADIUS: usize = 4;
    const DIAMETER: u32 = (RADIUS * 2 + 1) as u32;
    for line in 0..lines {
        let origin = if horizontal { line * width } else { line };
        let mut sum = (0..=RADIUS)
            .map(|position| u32::from(source[origin + position * stride]))
            .sum::<u32>();
        for position in 0..length {
            target[origin + position * stride] = (sum / DIAMETER) as u8;
            if position >= RADIUS {
                sum -= u32::from(source[origin + (position - RADIUS) * stride]);
            }
            if position + RADIUS + 1 < length {
                sum += u32::from(source[origin + (position + RADIUS + 1) * stride]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_wave_stays_inside_original_note_and_reuses_atlases() {
        let frames = instrumental_frames();
        assert_eq!(frames.fill, instrumental_frames().fill);
        let fill = frames.fill.to_rgba8().unwrap();
        let shape = frames.silhouette.to_rgba8().unwrap();
        let mut prior = 0u64;
        for level in 0..LEVELS {
            let frame = level * PHASES;
            let mut area = 0u64;
            for y in 0..TILE {
                for x in 0..TILE {
                    let a = fill.as_slice()
                        [((frame / COLUMNS) * TILE + y) * WIDTH + (frame % COLUMNS) * TILE + x]
                        .a;
                    assert!(a <= shape.as_slice()[y * TILE + x].a);
                    area += u64::from(a);
                }
            }
            assert!(area >= prior, "the note fill must rise monotonically");
            prior = area;
        }
        assert!(prior > 0);
        assert!(
            frames
                .glow
                .to_rgba8()
                .unwrap()
                .as_slice()
                .iter()
                .any(|p| p.a > 0)
        );
    }
}
