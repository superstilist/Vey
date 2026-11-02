//! Long cue gap plus a delayed UI frame must still animate the incoming line.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{LyricRow, MainWindow};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let label = std::env::args().nth(1).unwrap_or("gap".into());
    let wrapped = std::env::args().any(|a| a == "--wrapped");
    let mobile = std::env::args().any(|a| a == "--mobile");
    let capture = std::env::args().any(|a| a == "--capture");
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_duration(20.);
    w.set_position(9.);
    let mut style = w.get_lyric_style();
    style.inactive_blur = 0.;
    style.wobble = false;
    style.glow = false;
    w.set_lyric_style(style);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    if mobile {
        w.set_mobile(true);
        w.window().set_size(slint::LogicalSize::new(390., 844.));
    }
    w.set_lyric_lines(
        Rc::new(VecModel::from(vec![
            LyricRow {
                text: if wrapped {
                    "Before the long silence keep the first row visible. "
                        .repeat(12)
                        .into()
                } else {
                    "Before a long silence".into()
                },
                start_ms: 1000,
                end_ms: 2000,
                ..Default::default()
            },
            LyricRow {
                text: if wrapped {
                    "The incoming wrapped line moves smoothly into view. "
                        .repeat(12)
                        .into()
                } else {
                    "Move smoothly after the gap".into()
                },
                start_ms: 10000,
                end_ms: 18000,
                ..Default::default()
            },
        ]))
        .into(),
    );
    let samples = Rc::new(RefCell::new(vec![]));
    let collected = samples.clone();
    let weak = w.as_weak();
    std::fs::create_dir_all("target/native-lyrics-check")?;
    let capture_label = label.clone();
    let origin = Instant::now();
    let mut began = None;
    let mut blocked = false;
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let w = weak.upgrade().unwrap();
            if !native_window::ready(w.window()) {
                return;
            }
            let at = match began {
                Some(at) => at,
                None => {
                    if origin.elapsed().as_millis() > 1200 {
                        if wrapped {
                            assert_eq!(
                                w.get_native_active_line(),
                                0,
                                "line-only focus skipped to its last wrapped row"
                            );
                            let rows = w.get_native_lyric_lines();
                            let first = (0..rows.row_count())
                                .find(|i| rows.row_data(*i).unwrap().source_index == 1)
                                .unwrap();
                            assert_eq!(
                                w.get_native_preload_line(),
                                first as i32,
                                "upcoming rows must be selected before the cue"
                            );
                        }
                        if capture {
                            save(&w, &capture_label, "before");
                        }
                        began = Some(Instant::now());
                        w.set_player_state("playing".into());
                    }
                    return;
                }
            };
            let wall = Instant::now().duration_since(at).as_millis() as u64;
            if wrapped && w.get_lyrics_position() >= 10000 {
                let rows = w.get_native_lyric_lines();
                let first = (0..rows.row_count())
                    .find(|i| rows.row_data(*i).unwrap().source_index == 1)
                    .unwrap();
                assert_eq!(
                    w.get_native_active_line(),
                    first as i32,
                    "incoming source must start at its first wrapped row"
                );
            }
            if wall >= 800 && !blocked {
                blocked = true;
                std::thread::sleep(Duration::from_millis(650));
            }
            let wall = Instant::now().duration_since(at).as_millis() as u64;
            w.set_position(9. + wall as f32 / 1000.);
            collected.borrow_mut().push((
                wall,
                w.get_lyrics_position(),
                w.get_native_camera_position(),
                w.get_native_scroll_position(),
                w.get_native_scroll_snap(),
            ));
            if wall >= 2600 {
                if capture {
                    save(&w, &capture_label, "after");
                }
                slint::quit_event_loop().unwrap();
            }
        },
    );
    w.run()?;
    let samples = samples.borrow();
    let dir = "target/native-lyrics-check";
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        format!("{dir}/long-gap-{label}.json"),
        serde_json::to_vec_pretty(&*samples)?,
    )?;
    let moved = samples
        .iter()
        .filter(|s| s.1 >= 10000 && s.1 < 10800)
        .collect::<Vec<_>>();
    let intermediate = moved.iter().filter(|s| s.2 > 1. && s.2 < s.3 - 1.).count();
    println!(
        "long-gap {label}: {intermediate} intermediate camera samples; first={:?}",
        moved.first()
    );
    assert!(
        intermediate >= 3,
        "incoming line snapped instead of moving after the delayed frame"
    );
    assert!(
        moved.iter().all(|s| !s.4),
        "ordinary elapsed playback was treated as a seek"
    );
    let last = samples.last().unwrap();
    assert!((last.2 - last.3).abs() < 1.);
    Ok(())
}

fn save(w: &MainWindow, label: &str, phase: &str) {
    let f = w.window().take_snapshot().unwrap();
    image::save_buffer(
        format!("target/native-lyrics-check/long-gap-{label}-{phase}.png"),
        f.as_bytes(),
        f.width(),
        f.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
}
