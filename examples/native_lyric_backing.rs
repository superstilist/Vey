//! Real native pixel regression for one fill edge across a multiword backing cue.
use playervf::{BraccatoWord, LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

const CASES: [(&str, bool, bool); 4] = [
    ("phrase", false, false),
    ("wrapped", true, false),
    ("wrapped-wave", true, true),
    ("phrase-wave", false, true),
];

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}

fn fixture(w: &MainWindow, wrapped: bool, wave: bool) {
    let mut style = w.get_lyric_style();
    style.glow = false;
    style.fade = false;
    style.wave = wave;
    style.wobble = false;
    style.inactive_blur = 0.;
    style.active_scale = 1.;
    style.inactive_scale = 1.;
    w.set_lyric_style(style);
    let backing = if wrapped {
        "(куди йти, коли дорога кличе нас додому)"
    } else {
        "(куди йти…)"
    };
    w.set_lyric_lines(model(vec![LyricRow {
        text: "Знов не знаю, куди йти".into(),
        start_ms: 0,
        end_ms: 10000,
        words: model(vec![
            // This lead cue is already sung in every capture. Only the backing fill changes.
            WordRow {
                text: "Знов не знаю, куди йти".into(),
                ..Default::default()
            },
            WordRow {
                text: backing.into(),
                start_ms: 1000,
                end_ms: 5000,
                background: true,
                ..Default::default()
            },
        ]),
        ..Default::default()
    }]));
    w.set_position(0.5);
}

fn backing(w: &MainWindow) -> Vec<BraccatoWord> {
    let rows = w.get_native_lyric_lines();
    (0..rows.row_count())
        .filter_map(|i| rows.row_data(i))
        .flat_map(|r| {
            (0..r.parts.row_count())
                .filter_map(|i| r.parts.row_data(i))
                .collect::<Vec<_>>()
        })
        .filter(|p| p.background)
        .collect()
}

fn capture(w: &MainWindow, name: &str) -> image::RgbaImage {
    let frame = w.window().take_snapshot().unwrap();
    let frame =
        image::RgbaImage::from_raw(frame.width(), frame.height(), frame.as_bytes().to_vec())
            .unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    frame
        .save(format!("target/native-lyrics-check/backing-{name}.png"))
        .unwrap();
    frame
}

fn verify(off: &image::RgbaImage, mid: &image::RgbaImage, full: &image::RgbaImage, wrapped: bool) {
    // Find sung backing glyph cores by comparing the before/after frames, away
    // from the transport. Lead text and row geometry remain fixed throughout.
    let ink = (full.height() / 6..full.height() * 7 / 10)
        .flat_map(|y| (20..full.width() - 20).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let a = off.get_pixel(x, y);
            let b = full.get_pixel(x, y);
            (0..3).all(|c| b[c] > 215 && b[c] as i16 - a[c] as i16 > 70)
        })
        .collect::<Vec<_>>();
    assert!(ink.len() > 100, "must locate backing glyphs: {}", ink.len());
    let left = ink.iter().map(|p| p.0).min().unwrap() as f32;
    let right = ink.iter().map(|p| p.0).max().unwrap() as f32;
    let top = ink.iter().map(|p| p.1).min().unwrap() as f32;
    let bottom = ink.iter().map(|p| p.1).max().unwrap() as f32;
    let mut early = 0;
    let mut lit = 0;
    let mut tail = 0;
    let mut prematurely_lit = 0;
    for (x, y) in ink {
        let a = off.get_pixel(x, y);
        let b = mid.get_pixel(x, y);
        if x as f32 <= left + (right - left) * 0.2
            && (!wrapped || (y as f32) < top + (bottom - top) * 0.4)
        {
            early += 1;
            if (0..3).all(|c| b[c] as i16 - a[c] as i16 > 70) {
                lit += 1;
            }
        }
        if if wrapped {
            (y as f32) > top + (bottom - top) * 0.65
        } else {
            (x as f32) > left + (right - left) * 0.65
        } {
            tail += 1;
            if (0..3).any(|c| (b[c] as i16 - a[c] as i16).abs() > 3) {
                prematurely_lit += 1;
            }
        }
    }
    assert!(
        early > 10 && lit * 10 >= early * 9,
        "first part should be filled: {lit}/{early}"
    );
    assert!(tail > 30, "must sample the later word/row: {tail}");
    assert!(
        prematurely_lit <= 2,
        "fill restarted in later word/row: {prematurely_lit}/{tail}"
    );
    println!("backing fill pixels: early {lit}/{early}, premature tail {prematurely_lit}/{tail}");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        hook(info);
        std::process::exit(1);
    }));
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_mobile(true);
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_duration(10.);
    w.window().set_size(slint::LogicalSize::new(390., 844.));
    fixture(&w, false, false);
    let step = Rc::new(Cell::new(0));
    let completed = step.clone();
    let frames = Rc::new(RefCell::new(Vec::new()));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(900), move || {
        let w = weak.upgrade().unwrap(); let i = step.get(); let case = i / 3; let phase = i % 3;
        let (label, wrapped, wave) = CASES[case];
        let parts = backing(&w);
        assert_eq!(parts.len() > 1, wrapped, "only an actual wrap splits a cue");
        if wave {assert!(parts.iter().all(|p|p.glyphs.row_count()>0), "wave plans must be loaded");}
        if wrapped {assert!(parts[1].cue_offset > 0.4, "later row must be beyond the mid-frame fill");}
        frames.borrow_mut().push(capture(&w, &format!("{label}-{phase}")));
        if phase == 0 {w.set_position(2.6);}
        else if phase == 1 {w.set_position(5.8);}
        else {
            let mut f = frames.borrow_mut(); verify(&f[0], &f[1], &f[2], wrapped); f.clear();
            if case + 1 == CASES.len() {
                println!("native backing fill: phrase, mobile wrapping and opt-in letter path passed");
                slint::quit_event_loop().unwrap();
            } else {fixture(&w, CASES[case+1].1, CASES[case+1].2);}
        }
        step.set(i+1);
    });
    w.run()?;
    assert_eq!(completed.get(), CASES.len() * 3);
    Ok(())
}
