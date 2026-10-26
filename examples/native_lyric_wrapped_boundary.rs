//! A wrapped continuation must stay unfilled until the preceding fragment finishes.
use playervf::{LyricRow, MainWindow, WordRow};
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
    f.save(format!(
        "target/native-lyrics-check/wrapped-boundary-{name}.png"
    ))
    .unwrap();
    f
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1);
    }));
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_mobile(true);
    w.window().set_size(slint::LogicalSize::new(390., 844.));
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_duration(8.);
    w.set_position(0.9);
    let mut s = w.get_lyric_style();
    s.glow = false;
    s.fade = false;
    s.wave = false;
    s.wobble = false;
    s.inactive_blur = 0.;
    s.active_scale = 1.;
    s.inactive_scale = 1.;
    w.set_lyric_style(s);
    w.set_lyric_lines(
        Rc::new(VecModel::from(vec![LyricRow {
            text: "Illuminate continuation".into(),
            start_ms: 0,
            end_ms: 7000,
            words: Rc::new(VecModel::from(vec![WordRow {
                text: "Illuminate continuation".into(),
                start_ms: 1000,
                end_ms: 5000,
                ..Default::default()
            }]))
            .into(),
            ..Default::default()
        }]))
        .into(),
    );
    let step = Rc::new(Cell::new(0));
    let done = step.clone();
    let frames = Rc::new(RefCell::new(Vec::new()));
    let boundary = Rc::new(Cell::new(0.));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(900),move||{
        let w=weak.upgrade().unwrap();let i=step.get();let rows=w.get_native_lyric_lines();assert_eq!(rows.row_count(),2);
        frames.borrow_mut().push(capture(&w,&i.to_string()));
        if i==0 {let first=rows.row_data(0).unwrap().parts.row_data(0).unwrap();let t=(first.start_ms+first.duration_ms*first.cue_span).ceil()+2.;boundary.set(t);w.set_position((t-32.)/1000.);}
        else if i==1 {assert_eq!(w.get_native_active_line(),0);w.set_position(boundary.get()/1000.);}
        else if i==2 {assert_eq!(w.get_native_active_line(),1);w.set_position(5.2);}
        else {
            let f=frames.borrow();let mut ink=vec![];
            for y in f[3].height()/6..f[3].height()*7/10 {for x in 20..f[3].width()-20 {
                let off=f[0].get_pixel(x,y);let full=f[3].get_pixel(x,y);
                if (0..3).all(|c|full[c]>215 && full[c] as i16-off[c] as i16>70){ink.push((x,y));}
            }}
            assert!(ink.len()>100);let top=ink.iter().map(|p|p.1).min().unwrap();let bottom=ink.iter().map(|p|p.1).max().unwrap();let middle=(top+bottom)/2;
            let mut first=0;let mut next=0;let mut late=0;let mut premature=0;
            for (x,y) in ink {if y<middle {
                first+=1;let at=f[2].get_pixel(x,y);let full=f[3].get_pixel(x,y);
                if (0..3).any(|c|(at[c] as i16-full[c] as i16).abs()>1){late+=1;}
            }else{
                next+=1;let before=f[1].get_pixel(x,y);let off=f[0].get_pixel(x,y);
                if (0..3).any(|c|(before[c] as i16-off[c] as i16).abs()>3){premature+=1;}
            }}
            assert!(first>30&&next>30);assert_eq!(late,0,"previous row did not finish at its boundary");assert_eq!(premature,0,"next row started before the previous row finished");
            println!("wrapped boundary: {first} completed first-row cores, {next} second-row cores, no late/premature fill");slint::quit_event_loop().unwrap();
        }
        step.set(i+1);
    });
    w.run()?;
    assert_eq!(done.get(), 4);
    Ok(())
}
