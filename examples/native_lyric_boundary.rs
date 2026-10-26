//! Actual pixels at a cue deadline, plus scroll/stage focus before the next line.
use playervf::{
    braccato,
    lyrics::{self, LyricsAlternative},
    MainWindow,
};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
fn capture(w: &MainWindow, name: &str) -> image::RgbaImage {
    let f = w.window().take_snapshot().unwrap();
    let f = image::RgbaImage::from_raw(f.width(), f.height(), f.as_bytes().to_vec()).unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    f.save(format!("target/native-lyrics-check/boundary-{name}.png"))
        .unwrap();
    f
}
fn same_complete(before: &image::RgbaImage, end: &image::RgbaImage, settled: &image::RgbaImage) {
    let mut fill_changes = 0;
    let mut late_changes = 0;
    // Exclude transport/time controls. No scrolling or layout changes occur in this interval.
    for y in end.height() / 5..end.height() * 7 / 10 {
        for x in end.width() / 2..end.width() - 30 {
            let a = before.get_pixel(x, y);
            let b = end.get_pixel(x, y);
            let c = settled.get_pixel(x, y);
            if (0..3).any(|i| (a[i] as i16 - b[i] as i16).abs() > 3) {
                fill_changes += 1;
            }
            if (0..3).any(|i| (b[i] as i16 - c[i] as i16).abs() > 1) {
                late_changes += 1;
            }
        }
    }
    assert!(
        fill_changes > 30,
        "the before frame must still be filling: {fill_changes}"
    );
    assert_eq!(
        late_changes, 0,
        "ink continued changing after its supplied cue end"
    );
    println!(
        "cue deadline pixels: {fill_changes} pixels completed by the end, {late_changes} late"
    );
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1);
    }));
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    playervf::native_lyrics::install(&w);
    assert_eq!(w.get_lyrics_scroll_offset_ms(), 0);
    assert_eq!(w.get_lyrics_lookahead_ms(), 0);
    let mut style = w.get_lyric_style();
    style.glow = false;
    style.fade = false;
    style.wave = false;
    style.wobble = false;
    style.inactive_blur = 0.;
    style.active_scale = 1.;
    style.inactive_scale = 1.;
    w.set_lyric_style(style);
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_duration(10.);
    w.set_position(2.6);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let (lyrics,format,sync_type)=lyrics::parse_payload("<tt><body><p begin='1s' end='5s'><span begin='1s' end='3s'>Illuminate </span><span begin='4s' end='5s'>later</span></p><p begin='5s' end='7s'><span begin='5s' end='6s'>Next </span><span begin='6s' end='7s'>line</span></p></body></tt>").unwrap();
    w.set_lyric_lines(
        Rc::new(VecModel::from(braccato::rows(
            &LyricsAlternative {
                lyrics,
                format: format.into(),
                sync_type,
                provider: "boundary fixture".into(),
            },
            10000,
        )))
        .into(),
    );
    let frames = Rc::new(RefCell::new(Vec::new()));
    let step = Rc::new(Cell::new(0));
    let done = step.clone();
    let attempts = Rc::new(Cell::new(0));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(900),move||{
        let w=weak.upgrade().unwrap();let i=step.get();
        if i==3 && w.get_native_lyric_lines().row_data(0).unwrap().parts.row_data(0).unwrap().glyphs.row_count()==0 {
            attempts.set(attempts.get()+1);assert!(attempts.get()<12,"letter plans timed out");return;
        }
        if i<6 {
            frames.borrow_mut().push(capture(&w,&format!("{}-{}",if i<3{"run"}else{"letters"},i%3)));
            match i%3 {0=>w.set_position(3.),1=>w.set_position(3.4),_=>{
                let mut f=frames.borrow_mut();same_complete(&f[0],&f[1],&f[2]);f.clear();
                if i==2 {let mut s=w.get_lyric_style();s.wave=true;w.set_lyric_style(s);w.set_position(2.6);}
                else {w.set_position(4.6);w.set_player_state("playing".into());}
            }}
        } else if i==6 {
            assert!(w.get_lyrics_position()<5000);assert_eq!(w.get_native_active_line(),0,"scroll focus must not anticipate the next cue");
            assert_eq!(w.get_active_lyric_index(),0);capture(&w,"scroll-before-next");
            w.set_player_state("paused".into());w.set_position(4.95);w.set_lyrics_stage(true);
        } else if i==7 {
            let rows=w.get_native_lyric_lines();
            assert_eq!(rows.row_data(0).unwrap().stage_role,"current","stage must retain the unfinished line");
            assert_eq!(rows.row_data(1).unwrap().stage_role,"queued");capture(&w,"stage-before-next");w.set_position(5.);
        } else if i==8 {
            assert_eq!(w.get_native_active_line(),1);assert_eq!(w.get_native_lyric_lines().row_data(1).unwrap().stage_role,"current");
            capture(&w,"stage-at-next");println!("native boundary: whole-run/letter fill completes at cue end; scroll and stage wait for the next cue");
            slint::quit_event_loop().unwrap();
        }
        step.set(i+1);
    });
    w.run()?;
    assert_eq!(done.get(), 9);
    Ok(())
}
