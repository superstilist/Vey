//! Isolated native ambient cadence during repeated page navigation.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::MainWindow;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Samples {
    started: Option<Instant>,
    last_frame: slint::Image,
    idle: usize,
    travel: usize,
    resumed: usize,
    paused: usize,
    reduced: usize,
    pause_set: bool,
    reduced_set: bool,
    previous_tick: Option<Instant>,
    travel_intervals_ms: Vec<f64>,
    travelling_samples: usize,
    switches: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let legacy = std::env::var("VEY_LEGACY_AMBIENT_TRAVEL").as_deref() == Ok("1");
    let window = MainWindow::new()?;
    playervf::i18n::install(&window);
    let _ambient = playervf::ambient_native::install(&window);
    window
        .window()
        .set_size(slint::LogicalSize::new(960., 640.));
    let art = image::RgbaImage::from_fn(256, 256, |x, y| {
        image::Rgba([x as u8, y as u8, (x / 2 + y / 2) as u8, 255])
    });
    window.set_current_art(slint::Image::from_rgba8(
        slint::SharedPixelBuffer::clone_from_slice(art.as_raw(), 256, 256),
    ));
    window.set_player_state("playing".into());
    let samples = Rc::new(RefCell::new(Samples::default()));
    let timer_samples = samples.clone();
    let weak = window.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(10),
        move || {
            let Some(window) = weak.upgrade() else { return };
            if !native_window::ready(window.window()) {
                return;
            }
            let mut state = timer_samples.borrow_mut();
            if state.started.is_none() {
                if window.get_ambient_frame().size().width == 0 {
                    return;
                }
                state.started = Some(Instant::now());
            }
            let elapsed = state.started.unwrap().elapsed().as_millis();
            if elapsed >= 4400 && !state.pause_set {
                state.pause_set = true;
                window.set_player_state("paused".into());
            }
            if elapsed >= 5400 && !state.reduced_set {
                state.reduced_set = true;
                window.set_lyrics_reduced_motion(true);
                window.set_player_state("playing".into());
            }
            if (1600..3200).contains(&elapsed) && elapsed >= 1600 + state.switches as u128 * 130 {
                state.switches += 1;
                window.set_active_view(
                    if state.switches % 2 == 0 {
                        "home"
                    } else {
                        "library"
                    }
                    .into(),
                );
            }
            let moving = window.get_page_moving();
            let now = Instant::now();
            if let Some(previous) = state.previous_tick.replace(now) {
                if (1600..3200).contains(&elapsed) && moving {
                    state
                        .travel_intervals_ms
                        .push(now.duration_since(previous).as_secs_f64() * 1000.);
                }
            }
            if (1600..3200).contains(&elapsed) && moving {
                state.travelling_samples += 1;
            }
            let frame = window.get_ambient_frame();
            if frame != state.last_frame {
                if (800..1600).contains(&elapsed) {
                    state.idle += 1;
                }
                if (1600..3200).contains(&elapsed) && moving {
                    state.travel += 1;
                }
                if (3600..4400).contains(&elapsed) {
                    state.resumed += 1;
                }
                if (4800..5400).contains(&elapsed) {
                    state.paused += 1;
                }
                if (5800..6400).contains(&elapsed) {
                    state.reduced += 1;
                }
                state.last_frame = frame;
            }
            if elapsed >= 6400 {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    window.run()?;
    let result = samples.borrow();
    let mut intervals = result.travel_intervals_ms.clone();
    intervals.sort_by(f64::total_cmp);
    let p95 = intervals
        .get(intervals.len().saturating_sub(1) * 95 / 100)
        .copied();
    println!(
        "{}",
        serde_json::json!({
            "legacy": legacy, "idle_frames_800ms": result.idle,
            "travel_frames_1600ms": result.travel, "resumed_frames_800ms": result.resumed,
            "paused_frames_600ms": result.paused, "reduced_frames_600ms": result.reduced,
            "travelling_samples": result.travelling_samples, "route_switches": result.switches,
            "ambient_opacity": window.get_ambient_opacity(),
            "travel_event_loop_p95_ms": p95,
        })
    );
    assert!(
        result.travelling_samples > 5,
        "fixture did not observe page travel"
    );
    assert!(
        result.travel > 0 && result.idle > 0 && result.resumed > 0,
        "ambient frames must continue"
    );
    assert_eq!(result.paused, 0, "paused ambient must settle");
    assert_eq!(result.reduced, 0, "reduced-motion ambient must settle");
    if !legacy {
        assert!(result.travel <= 18, "travel frame cap was exceeded");
    }
    Ok(())
}
