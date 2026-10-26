//! Continuous native 2x playback: grouped cues, live retargeting and exact paused seek.
use playervf::{LyricRow, MainWindow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

fn capture(w: &MainWindow, name: &str) {
    let frame = w.window().take_snapshot().unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    image::save_buffer(
        format!("target/native-lyrics-check/timing-{name}.png"),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        hook(info);
        std::process::exit(1);
    }));
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    // Exercise the optional anticipatory/grouped camera independently of sung focus.
    w.set_lyrics_scroll_offset_ms(500);
    w.set_lyrics_lookahead_ms(540);
    w.set_lyrics_ending_threshold_ms(500);
    w.set_duration(10.);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    w.set_lyric_lines(ModelRc::new(Rc::new(VecModel::from(
        [1000, 2000, 2200, 2400, 3300, 4500, 5500]
            .into_iter()
            .enumerate()
            .map(|(i, start)| LyricRow {
                text: format!("Timing line {}", i + 1).into(),
                start_ms: start,
                end_ms: start + 950,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))));
    let phase = Rc::new(Cell::new(0));
    let done = phase.clone();
    let baseline = Rc::new(Cell::new(0.));
    let group = Rc::new(Cell::new(0.));
    let previous = Rc::new(Cell::new(0.));
    let checks = Rc::new(Cell::new(0));
    let origin = Instant::now();
    let playback_origin = Rc::new(Cell::new(None::<Instant>));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(25),move||{
        let w=weak.upgrade().unwrap();let p=phase.get();let wall=origin.elapsed().as_secs_f32();
        assert!(wall<15.,"native timing fixture stalled at phase {p}");
        // Warm layout before starting the continuously sampled playback clock.
        if wall<0.6 || w.get_native_lyric_lines().row_count()==0 {return;}
        let Some(playback_start)=playback_origin.get() else {playback_origin.set(Some(Instant::now()));return;};
        if p<5 {w.set_rate(2.);w.set_player_state("playing".into());w.set_position(playback_start.elapsed().as_secs_f32()*2.);}
        let ms=w.get_lyrics_position();let target=w.get_native_scroll_position();
        if p==0 && ms>=400 {baseline.set(target);phase.set(1);}
        else if p==1 && ms>=1550 {
            assert!(target>baseline.get()+100.,"new group must move at its scroll cue: time={ms}, target={target}, baseline={}, anchor={}",baseline.get(),w.get_native_active_line());
            assert_eq!(w.get_active_lyric_index(),0,"scroll lead must not advance the sung lyric");
            group.set(target);phase.set(2);
        } else if p==2 {
            assert!((target-group.get()).abs()<0.5,"a grouped cue scrolled again at {ms}ms");
            checks.set(checks.get()+1);
            // Keep GPU readback outside the continuous timing interval: it
            // blocks the event loop and can legitimately merge a late group.
            if ms>=2150 {assert!(checks.get()>5);phase.set(3);}
        } else if p==3 && ms>=2850 {
            assert!(target>group.get()+50.,"next group must start immediately");
            previous.set(target);phase.set(4);
        } else if p==4 && ms>=4050 {
            assert!(target>previous.get()+50.,"a 750ms motion must not block the next group: time={ms}, target={target}, previous={}, anchor={}",previous.get(),w.get_native_active_line());
            assert!((w.get_native_content_y()+target).abs()>1.,"new target should retarget an ongoing motion");
            capture(&w,"retarget");w.set_player_state("paused".into());w.set_position(1.1);phase.set(5);
        } else if p==5 && ms==1100 {
            assert_eq!(w.get_native_active_line(),0,"paused seek must have no scroll lead");
            assert_eq!(w.get_active_lyric_index(),0);
            assert!((target-baseline.get()).abs()<1.,"paused seek must restore exact line geometry");
            capture(&w,"paused-seek");phase.set(6);
            println!("native timing: 2x clock, grouped cues, retarget during motion and exact paused seek passed ({})",checks.get());
            slint::quit_event_loop().unwrap();
        }
    });
    w.run()?;
    assert_eq!(done.get(), 6);
    Ok(())
}
