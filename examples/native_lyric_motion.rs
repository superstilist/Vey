//! Continuously sample the real native camera through a cue and a rapid retarget.
use playervf::{LyricRow, MainWindow};
use slint::{ComponentHandle, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

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
                text: "Retarget while still moving".into(),
                start_ms: 3400,
                end_ms: 10000,
                ..Default::default()
            },
        ]))
        .into(),
    );
    let start = Rc::new(Cell::new(None::<Instant>));
    let origin = Instant::now();
    let samples = Rc::new(RefCell::new(Vec::<(i32, f32, f32)>::new()));
    let finished = Rc::new(Cell::new(false));
    let done = finished.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(20),move||{
        let w=weak.upgrade().unwrap();assert!(origin.elapsed().as_secs()<15,"native camera fixture stalled");
        let Some(at)=start.get() else {
            if origin.elapsed().as_millis()<1200 {return;}
            start.set(Some(Instant::now()));w.set_player_state("playing".into());return;
        };
        w.set_position(2.6+at.elapsed().as_secs_f32());
        let ms=w.get_lyrics_position();let y=w.get_native_content_y();let target=w.get_native_scroll_position();
        samples.borrow_mut().push((ms,y,target));
        if ms<4400 {return;}
        w.set_player_state("paused".into());
        let samples=samples.borrow();let first=samples[0].1;
        let before=samples.iter().filter(|s|s.0<3000).collect::<Vec<_>>();
        assert!(before.len()>5);assert!(before.iter().all(|s|(s.1-first).abs()<1.),"camera advanced before the cue");
        let moving=samples.iter().filter(|s|s.0>=3000 && s.0<3400).collect::<Vec<_>>();
        assert!(moving.len()>5);
        let travel=moving.last().unwrap().2;
        assert!(moving.iter().any(|s|(s.1-first).abs()>5. && (s.1+s.2).abs()>5.),"camera snapped instead of moving through intermediate positions");
        assert!(samples.iter().filter(|s|s.0>=3400).any(|s|s.2>travel+50. && (s.1+s.2).abs()>5.),"next cue failed to retarget ongoing motion");
        for pair in samples.windows(2) {
            assert!(pair[1].1<=pair[0].1+0.75,"camera reversed/jittered during forward playback: {pair:?}");
            assert!(pair[1].1>=-pair[1].2-1.,"camera overshot its target");
        }
        let last=samples.last().unwrap();assert!((last.1+last.2).abs()<1.,"camera did not settle: {last:?}");
        std::fs::create_dir_all("target/native-lyrics-check").unwrap();
        std::fs::write("target/native-lyrics-check/motion-samples.json",serde_json::to_vec_pretty(&*samples).unwrap()).unwrap();
        println!("native camera: {} continuous samples, no early movement, intermediate positions, rapid retarget, no reversal/overshoot and exact settling passed",samples.len());
        finished.set(true);slint::quit_event_loop().unwrap();
    });
    w.run()?;
    assert!(done.get());
    Ok(())
}
