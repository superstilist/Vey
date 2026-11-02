//! Sample actual camera velocity through a short cue and a mid-flight retarget.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{LyricRow, MainWindow};
use slint::{ComponentHandle, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(serde::Serialize)]
struct Sample {
    wall: f64,
    ms: i32,
    y: f32,
    target: f32,
    camera: f32,
    driven: bool,
    extent: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let label = std::env::args().nth(1).unwrap_or("sample".into());
    let legacy = std::env::args().any(|arg| arg == "--legacy");
    if legacy {
        std::env::set_var("VEY_LEGACY_LYRIC_SCROLL", "1");
    }
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_position(2.6);
    w.set_duration(12.);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    w.set_lyric_lines(
        Rc::new(VecModel::from(vec![
            LyricRow {
                text: "The first line stays focused".into(),
                start_ms: 1000,
                end_ms: 3000,
                ..Default::default()
            },
            LyricRow {
                text: "A soft move to the next line".into(),
                start_ms: 3000,
                end_ms: 3400,
                ..Default::default()
            },
            LyricRow {
                text: "Keep moving through the next cue".into(),
                start_ms: 3400,
                end_ms: 10000,
                ..Default::default()
            },
        ]))
        .into(),
    );
    let origin = Instant::now();
    let mut began = None::<Instant>;
    let samples = Rc::new(RefCell::new(Vec::<Sample>::new()));
    let collected = samples.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let w = weak.upgrade().unwrap();
            if !native_window::ready(w.window()) {
                return;
            }
            let Some(at) = began else {
                if origin.elapsed().as_millis() > 1200 {
                    began = Some(Instant::now());
                    w.set_player_state("playing".into());
                }
                return;
            };
            let wall = at.elapsed().as_secs_f64() * 1000.;
            w.set_position(2.6 + (wall / 1000.) as f32);
            collected.borrow_mut().push(Sample {
                wall,
                ms: w.get_lyrics_position(),
                y: w.get_native_content_y(),
                target: w.get_native_scroll_position(),
                camera: w.get_native_camera_position(),
                driven: w.get_native_camera_driven(),
                extent: w.get_native_content_height(),
            });
            if wall >= 2200. {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    w.run()?;
    timer.stop();
    let samples = samples.borrow();
    std::fs::create_dir_all("target/native-lyrics-check")?;
    std::fs::write(
        format!("target/native-lyrics-check/line-flow-{label}.json"),
        serde_json::to_vec_pretty(&*samples)?,
    )?;
    let mut means = vec![];
    for interval in [(3280, 3380), (3420, 3520), (3520, 3620)] {
        let speeds = samples
            .windows(2)
            .filter(|pair| pair[1].ms >= interval.0 && pair[1].ms < interval.1)
            .map(|pair| -(pair[1].y - pair[0].y) as f64 / (pair[1].wall - pair[0].wall) * 1000.)
            .collect::<Vec<_>>();
        // Software rendering may produce only two real frames per 100ms.
        assert!(speeds.len() >= 2, "need moving frames around the retarget");
        let mean = speeds.iter().sum::<f64>() / speeds.len() as f64;
        means.push(mean);
        println!(
            "velocity {}..{}: samples={}, mean={mean:.2}px/s",
            interval.0,
            interval.1,
            speeds.len()
        );
    }
    if !legacy {
        assert!(
            means[0] > 50. && means[1] >= means[0] * 0.75,
            "line retarget braked into a visible pause: {means:?}"
        );
        assert!(samples.iter().any(|s| s.driven), "native camera must drive");
        let settled = samples
            .iter()
            .filter(|s| s.driven && (s.camera - s.target).abs() < 0.5)
            .collect::<Vec<_>>();
        assert!(settled.len() > 10, "need settled camera frames");
        assert!(
            settled.iter().all(|s| (s.y + s.camera).abs() < 1.5),
            "displayed content must track the settled camera"
        );
    }
    // A settled line must move at once, not spend a quarter second at rest.
    let start = samples
        .windows(2)
        .position(|pair| pair[1].target - pair[0].target > 100.)
        .map(|i| i + 1)
        .expect("fixture must commit the next line");
    let base = samples[start].y;
    let moved = samples[start..]
        .iter()
        .find(|s| base - s.y >= 20.)
        .expect("line transition must move");
    let latency = moved.wall - samples[start].wall;
    println!("20px launch latency: {latency:.1}ms");
    assert!(
        latency < 200.,
        "line transition stalled at launch: {latency:.1}ms"
    );
    for pair in samples.windows(2) {
        assert!(pair[1].y <= pair[0].y + 0.75, "camera reversed");
    }
    let last = samples.last().unwrap();
    assert!(
        (last.y + last.target).abs() < 1.,
        "camera must finish exactly"
    );
    Ok(())
}
