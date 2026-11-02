//! Real Slint navigation latency, without timed screenshot readbacks.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{MainWindow, TrackRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

struct Run {
    route: usize,
    request: Option<Instant>,
    next: Instant,
    previous: Instant,
    samples: Vec<serde_json::Value>,
    intervals: Vec<f64>,
    first: Option<f64>,
    origin: (f32, f32),
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .expect("NEW_OUTPUT_DIRECTORY required");
    std::fs::create_dir(&output)?;
    let window = MainWindow::new()?;
    playervf::i18n::install(&window);
    playervf::navigation_fx::install(&window);
    window
        .window()
        .set_size(slint::LogicalSize::new(1100., 760.));
    let rows: ModelRc<TrackRow> = Rc::new(VecModel::from(
        (0..300)
            .map(|i| TrackRow {
                title: format!("Navigation fixture {i}").into(),
                artist: "Synthetic library".into(),
                duration: "3:00".into(),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))
    .into();
    window.set_tracks(rows.clone());
    window.set_home_page_tracks(rows.clone());
    window.set_library_page_tracks(rows.clone());
    window.set_liked_page_tracks(rows.clone());
    window.set_search_page_tracks(rows);
    let routes = [
        "library", "settings", "home", "liked", "search", "home", "library", "settings", "home",
    ];
    let now = Instant::now();
    let run = Rc::new(RefCell::new(Run {
        route: 0,
        request: None,
        next: now + Duration::from_millis(150),
        previous: now,
        samples: vec![],
        intervals: vec![],
        first: None,
        origin: (0., 0.),
    }));
    let result = run.clone();
    let weak = window.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(8),move || {
        let Some(w) = weak.upgrade() else {return};
        if !native_window::ready(w.window()) {return;}
        let now = Instant::now();
        let mut r = run.borrow_mut();
        let interval = now.duration_since(r.previous).as_secs_f64()*1000.;
        r.previous = now;
        if r.request.is_some() {r.intervals.push(interval);}
        let position = || (w.get_player_motion_x(),w.get_player_motion_y());
        if let Some(request) = r.request {
            assert!(request.elapsed()<Duration::from_secs(5),"navigation did not settle");
            let p = position();
            if r.first.is_none() && (p.0-r.origin.0).abs().max((p.1-r.origin.1).abs())>5. {
                r.first=Some(request.elapsed().as_secs_f64()*1000.);
            }
            if !w.get_page_moving() && w.get_displayed_view()==routes[r.route] {
                let sample=serde_json::json!({"route":routes[r.route],"first_5px_ms":r.first,
                    "settled_ms":request.elapsed().as_secs_f64()*1000.,"duration_ms":w.get_camera_duration()});
                r.samples.push(sample);
                r.route+=1;r.request=None;r.next=now+Duration::from_millis(64);
            }
        } else if now>=r.next {
            if r.route==routes.len() {slint::quit_event_loop().unwrap();return;}
            r.origin=position();r.first=None;r.request=Some(Instant::now());
            w.set_active_view(routes[r.route].into());
        }
    });
    window.run()?;
    let r = result.borrow();
    assert_eq!(r.samples.len(), routes.len());
    let mut intervals = r.intervals.clone();
    intervals.sort_by(f64::total_cmp);
    let report = serde_json::json!({"samples":r.samples,"event_loop_ms":{"p95":intervals[(intervals.len()-1)*95/100],"max":intervals.last()},
        "workload":{"tracks":300,"viewport":[1100,760],"timer_ms":8},"legacy_capture_env":std::env::var_os("VEY_LEGACY_NAVIGATION_CAPTURE").is_some()});
    std::fs::write(
        std::path::Path::new(&output).join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    let pixels = window.window().take_snapshot()?;
    image::save_buffer(
        std::path::Path::new(&output).join("settled.png"),
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ColorType::Rgba8,
    )?;
    println!("{report}");
    Ok(())
}
