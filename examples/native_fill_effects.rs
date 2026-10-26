//! Reproduce stale halos and mismatched fill when letter motion returns to a shaped run.
use playervf::{LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(v)).into()
}
fn capture(w: &MainWindow, label: &str) -> image::RgbaImage {
    let f = w.window().take_snapshot().unwrap();
    let image = image::RgbaImage::from_raw(f.width(), f.height(), f.as_bytes().to_vec()).unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    image
        .save(format!(
            "target/native-lyrics-check/fill-effects-{label}.png"
        ))
        .unwrap();
    image
}
fn differences(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    (250..650)
        .flat_map(|y| (720..1400).map(move |x| (x, y)))
        .filter(|&(x, y)| a.get_pixel(x, y) != b.get_pixel(x, y))
        .count()
}
fn identical_radius_tiles(w: &MainWindow) -> bool {
    let rows = w.get_native_lyric_lines();
    let row = rows.row_data(0).unwrap();
    let mut word = row.parts.row_data(0).unwrap();
    if word.glow_levels < 2 {
        return false;
    }
    let source = word.glow_wide.to_rgba8().unwrap();
    let width = source.width();
    let height = source.height();
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
    pixels.make_mut_bytes().copy_from_slice(source.as_bytes());
    let tile = width as usize / word.glow_levels as usize * 4;
    let bytes = pixels.make_mut_bytes();
    for y in 0..height as usize {
        let row = y * width as usize * 4;
        for n in 1..word.glow_levels as usize {
            bytes.copy_within(row..row + tile, row + n * tile);
        }
    }
    word.glow_wide = slint::Image::from_rgba8(pixels);
    row.parts.set_row_data(0, word);
    true
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1)
    }));
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_duration(12.);
    w.set_position(1.3);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let mut s = w.get_lyric_style();
    s.glow = false;
    s.fade = false;
    s.wave = false;
    s.wobble = false;
    s.inactive_blur = 0.;
    s.active_scale = 1.;
    s.inactive_scale = 1.;
    w.set_lyric_style(s);
    w.set_lyric_lines(model(vec![LyricRow {
        text: "Feel good".into(),
        start_ms: 0,
        end_ms: 6000,
        words: model(vec![
            WordRow {
                text: "Feel good".into(),
                ..Default::default()
            },
            WordRow {
                text: "Change, ".into(),
                start_ms: 1000,
                end_ms: 1568,
                background: true,
                ..Default::default()
            },
            WordRow {
                text: "change, change, change".into(),
                start_ms: 3000,
                end_ms: 5000,
                background: true,
                ..Default::default()
            },
        ]),
        ..Default::default()
    }]));
    let step = Rc::new(Cell::new(0));
    let done = step.clone();
    let frames = Rc::new(RefCell::new(Vec::new()));
    let issues = Rc::new(RefCell::new(Vec::new()));
    let retries = Rc::new(Cell::new(0));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    let on_tick = move || {
        let w = weak.upgrade().unwrap();
        let i = step.get();
        if i == 0 {
            frames.borrow_mut().push(capture(&w, "glow-off-active"));
            let mut s = w.get_lyric_style();
            s.glow = true;
            w.set_lyric_style(s);
        } else if i == 1 {
            let active = capture(&w, "glow-on-active");
            let changed = differences(&frames.borrow()[0], &active);
            assert!(changed > 50, "active syllable needs real glow: {changed}");
            println!("active glow: {changed} pixels");
            let f = frames.borrow();
            let off = &f[0];
            let mut top = 520;
            let mut left = 1400;
            for y in 420..520 {
                for x in 750..1400 {
                    if off.get_pixel(x, y)[0] > 40 {
                        top = top.min(y);
                        left = left.min(x);
                    }
                }
            }
            assert!(top < 520 && left < 1400);
            let mut boxes = 0;
            // Far above the first backing glyph: a Gaussian halo is negligible
            // here, while a mistakenly colorized transparent image quad is bright.
            for y in top - 17..top - 13 {
                for x in left + 2..left + 18 {
                    if active.get_pixel(x, y)[0] > off.get_pixel(x, y)[0].saturating_add(5) {
                        boxes += 1;
                    }
                }
            }
            println!("transparent glow-margin violations: {boxes}");
            if boxes > 0 {
                issues
                    .borrow_mut()
                    .push("glow paints a rectangular transparent margin");
            }
            w.set_position(1.9);
        } else if i == 2 {
            frames.borrow_mut().push(capture(&w, "glow-on-expired"));
            let mut s = w.get_lyric_style();
            s.glow = false;
            w.set_lyric_style(s);
        } else if i == 3 {
            let expired = capture(&w, "glow-off-expired");
            let residual = differences(&frames.borrow()[1], &expired);
            println!("expired cue glow residual: {residual} pixels");
            if residual != 0 {
                issues.borrow_mut().push("glow survives the actual cue");
            }
            let mut s = w.get_lyric_style();
            s.wave = true;
            w.set_lyric_style(s);
            w.set_position(4.);
            w.set_lyric_lines(model(vec![LyricRow {
                text: "llllllll".into(),
                start_ms: 0,
                end_ms: 11000,
                words: model(vec![WordRow {
                    text: "llllllll".into(),
                    start_ms: 1000,
                    end_ms: 7000,
                    ..Default::default()
                }]),
                ..Default::default()
            }]));
        } else if i == 4 {
            let word = w
                .get_native_lyric_lines()
                .row_data(0)
                .unwrap()
                .parts
                .row_data(0)
                .unwrap();
            let count = word.glyphs.row_count();
            if count == 0 {
                retries.set(retries.get() + 1);
                assert!(retries.get() < 12, "glyph plan timed out");
                return;
            }
            let last = word.glyphs.row_data(count - 1).unwrap();
            let window = (last.delay_ms + last.wave_ms).min(word.duration_ms);
            assert!(window > 1000.);
            w.set_position((word.start_ms + window - 1.) / 1000.);
            println!("letter return boundary: {window}ms");
        } else if i == 5 {
            frames
                .borrow_mut()
                .push(capture(&w, "letter-fill-before-return"));
            let mut s = w.get_lyric_style();
            s.wave = false;
            w.set_lyric_style(s);
        } else if i == 6 {
            let plain = capture(&w, "shaped-fill-before-return");
            let f = frames.borrow();
            let wave = &f[2];
            let mut left = 1400;
            let mut right = 720;
            for y in 250..650 {
                for x in 720..1400 {
                    if plain.get_pixel(x, y)[0] > 210 {
                        left = left.min(x);
                        right = right.max(x);
                    }
                }
            }
            let mut deficit = 0usize;
            let mut cores = 0usize;
            for y in 250..650 {
                for x in 720..1400 {
                    let a = plain.get_pixel(x, y)[0];
                    let b = wave.get_pixel(x, y)[0];
                    if a > 210 && (x - left) as f32 > 0.85 * (right - left) as f32 {
                        cores += 1;
                        deficit += a.saturating_sub(b) as usize;
                    }
                }
            }
            assert!(cores > 80, "need real final-glyph cores");
            let mean = deficit as f32 / cores as f32;
            println!(
                "letter/shaped fill mismatch before return: mean deficit={mean:.2}, cores={cores}"
            );
            if mean > 8. {
                issues
                    .borrow_mut()
                    .push("letter fill and whole-run fill disagree");
            }
            drop(f);
            frames.borrow_mut().clear();
            w.set_position(2.5);
            w.set_lyric_lines(model(vec![LyricRow {
                text: "WiiiWiii".into(),
                start_ms: 0,
                end_ms: 11000,
                words: model(vec![WordRow {
                    text: "WiiiWiii".into(),
                    start_ms: 1000,
                    end_ms: 7000,
                    ..Default::default()
                }]),
                ..Default::default()
            }]));
        } else if i == 7 {
            frames
                .borrow_mut()
                .push(capture(&w, "wide-narrow-run-quarter"));
            w.set_position(7.2);
        } else if i == 8 {
            frames
                .borrow_mut()
                .push(capture(&w, "wide-narrow-run-full"));
            w.set_position(2.5);
            let mut s = w.get_lyric_style();
            s.wave = true;
            w.set_lyric_style(s);
        } else if i == 9 {
            let rows = w.get_native_lyric_lines();
            let row = rows.row_data(0).unwrap();
            let mut word = row.parts.row_data(0).unwrap();
            if word.glyphs.row_count() == 0 {
                retries.set(retries.get() + 1);
                assert!(retries.get() < 15);
                return;
            }
            // Isolate mask timing from physical letter motion. A transparent
            // sentinel keeps the glyph path mounted; actual glyphs stay still.
            let mut glyphs = (0..word.glyphs.row_count())
                .filter_map(|n| word.glyphs.row_data(n))
                .collect::<Vec<_>>();
            for glyph in &mut glyphs {
                glyph.delay_ms = 0.;
                glyph.wave_ms = 1.;
            }
            glyphs.push(playervf::BraccatoGlyph {
                text: "".into(),
                start: 1.,
                delay_ms: 6000.,
                wave_ms: 1.,
                ..Default::default()
            });
            word.glyphs = model(glyphs);
            row.parts.set_row_data(0, word);
        } else if i == 10 {
            let letters = capture(&w, "wide-narrow-letters-quarter");
            let f = frames.borrow();
            let mut total = 0usize;
            let mut cores = 0usize;
            for y in 251..649 {
                for x in 721..1399 {
                    if (y - 1..=y + 1)
                        .all(|py| (x - 1..=x + 1).all(|px| f[1].get_pixel(px, py)[0] > 215))
                    {
                        cores += 1;
                        total +=
                            letters.get_pixel(x, y)[0].abs_diff(f[0].get_pixel(x, y)[0]) as usize;
                    }
                }
            }
            assert!(cores > 100);
            let error = total as f32 / cores as f32;
            println!("wide/narrow glyph fill conformance: mean error={error:.2}, cores={cores}");
            if error > 8. {
                issues
                    .borrow_mut()
                    .push("glyph-width-dependent fill mismatch");
            }
            drop(f);
            frames.borrow_mut().clear();
            w.set_position(1.3);
            let mut s = w.get_lyric_style();
            s.wave = false;
            s.glow = true;
            s.glow_duration = 5000.;
            w.set_lyric_style(s);
            w.set_lyric_lines(model(vec![LyricRow {
                text: "Change,".into(),
                start_ms: 0,
                end_ms: 6000,
                words: model(vec![WordRow {
                    text: "Change,".into(),
                    start_ms: 1000,
                    end_ms: 1568,
                    background: true,
                    ..Default::default()
                }]),
                ..Default::default()
            }]));
        } else if i == 11 || i == 13 {
            if !identical_radius_tiles(&w) {
                retries.set(retries.get() + 1);
                assert!(retries.get() < 18);
                return;
            }
        } else if i == 12 {
            frames.borrow_mut().push(capture(&w, "atlas-wide-weight"));
            let mut s = w.get_lyric_style();
            s.glow_duration = 1200.;
            w.set_lyric_style(s);
        } else if i == 14 {
            let other = capture(&w, "atlas-tight-weight");
            let f = frames.borrow();
            let first = &f[0];
            let mut maximum = 0u8;
            let mut total = 0usize;
            let mut samples = 0usize;
            for y in 250..650 {
                for x in 720..1400 {
                    let a = first.get_pixel(x, y)[0];
                    let b = other.get_pixel(x, y)[0];
                    if a.max(b) > 30 && a.max(b) < 200 {
                        let d = a.abs_diff(b);
                        maximum = maximum.max(d);
                        total += d as usize;
                        samples += 1;
                    }
                }
            }
            assert!(samples > 100);
            let mean = total as f32 / samples as f32;
            println!("identical-radius atlas feather: max difference={maximum}, mean={mean:.2}");
            if maximum > 5 || mean > 2. {
                issues
                    .borrow_mut()
                    .push("fill depends on atlas radius tile");
            }
            println!("fill effect issues: {:?}", issues.borrow());
            assert!(issues.borrow().is_empty(), "fill effects regressed");
            step.set(15);
            slint::quit_event_loop().unwrap();
            return;
        }
        step.set(i + 1);
    };
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(950),
        on_tick,
    );
    w.run()?;
    assert_eq!(done.get(), 15);
    Ok(())
}
