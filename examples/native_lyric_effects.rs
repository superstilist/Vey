//! Compare real native focus/sweep/typography against the supplied Braccato references.
use playervf::{
    braccato,
    lyrics::{self, LyricsAlternative},
    MainWindow,
};
use slint::{ComponentHandle, Model, VecModel};
use std::{cell::Cell, rc::Rc, time::Duration};
fn document(w: &MainWindow, text: &str) {
    let (lyrics, format, sync_type) = lyrics::parse_payload(text).unwrap();
    let alt = LyricsAlternative {
        lyrics,
        format: format.into(),
        sync_type,
        provider: "visual fixture".into(),
    };
    let rows = braccato::rows(&alt, 18000);
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(rows.iter().any(|r| r.words.row_count() > 0));
    w.set_lyric_lines(Rc::new(VecModel::from(rows)).into());
}
fn save(w: &MainWindow, name: &str) {
    let frame = w.window().take_snapshot().unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    image::save_buffer(
        format!("target/native-lyrics-check/effects-{name}.png"),
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
        std::process::exit(1)
    }));
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_position(2.6);
    w.set_duration(18.);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    document(&w,"[00:01.00]There's a snake in the garden, so now everyone's started to wonder\n[00:06.00]You're just making it harder if you think you can trust one another\n[00:11.00]The words stay clear while the other lines drift out of focus");
    let tick = Rc::new(Cell::new(0));
    let count = tick.clone();
    let attempts = Rc::new(Cell::new(0));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(2200),move||{
        let w=weak.upgrade().unwrap();let n=tick.get();
        if n==0 {
            let rows=w.get_native_lyric_lines();
            if !(0..rows.row_count()).filter_map(|i|rows.row_data(i)).any(|r|r.sustain_distance>0 && r.sustain_blurred.size().width>0) {
                attempts.set(attempts.get()+1);assert!(attempts.get()<10,"inactive paint did not become ready");return;
            }
            save(&w,"english");
            document(&w,"<tt><body><p begin='1s' end='5s'><span begin='1s' end='1.4s'>Ще </span><span begin='1.4s' end='1.7s'>й </span><span begin='1.7s' end='3.4s'>забулися </span><span begin='3.4s' end='5s'>слова</span></p><p begin='5s' end='8s'>Повтікали по лісам</p><p begin='8s' end='11s'>Залишили болю слід</p><p begin='11s' end='15s'>Змарнували цілий світ</p></body></tt>");
        } else if n==1 {save(&w,"ukrainian");w.set_position(6.1);}
        else if n==2 {save(&w,"handoff");w.set_lyrics_reduced_motion(true);w.set_mobile(true);w.window().set_size(slint::LogicalSize::new(390.,844.));}
        else if n==3 {save(&w,"mobile-reduced");w.set_lyrics_reduced_motion(false);w.set_mobile(false);w.window().set_size(slint::LogicalSize::new(1440.,900.));w.set_position(1.85);}
        else if n==4 {save(&w,"glow-attack");w.set_position(2.3);}
        else if n==5 {save(&w,"glow-contract");w.set_position(3.2);}
        else if n==6 {save(&w,"glow-settle");println!("native effects: focus, Unicode spacing, handoff, mobile and shrinking glow frames passed");slint::quit_event_loop().unwrap();}
        tick.set(n+1);
    });
    w.run()?;
    assert_eq!(count.get(), 7);
    Ok(())
}
