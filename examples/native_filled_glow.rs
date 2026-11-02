//! Reported phrase: stable ink and a halo on the sung prefix of an active line.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};
fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(v)).into()
}
fn capture(w: &MainWindow, out: &std::path::Path, name: &str) -> image::RgbaImage {
    let p = w.window().take_snapshot().unwrap();
    let image = image::RgbaImage::from_raw(p.width(), p.height(), p.as_bytes().to_vec()).unwrap();
    image.save(out.join(format!("{name}.png"))).unwrap();
    image
}
struct State {
    step: usize,
    wait: usize,
    off: Option<image::RgbaImage>,
    checks: Vec<serde_json::Value>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1)
    }));
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("NEW_OUTPUT_DIRECTORY required"),
    );
    std::fs::create_dir(&out)?;
    let enforce = std::env::args().any(|a| a == "--assert");
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    w.set_active_view("lyrics".into());
    w.set_duration(12.);
    w.set_lyric_size(48.);
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_position(1.3);
    let mut style = w.get_lyric_style();
    style.glow = false;
    style.fade = false;
    style.wave = std::env::args().any(|a| a == "--wave");
    style.wobble = std::env::args().any(|a| a == "--wobble");
    let reduced = std::env::args().any(|a| a == "--reduced");
    w.set_lyrics_reduced_motion(reduced);
    style.inactive_blur = 0.;
    style.active_scale = 1.;
    style.inactive_scale = 1.;
    w.set_lyric_style(style);
    let words = [
        ("So ", 0, 300),
        ("she ", 300, 700),
        ("ran ", 700, 1000),
        ("into ", 1000, 1800),
        ("the ", 1800, 2300),
        ("bedroom", 2300, 8000),
    ];
    w.set_lyric_lines(model(vec![LyricRow {
        text: "So she ran into the bedroom".into(),
        start_ms: 0,
        end_ms: 9000,
        words: model(
            words
                .into_iter()
                .map(|(text, start_ms, end_ms)| WordRow {
                    text: text.into(),
                    start_ms,
                    end_ms,
                    explicit: true,
                    ..Default::default()
                })
                .collect(),
        ),
        ..Default::default()
    }]));
    let state = Rc::new(RefCell::new(State {
        step: 0,
        wait: 0,
        off: None,
        checks: vec![],
    }));
    let result = state.clone();
    let weak = w.as_weak();
    let output = out.clone();
    let timer = slint::Timer::default();
    let times = [1.3f32, 6.5, 9.7];
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
        let Some(w) = weak.upgrade() else { return };
        if !native_window::ready(w.window()) { return; }
        let mut s = state.borrow_mut();
        s.wait += 1;
        assert!(s.wait < 120, "native clock or paint did not settle");
        if s.wait < 10 || (w.get_lyrics_position() as f32 - w.get_position() * 1000.).abs() > 2. { return; }
        let case = s.step / 2;
        let on = s.step % 2 == 1;
        let frame = capture(&w, &output, &format!("case-{case}-{}", if on { "glow" } else { "plain" }));
        if !on {
            s.off = Some(frame);
            let mut style = w.get_lyric_style();
            style.glow = true;
            w.set_lyric_style(style);
        } else {
            let off = s.off.take().unwrap();
            let mut prefix = 0;
            let mut erased = 0;
            let scale = frame.width() as f32 / 1440.;
            for y in (250. * scale) as u32..(650. * scale) as u32 {
                for x in (750. * scale) as u32..(1400. * scale) as u32 {
                    let a = off.get_pixel(x, y)[0];
                    let b = frame.get_pixel(x, y)[0];
                    if x < (960. * scale) as u32 && a < 100 && b > a.saturating_add(2) { prefix += 1; }
                    if a > 200 && b.saturating_add(3) < a { erased += 1; }
                }
            }
            let check = serde_json::json!({"time": times[case], "prefix_halo_pixels": prefix, "erased_ink_pixels": erased});
            println!("{check}");
            if enforce {
                assert_eq!(erased, 0, "glow cannot erase sharp ink");
                if case == 2 || reduced { assert_eq!(prefix, 0, "inactive/reduced line must release its halo"); }
                else { assert!(prefix > 30, "sung prefix needs a sustained halo"); }
            }
            s.checks.push(check);
            if case == 2 { slint::quit_event_loop().unwrap(); return; }
            let mut style = w.get_lyric_style();
            style.glow = false;
            w.set_lyric_style(style);
            w.set_position(times[case + 1]);
        }
        s.step += 1;
        s.wait = 0;
    });
    w.run()?;
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&result.borrow().checks)?,
    )?;
    Ok(())
}
