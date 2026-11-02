//! Cold/repeated player entry and real cue handoffs; no snapshot readback.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let label = std::env::args().nth(1).unwrap_or("sample".into());
    let line_only = std::env::args().any(|a| a == "--line-only");
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    let render_times = Rc::new(RefCell::new(Vec::<f64>::new()));
    let measured = render_times.clone();
    let mut render_start = Instant::now();
    let notifier = w
        .window()
        .set_rendering_notifier(move |state, _| match state {
            slint::RenderingState::BeforeRendering => render_start = Instant::now(),
            slint::RenderingState::AfterRendering => measured
                .borrow_mut()
                .push(render_start.elapsed().as_secs_f64() * 1000.),
            _ => {}
        });
    println!(
        "render notifier={notifier:?}, native gradients={}",
        w.get_native_text_gradients()
    );
    playervf::navigation_fx::install(&w);
    w.set_navigation_blur(!std::env::args().any(|a| a == "--no-blur"));
    let mut style = w.get_lyric_style();
    if std::env::args().any(|a| a == "--no-focus") {
        style.inactive_blur = 0.;
    }
    if std::env::args().any(|a| a == "--no-wobble") {
        style.wobble = false;
    }
    if std::env::args().any(|a| a == "--no-glow") {
        style.glow = false;
    }
    if std::env::args().any(|a| a == "--reduced") {
        w.set_lyrics_reduced_motion(true);
    }
    w.set_lyric_style(style);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    w.set_duration(300.);
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(!line_only);
    w.set_lyric_lines(
        Rc::new(VecModel::from(
            (0..300)
                .map(|line| {
                    let parts = (0..if line_only { 0 } else { 8 })
                        .map(|word| WordRow {
                            text: format!("vocal{word} ").into(),
                            start_ms: line * 1000 + word * 125,
                            end_ms: line * 1000 + (word + 1) * 125,
                            explicit: true,
                            ..Default::default()
                        })
                        .collect::<Vec<_>>();
                    LyricRow {
                        text: format!("A continuous lyric phrase number {line}").into(),
                        start_ms: line * 1000,
                        end_ms: (line + 1) * 1000,
                        words: Rc::new(VecModel::from(parts)).into(),
                        ..Default::default()
                    }
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    let samples = Rc::new(RefCell::new(Vec::<serde_json::Value>::new()));
    let origin = Instant::now();
    let mut previous = 0.;
    let mut phase = 0;
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(16),move||{
        let w=weak.upgrade().unwrap();let now=origin.elapsed().as_secs_f64();
        if !native_window::ready(w.window()) {previous=now;return;}
        if w.get_native_scroll_snap() && w.get_native_active_line()>=0 && w.get_active_view()=="lyrics" {
            assert!(w.get_native_active_line()>=w.get_native_render_start() && w.get_native_active_line()<w.get_native_render_end(),"snapped cue must already be in the painted band");
        }
        let gap=(now-previous)*1000.;previous=now;
        if now>=1. {samples.borrow_mut().push(serde_json::json!({"at":now,"gap_ms":gap,"phase":phase,"line":w.get_native_active_line(),"moving":w.get_page_moving(),"window":w.get_native_render_start(),"delegates":w.get_native_render_lines().row_count(),"blur_ready":w.get_navigation_fx_ready()}));}
        if now>=2. && phase==0 {w.set_position(100.);w.set_player_state("playing".into());w.set_active_view("lyrics".into());phase=1;}
        if phase==1 {w.set_position(100.+(now-2.) as f32);}
        if now>=7. && phase==1 {w.set_active_view("home".into());phase=2;}
        if now>=8. && phase==2 {w.set_position(106.);w.set_active_view("lyrics".into());phase=3;}
        if phase==3 {w.set_position(106.+(now-8.) as f32);}
        if now>=12. {
            let samples=samples.borrow();
            for p in 1..=3 {let mut gaps=samples.iter().filter(|s|s["phase"]==p).map(|s|s["gap_ms"].as_f64().unwrap()).collect::<Vec<_>>();
                gaps.sort_by(f64::total_cmp);println!("phase {p}: samples={} p95={:.2}ms max={:.2}ms over50={}",gaps.len(),gaps[(gaps.len()-1)*95/100],gaps.last().unwrap(),gaps.iter().filter(|g|**g>50.).count());}
            std::fs::create_dir_all("target/native-lyrics-check").unwrap();
            std::fs::write(format!("target/native-lyrics-check/stutter-{label}.json"),serde_json::to_vec_pretty(&*samples).unwrap()).unwrap();
            let mut renders=render_times.borrow_mut();renders.sort_by(f64::total_cmp);
            if !renders.is_empty() {println!("render CPU: p95={:.2}ms max={:.2}ms",renders[(renders.len()-1)*95/100],renders.last().unwrap());}
            if let Some(pixels)=w.get_navigation_to_image().to_rgba8() {
                assert!(pixels.width()<=768 && pixels.height()<=640);
                assert!(pixels.as_slice().iter().any(|pixel|pixel.a==0),"blur must exclude the background");
                println!("navigation blur pixels verified: {}x{}",pixels.width(),pixels.height());
            }
            slint::quit_event_loop().unwrap();
        }
    });
    w.run()?;
    Ok(())
}
