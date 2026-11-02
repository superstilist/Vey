//! A fill must cross glyph interiors spatially instead of fading whole letters.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

const CASES: [&str; 4] = ["H", "HHHHH", "ШШШШШ", "(куди йти…)"];

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(items)).into()
}
fn fixture(w: &MainWindow, text: &str, motion: bool) {
    let mut words = vec![];
    if motion {
        words.push(WordRow {
            text: "Hi ".into(),
            // A finished finite cue is neutral in every reference frame. An
            // untimed prefix still wobbles at 500ms and pollutes the fill mask.
            start_ms: 0,
            end_ms: 200,
            ..Default::default()
        });
    }
    words.push(WordRow {
        text: text.into(),
        start_ms: 1000,
        end_ms: if motion { 1400 } else { 5000 },
        ..Default::default()
    });
    w.set_lyric_lines(model(vec![LyricRow {
        text: text.into(),
        start_ms: 0,
        end_ms: 10000,
        words: model(words),
        ..Default::default()
    }]));
    w.set_position(0.5);
}
fn capture(w: &MainWindow, case: usize, phase: usize) -> image::RgbaImage {
    let f = w.window().take_snapshot().unwrap();
    let f = image::RgbaImage::from_raw(f.width(), f.height(), f.as_bytes().to_vec()).unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    f.save(format!(
        "target/native-lyrics-check/spatial-{}-{}-{case}-{phase}.png",
        if w.get_native_text_gradients() {
            "gpu"
        } else {
            "software"
        },
        if w.get_lyric_style().wobble {
            "motion"
        } else {
            "static"
        }
    ))
    .unwrap();
    f
}
fn verify(
    w: &MainWindow,
    off: &image::RgbaImage,
    full: &image::RgbaImage,
    middle: &image::RgbaImage,
    fraction: f32,
) {
    let cores = (250..650)
        .flat_map(|y| (730..1400).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            (y - 1..=y + 1).all(|yy| {
                (x - 1..=x + 1).all(|xx| {
                    full.get_pixel(xx, yy)[0] > 215
                        && full.get_pixel(xx, yy)[0] > off.get_pixel(xx, yy)[0].saturating_add(70)
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(cores.len() > 80, "need actual glyph interiors");
    let left = cores.iter().map(|p| p.0).min().unwrap() as f32;
    let row = w.get_native_lyric_lines().row_data(0).unwrap();
    let word = row.parts.row_data(row.parts.row_count() - 1).unwrap();
    let bytes: &[u8] = if word.font_family == "Satoshi" {
        include_bytes!("../assets/fonts/Satoshi-700.otf")
    } else {
        include_bytes!("../assets/fonts/NotoSans.ttf")
    };
    let face = ttf_parser::Face::parse(bytes, 0).unwrap();
    let glyph = face.glyph_index(word.text.chars().next().unwrap()).unwrap();
    let bearing =
        face.glyph_bounding_box(glyph).unwrap().x_min as f32 * 72. / face.units_per_em() as f32;
    let origin = left - bearing - 1.;
    // Use font advance and bearing, not the visible ink bounds: wide side
    // bearings would otherwise make a valid feather look prematurely lit.
    let advance = playervf::native_lyric_fonts::measure_with(
        word.text.as_str(),
        72.,
        word.font_family.as_str(),
        700,
    );
    let mut sung = 0;
    let mut future = 0;
    let mut bad_sung = 0;
    let mut bad_future = 0;
    for (x, y) in cores {
        let part = (x as f32 - origin) / advance;
        let a = off.get_pixel(x, y)[0] as f32;
        let b = full.get_pixel(x, y)[0] as f32;
        let actual = (middle.get_pixel(x, y)[0] as f32 - a) / (b - a).max(1.);
        if part < fraction - 0.18 {
            sung += 1;
            if actual < 0.85 {
                bad_sung += 1;
            }
        }
        if part > fraction + 0.18 {
            future += 1;
            if actual > 0.15 {
                bad_future += 1;
            }
        }
    }
    if fraction == 0.5 {
        assert!(sung > 10 && future > 10, "need both sides of the fill edge");
    }
    println!("spatial fill {fraction:.2}: sung={sung}, future={future}, wrong sung={bad_sung}, early={bad_future}");
    assert_eq!(
        bad_future, 0,
        "future side must stay dim, including inside one letter"
    );
    if !w.get_lyric_style().wobble || w.get_lyrics_reduced_motion() {
        assert_eq!(
            bad_sung, 0,
            "sung side must be bright, not whole-glyph alpha"
        );
    } else if fraction == 0.5 {
        assert!(
            sung - bad_sung > sung / 2,
            "moving word still needs real sung ink"
        );
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1);
    }));
    let w = MainWindow::new()?;
    let motion = std::env::args().any(|s| s == "--motion");
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_duration(12.);
    w.set_lyric_size(72.);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let mut style = w.get_lyric_style();
    style.glow = false;
    style.fade = false;
    style.wave = false;
    style.wobble = motion && !std::env::args().any(|s| s == "--still");
    style.swipe = true;
    style.inactive_blur = 0.;
    style.active_scale = 1.;
    style.inactive_scale = 1.;
    w.set_lyric_style(style);
    fixture(&w, CASES[0], motion);
    let tick = Rc::new(Cell::new(0));
    let done = tick.clone();
    let frames = Rc::new(RefCell::new(Vec::new()));
    let retries = Rc::new(Cell::new(0));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    let on_tick = move || {
        let w = weak.upgrade().unwrap();
        let n = tick.get();
        let case = n / 5;
        let phase = n % 5;
        if !native_window::ready(w.window()) {
            return;
        }
        // The native driver is another event-loop timer. A screenshot timer
        // can run before a pending paused seek was sampled on a busy frame.
        if (w.get_lyrics_position() as f32 - w.get_position() * 1000.).abs() > 2. {
            retries.set(retries.get() + 1);
            if retries.get() == 1 {
                println!(
                    "waiting: media={}, clock={}, view={}, tab={}, minimized={}, offset={}",
                    w.get_position(),
                    w.get_lyrics_position(),
                    w.get_active_view(),
                    w.get_player_tab(),
                    w.window().is_minimized(),
                    w.get_lyrics_effective_offset_ms()
                );
            }
            assert!(
                retries.get() < 20,
                "native clock did not sample the requested position"
            );
            w.window().request_redraw();
            return;
        }
        retries.set(0);
        if phase == 0 {
            frames.borrow_mut().push(capture(&w, case, phase));
            let row = w.get_native_lyric_lines().row_data(0).unwrap();
            let word = row.parts.row_data(row.parts.row_count() - 1).unwrap();
            println!(
                "case {}: {} / {} / {}ms / gradients={} / reduced={}",
                case,
                word.text,
                word.font_family,
                word.duration_ms,
                w.get_native_text_gradients(),
                w.get_lyrics_reduced_motion()
            );
            w.set_position(if motion { 1.5 } else { 5. });
        } else if phase == 1 {
            frames.borrow_mut().push(capture(&w, case, phase));
            w.set_position(if motion { 1.1 } else { 2. });
        } else {
            let current = capture(&w, case, phase);
            let f = frames.borrow();
            verify(&w, &f[0], &f[1], &current, 0.25 * (phase - 1) as f32);
            if phase == 4 {
                drop(f);
                frames.borrow_mut().clear();
                if case + 1 == CASES.len() * 2 {
                    tick.set(n + 1);
                    slint::quit_event_loop().unwrap();
                    return;
                }
                w.set_lyrics_reduced_motion(case + 1 >= CASES.len());
                fixture(&w, CASES[(case + 1) % CASES.len()], motion);
            } else {
                w.set_position(
                    1. + if motion {
                        0.1 * phase as f32
                    } else {
                        phase as f32
                    },
                );
            }
        }
        tick.set(n + 1);
    };
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(650),
        on_tick,
    );
    w.run()?;
    assert_eq!(done.get(), CASES.len() * 10);
    Ok(())
}
