//! Native overflow regression matching the long hyphenated chant in the report.
use playervf::{LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::Cell, rc::Rc, time::Duration};
fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}
fn fixture(w: &MainWindow, text: &str, rich: bool) {
    w.set_lyrics_word_timed(rich);
    w.set_position(1.05);
    w.set_lyric_lines(model(vec![
        LyricRow {
            text: text.into(),
            start_ms: 1000,
            end_ms: 9000,
            words: if rich {
                model(vec![WordRow {
                    text: text.into(),
                    start_ms: 1000,
                    end_ms: 9000,
                    ..Default::default()
                }])
            } else {
                ModelRc::default()
            },
            ..Default::default()
        },
        LyricRow {
            text: "Feel good".into(),
            start_ms: 10000,
            end_ms: 12000,
            ..Default::default()
        },
    ]));
}
fn check(w: &MainWindow, text: &str) {
    let rows = w.get_native_lyric_lines();
    let size = if w.get_mobile() { 32. } else { 48. };
    let limit = (w.get_lyrics_widget_width()
        - if w.get_lyrics_stage() || size >= 40. {
            80.
        } else {
            56.
        })
    .max(80.);
    let mut joined = String::new();
    let mut count = 0;
    for i in 0..rows.row_count() {
        let r = rows.row_data(i).unwrap();
        if r.source_index != 0 {
            continue;
        }
        let width = playervf::native_lyric_fonts::measure_with(
            &r.text,
            size,
            &w.get_lyric_style().family,
            700,
        );
        assert!(
            width <= limit + 0.5,
            "row {i} overflows: {width} > {limit}: {}",
            r.text
        );
        joined.push_str(&r.text);
        count += 1;
        for n in 0..r.parts.row_count() {
            let p = r.parts.row_data(n).unwrap();
            assert_eq!(
                (p.start_ms, p.duration_ms, p.source_part),
                (1000., 8000., 0)
            );
        }
    }
    assert_eq!(joined, text);
    assert!(count > 1, "fixture must actually wrap");
}
fn capture(w: &MainWindow, name: &str) {
    let frame = w.window().take_snapshot().unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    image::save_buffer(
        format!("target/native-lyrics-check/overflow-{name}.png"),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
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
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_duration(12.);
    w.set_current_title("Wrap and flow".into());
    w.set_current_artist("Native regression fixture".into());
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let chant = format!("{}Ha", "Ha-".repeat(40));
    let unbroken = "Supercalifragilisticexpialidocious".repeat(6);
    fixture(&w, &chant, false);
    let step = Rc::new(Cell::new(0));
    let completed = step.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(1000),move||{
        let w=weak.upgrade().unwrap();let i=step.get();
        if i<5 {check(&w,&chant);}else{check(&w,&unbroken);}
        match i{
            0=>{capture(&w,"desktop-line");w.set_mobile(true);w.window().set_size(slint::LogicalSize::new(390.,844.));}
            1=>{capture(&w,"mobile-line");w.set_mobile(false);w.window().set_size(slint::LogicalSize::new(1440.,900.));fixture(&w,&chant,true);}
            2=>{assert_eq!(w.get_native_active_line(),0,"one long cue must begin on its first row");capture(&w,"flow-start");w.set_position(3.4);}
            3=>{assert!(w.get_native_active_line()>0,"follow the fill into later rows");capture(&w,"flow-mid");w.set_lyrics_stage(true);}
            4=>{capture(&w,"stage");w.set_lyrics_stage(false);fixture(&w,&unbroken,true);}
            5=>{assert_eq!(w.get_native_active_line(),0);capture(&w,"unbroken-start");w.set_position(6.4);}
            6=>{assert!(w.get_native_active_line()>0);capture(&w,"unbroken-follow");
                println!("native overflow: desktop/mobile/stage chant, continuous cue fill and unbroken-token follow passed");slint::quit_event_loop().unwrap();}
            _=>unreachable!(),
        }
        step.set(i+1);
    });
    w.run()?;
    assert_eq!(completed.get(), 7);
    Ok(())
}
