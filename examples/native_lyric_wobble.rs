//! Actual native pixels: whole-word wobble, still unsung text, resting ink and independent letter waves.
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
    let frame = w.window().take_snapshot().unwrap();
    let image =
        image::RgbaImage::from_raw(frame.width(), frame.height(), frame.as_bytes().to_vec())
            .unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    image
        .save(format!("target/native-lyrics-check/wobble-{name}.png"))
        .unwrap();
    image
}
fn changed(a: &image::RgbaImage, b: &image::RgbaImage, left: u32) -> usize {
    (300..480)
        .flat_map(|y| (left..a.width() - 40).map(move |x| (x, y)))
        .filter(|&(x, y)| a.get_pixel(x, y) != b.get_pixel(x, y))
        .count()
}
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
    w.set_lyrics_word_timed(true);
    w.set_duration(8.);
    w.set_position(1.22);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let mut style = w.get_lyric_style();
    assert!(
        style.wobble,
        "normal native theme should enable word motion"
    );
    style.glow = false;
    style.fade = false;
    style.swipe = false;
    style.wave = false;
    style.wobble = false;
    style.inactive_blur = 0.;
    style.active_scale = 1.;
    style.inactive_scale = 1.;
    w.set_lyric_style(style);
    let (lyrics,format,sync_type)=lyrics::parse_payload("<tt><body><p begin='1s' end='6s'><span begin='1s' end='1.2s'>Il</span><span begin='1.2s' end='2s'>luminate </span><span begin='3s' end='4s'>quiet</span></p></body></tt>").unwrap();
    w.set_lyric_lines(
        Rc::new(VecModel::from(braccato::rows(
            &LyricsAlternative {
                lyrics,
                format: format.into(),
                sync_type,
                provider: "wobble fixture".into(),
            },
            8000,
        )))
        .into(),
    );
    let phase = Rc::new(Cell::new(0));
    let done = phase.clone();
    let frames = Rc::new(RefCell::new(Vec::new()));
    let retries = Rc::new(Cell::new(0));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(850),move||{
        let w=weak.upgrade().unwrap();let p=phase.get();
        if p==0 {
            let row=w.get_native_lyric_lines().row_data(0).unwrap();let first=row.parts.row_data(0).unwrap();let second=row.parts.row_data(1).unwrap();
            assert_eq!((first.motion_start_ms,first.motion_duration_ms),(1000.,1000.));
            assert_eq!((second.motion_start_ms,second.motion_duration_ms),(1000.,1000.));
            let offset=playervf::native_lyric_fonts::measure_with(&first.text,48.,&first.font_family,700)/48.;
            assert!((first.motion_origin_em-second.motion_origin_em-offset).abs()<0.001,"syllables need the same global transform origin");
            frames.borrow_mut().push(capture(&w,"off"));let mut s=w.get_lyric_style();s.wobble=true;w.set_lyric_style(s);
        } else if p==1 {
            let peak=capture(&w,"peak");let f=frames.borrow();let difference=changed(&f[0],&peak,720);
            assert!(difference>100,"word wobble did not move ink: {difference}");
            let mut right=0;for y in 300..480 {for x in 720..1400 {if f[0].get_pixel(x,y).0[..3].iter().all(|v|*v>210){right=right.max(x);}}}
            assert!(right>720);assert_eq!(changed(&f[0],&peak,right+16),0,"unsung following word moved early");
            println!("word wobble: {difference} changed pixels, syllables share one origin, unsung following word stayed still");w.set_position(2.2);
        } else if p==2 {
            let settled=capture(&w,"settled");assert_eq!(changed(&frames.borrow()[0],&settled,720),0,"word motion failed to restore exact resting pixels");
            w.set_lyrics_reduced_motion(true);w.set_position(1.22);
        } else if p==3 {
            let reduced=capture(&w,"reduced");assert_eq!(changed(&frames.borrow()[0],&reduced,720),0,"reduced motion still wobbled");
            w.set_lyrics_reduced_motion(false);let mut s=w.get_lyric_style();s.wobble=false;s.wave=true;w.set_lyric_style(s);
        } else if p==4 {
            let row=w.get_native_lyric_lines().row_data(0).unwrap();
            if (0..row.parts.row_count()).any(|i|row.parts.row_data(i).unwrap().glyphs.row_count()==0) {
                retries.set(retries.get()+1);assert!(retries.get()<12,"letter plans timed out");return;
            }
            let wave=capture(&w,"letters-independent");assert!(changed(&frames.borrow()[0],&wave,720)>50,"letter wave incorrectly depends on word wobble");
            w.set_position(4.8);
        } else if p==5 {
            frames.borrow_mut().push(capture(&w,"letters-resting"));let mut s=w.get_lyric_style();s.wave=false;w.set_lyric_style(s);
        } else if p==6 {
            let plain=capture(&w,"plain-resting");assert_eq!(changed(&frames.borrow()[1],&plain,720),0,"settled letter wave did not return to the shaped run");
            println!("native wobble: exact rest, reduced motion, independent letter wave and resting single-run pixels passed");
            phase.set(7);slint::quit_event_loop().unwrap();return;
        }
        phase.set(p+1);
    });
    w.run()?;
    assert_eq!(done.get(), 7);
    Ok(())
}
