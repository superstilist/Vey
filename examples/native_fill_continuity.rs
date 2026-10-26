//! Continuous ink coverage on the reported repeated-word/backing-vocal pattern.
use playervf::{LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(v)).into()
}
fn frame(w: &MainWindow) -> image::RgbaImage {
    let f = w.window().take_snapshot().unwrap();
    image::RgbaImage::from_raw(f.width(), f.height(), f.as_bytes().to_vec()).unwrap()
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
    w.set_duration(8.);
    w.set_position(1.8);
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
        start_ms: 1000,
        end_ms: 6000,
        words: model(vec![
            WordRow {
                text: "Feel ".into(),
                start_ms: 1000,
                end_ms: 1440,
                ..Default::default()
            },
            WordRow {
                text: "good".into(),
                start_ms: 1577,
                end_ms: 1988,
                ..Default::default()
            },
            WordRow {
                text: "Change, ".into(),
                start_ms: 1866,
                end_ms: 2434,
                background: true,
                ..Default::default()
            },
            WordRow {
                text: "change, ".into(),
                start_ms: 2828,
                end_ms: 3333,
                background: true,
                ..Default::default()
            },
            WordRow {
                text: "change, ".into(),
                start_ms: 3737,
                end_ms: 4239,
                background: true,
                ..Default::default()
            },
            WordRow {
                text: "change".into(),
                start_ms: 4526,
                end_ms: 5023,
                background: true,
                ..Default::default()
            },
        ]),
        ..Default::default()
    }]));
    let tick = Rc::new(Cell::new(0));
    let done = tick.clone();
    let previous = Rc::new(RefCell::new(None::<image::RgbaImage>));
    let drops = Rc::new(Cell::new(0usize));
    let advances = Rc::new(Cell::new(0usize));
    let advancing_frames = Rc::new(Cell::new(0usize));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(150),
        move || {
            let w = weak.upgrade().unwrap();
            let i = tick.get();
            if i < 6 {
                tick.set(i + 1);
                return;
            }
            let n = i - 6;
            let current = frame(&w);
            if let Some(old) = previous.borrow().as_ref() {
                let previous_advances = advances.get();
                for y in 250..650 {
                    for x in 720..current.width() - 40 {
                        let a = old.get_pixel(x, y)[0];
                        let b = current.get_pixel(x, y)[0];
                        if a > b.saturating_add(4) {
                            drops.set(drops.get() + 1);
                        }
                        if b > a.saturating_add(4) {
                            advances.set(advances.get() + 1);
                        }
                    }
                }
                if advances.get() > previous_advances {
                    advancing_frames.set(advancing_frames.get() + 1);
                }
            }
            if n % 10 == 0 {
                std::fs::create_dir_all("target/native-lyrics-check").unwrap();
                current
                    .save(format!(
                        "target/native-lyrics-check/fill-continuity-{n}.png"
                    ))
                    .unwrap();
            }
            *previous.borrow_mut() = Some(current);
            if n == 40 {
                println!(
                    "continuous native fill: advances={}, backward pixels={}",
                    advances.get(),
                    drops.get()
                );
                assert!(advances.get() > 1000, "the cue must visibly fill");
                assert!(
                    advancing_frames.get() >= 24,
                    "fill must advance through the cue, not switch once: {} frames",
                    advancing_frames.get()
                );
                println!("advancing fill frames: {}/40", advancing_frames.get());
                assert_eq!(
                    drops.get(),
                    0,
                    "ink coverage went backwards while its cue advanced"
                );
                tick.set(47);
                slint::quit_event_loop().unwrap();
                return;
            }
            w.set_position((1866. + 568. * (n + 1) as f32 / 40.) / 1000.);
            tick.set(i + 1);
        },
    );
    w.run()?;
    assert_eq!(done.get(), 47);
    Ok(())
}
