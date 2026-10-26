//! Pixel proof that native focus fades in both directions without a bright double copy.
use playervf::{LyricRow, MainWindow};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

fn capture(w: &MainWindow, name: &str) -> image::RgbaImage {
    let f = w.window().take_snapshot().unwrap();
    let image = image::RgbaImage::from_raw(f.width(), f.height(), f.as_bytes().to_vec()).unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    image
        .save(format!("target/native-lyrics-check/handoff-{name}.png"))
        .unwrap();
    image
}
fn difference(a: &image::RgbaImage, b: &image::RgbaImage) -> u64 {
    let mut sum = 0;
    for y in 300..480 {
        for x in a.width() / 2..a.width() - 40 {
            let a = a.get_pixel(x, y);
            let b = b.get_pixel(x, y);
            sum += (0..3).map(|c| a[c].abs_diff(b[c]) as u64).sum::<u64>();
        }
    }
    sum
}
fn intermediate(a: &image::RgbaImage, middle: &image::RgbaImage, b: &image::RgbaImage) {
    let total = difference(a, b);
    let from = difference(a, middle);
    let to = difference(middle, b);
    assert!(
        total > 10000,
        "focus endpoints must visibly differ: {total}"
    );
    assert!(
        from > total / 100 && to > total / 100,
        "focus snapped instead of blending: {from}/{to}/{total}"
    );
    // Alpha crossfades may darken overlapping coverage, but must never add light.
    let mut brighter = 0;
    for y in 300..480 {
        for x in a.width() / 2..a.width() - 40 {
            let p = middle.get_pixel(x, y);
            let first = a.get_pixel(x, y);
            let last = b.get_pixel(x, y);
            if (0..3).any(|c| p[c] > first[c].max(last[c]).saturating_add(3)) {
                brighter += 1;
            }
        }
    }
    assert_eq!(brighter, 0, "focus handoff added a bright duplicate");
    println!("focus pixels: endpoint difference={total}, intermediate distances={from}/{to}, bright duplicates={brighter}");
}
fn toggle(w: &MainWindow, active: bool) {
    let rows = w.get_native_lyric_lines();
    let mut row = rows.row_data(0).unwrap();
    row.active = active;
    row.sustain_distance = if active { 0 } else { 1 };
    rows.set_row_data(0, row);
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
    w.set_position(2.);
    w.set_duration(10.);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let mut style = w.get_lyric_style();
    style.glow = false;
    style.wobble = false;
    style.wave = false;
    style.active_scale = 1.;
    style.inactive_scale = 1.;
    w.set_lyric_style(style);
    w.set_lyric_lines(
        Rc::new(VecModel::from(vec![
            LyricRow {
                text: "Illuminate fading line".into(),
                start_ms: 1000,
                end_ms: 5000,
                ..Default::default()
            },
            LyricRow {
                text: "The next line stays quiet".into(),
                start_ms: 6000,
                end_ms: 9000,
                ..Default::default()
            },
        ]))
        .into(),
    );
    let phase = Rc::new(Cell::new(0));
    let done = phase.clone();
    let frames = Rc::new(RefCell::new(Vec::new()));
    let since = Rc::new(Cell::new(Instant::now()));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(30),move||{
        let w=weak.upgrade().unwrap(); let p=phase.get(); let elapsed=since.get().elapsed().as_millis();
        if p==0 {
            if elapsed<1200 {return;}
            let rows=w.get_native_lyric_lines();
            assert!(elapsed<10000,"focus mask did not warm");
            if !rows.row_data(0).is_some_and(|r|r.sustain_blurred.size().width>0) {return;}
            frames.borrow_mut().push(capture(&w,"sharp"));toggle(&w,false);
        } else if p==1 && elapsed>=45 {println!("outgoing sample at {elapsed}ms");frames.borrow_mut().push(capture(&w,"out-middle"));}
        else if p==2 && elapsed>=500 {
            let end=capture(&w,"blurred");let f=frames.borrow();intermediate(&f[0],&f[1],&end);drop(f);
            frames.borrow_mut().push(end);toggle(&w,true);
        } else if p==3 && elapsed>=45 {println!("incoming sample at {elapsed}ms");frames.borrow_mut().push(capture(&w,"in-middle"));}
        else if p==4 && elapsed>=400 {
            let end=capture(&w,"restored");let f=frames.borrow();intermediate(&f[2],&f[3],&end);
            assert!(difference(&f[0],&end)<100,"focus failed to restore the original pixels");
            drop(f); w.set_lyrics_reduced_motion(true);
        } else if p==5 && elapsed>=250 {toggle(&w,false);}
        else if p==6 && elapsed>=60 {frames.borrow_mut().push(capture(&w,"reduced-immediate"));}
        else if p==7 && elapsed>=400 {
            let end=capture(&w,"reduced-settled");let f=frames.borrow();
            assert_eq!(difference(f.last().unwrap(),&end),0,"reduced motion still animated focus");
            println!("native focus handoff: smooth outgoing/incoming pixels, no additive duplicate, exact restoration and reduced motion passed");
            drop(f);frames.borrow_mut().clear();
            let mut style=w.get_lyric_style();style.inactive_blur=0.;w.set_lyric_style(style);
            w.set_lyrics_reduced_motion(false);w.set_position(5.);
        } else if (8..=12).contains(&p) && elapsed>=600 {
            frames.borrow_mut().push(capture(&w,&format!("fade-{}",p-8)));
            if p<12 {w.set_position(5.+(p-7) as f32*0.125);}
            else {
                let f=frames.borrow();let total=difference(&f[0],&f[4]) as f64;
                assert!(total>10000.);
                for (i,expected) in [(1,0.15625),(2,0.5),(3,0.84375)] {
                    let ratio=difference(&f[0],&f[i]) as f64/total;
                    assert!((ratio-expected).abs()<0.06,"highlight fade did not follow its smooth curve: {i}: {ratio}");
                }
                println!("native unhighlight: smooth 500ms line fade verified at 125/250/375ms");
                phase.set(13);slint::quit_event_loop().unwrap();return;
            }
        } else {return;}
        phase.set(p+1);since.set(Instant::now());
    });
    w.run()?;
    assert_eq!(done.get(), 13);
    Ok(())
}
