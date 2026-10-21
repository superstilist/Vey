//! Real Windows WebView2 decoding/transport check using the generated MP4 fixture.
#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use base64::Engine;
    use playervf::{
        video::{install_source, VideoSource},
        MainWindow,
    };
    use slint::ComponentHandle;
    use std::{
        cell::Cell,
        rc::Rc,
        time::{Duration, Instant},
    };
    let video = std::fs::read("target/video-surface-fixture.mp4")?;
    let window = MainWindow::new()?;
    playervf::i18n::install(&window);
    window.set_lyrics_reduced_motion(true);
    window.set_current_track_key("fixture".into());
    window.set_current_title("Native video fixture".into());
    window.set_art_mode("clip".into());
    window.set_active_view("lyrics".into());
    window.set_player_state("paused".into());
    window.set_duration(4.);
    window.show()?;
    let _surface = install_source(
        &window,
        VideoSource {
            url: format!(
                "data:video/mp4;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(video)
            ),
            mime: "video/mp4".into(),
        },
    );
    let started = Instant::now();
    let step = Rc::new(Cell::new(0));
    let result = step.clone();
    let clock = Rc::new(Cell::new(Instant::now()));
    let weak = window.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let w = weak.upgrade().unwrap();
            if started.elapsed() >= Duration::from_secs(30) || !w.get_video_muted() {
                eprintln!("Native video timed out: step={} pos={} stage={} moving={} reveal={} bounds={}x{}",step.get(),w.get_video_position(),w.get_player_on_stage(),w.get_page_moving(),w.get_player_reveal(),w.get_video_width(),w.get_video_height());
                step.set(-1);
                slint::quit_event_loop().unwrap();
                return;
            }
            match step.get() {
                0 if w.get_video_ready() => {
                    println!("PASS native decoded frame");
                    w.set_position(1.5);
                    step.set(1);
                }
                1 if w.get_video_ready() && (w.get_video_position() - 1.5).abs() < 0.08 => {
                    println!("PASS native paused seek");
                    clock.set(Instant::now());
                    w.set_player_state("playing".into());
                    step.set(2);
                }
                2 => {
                    w.set_position(1.5 + clock.get().elapsed().as_secs_f32());
                    if clock.get().elapsed() > Duration::from_millis(700)
                        && w.get_video_position() > 1.9
                    {
                        println!("PASS native playback follows song clock");
                        w.set_art_mode("cover".into());
                        step.set(3);
                    }
                }
                3 if !w.get_video_ready() => {
                    println!("PASS native cover fallback and mute");
                    slint::quit_event_loop().unwrap();
                    step.set(4);
                }
                _ => {}
            }
        },
    );
    window.run()?;
    assert_eq!(result.get(), 4);
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn main() {
    println!("This example checks the Windows WebView2 surface.");
}
