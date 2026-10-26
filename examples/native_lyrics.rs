//! Real native Slint/Braccato fixture, independent of user state, WebView and audio hardware.
use playervf::{
    braccato,
    lyrics::{self, LyricsAlternative},
    MainWindow,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

fn set_lyrics(w: &MainWindow, text: &str) {
    let (lyrics, format, sync_type) = lyrics::parse_payload(text).unwrap();
    let mut alt = LyricsAlternative {
        provider: "native fixture".into(),
        format: format.into(),
        sync_type,
        lyrics,
    };
    alt.lyrics.lines[0].translation =
        "Нативний Braccato: справжній час складів і бек-вокалу".into();
    if alt.lyrics.lines[0].romanization.is_empty() {
        alt.lyrics.lines[0].romanization = "Single word, an echo".into();
    }
    let rows = braccato::rows(&alt, 16000);
    w.set_lyrics_timed(braccato::effective_sync_type(&alt) != lyrics::LyricsSyncType::Plain);
    w.set_lyrics_word_timed(rows.iter().any(|r| r.words.row_count() > 0));
    w.set_lyric_lines(ModelRc::new(Rc::new(VecModel::from(rows))));
}
fn save(w: &MainWindow, name: &str) -> Vec<u8> {
    let frame = w.window().take_snapshot().unwrap();
    let dir = std::path::Path::new("target/native-lyrics-check");
    std::fs::create_dir_all(dir).unwrap();
    image::save_buffer(
        dir.join(format!("{name}.png")),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
    frame.as_bytes().to_vec()
}
const RICH:&str="<tt xmlns:ttm='http://www.w3.org/ns/ttml#metadata'><body><p begin='1s' end='6s'><span begin='1s' end='1.5s'>Sin</span><span begin='1.5s' end='2s'>gle </span><span begin='2s' end='3s'>word, </span><span begin='3s' end='4s'>an </span><span begin='4s' end='5.5s'>echo</span><span ttm:role='x-bg' begin='2.2s' end='4s'>(echo)</span></p><p begin='4.5s' end='8s'>Overlapping voice stays visible</p><p begin='9s' end='12s'>A later line with room to scroll</p></body></tt>";
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        std::process::exit(1);
    }));
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    playervf::native_lyrics::install(&w);
    w.set_lyrics_reduced_motion(true);
    w.set_current_title("Rust Braccato".into());
    w.set_current_artist("Native Slint fixture".into());
    w.set_duration(16.);
    w.set_position(2.6);
    w.set_active_view("lyrics".into());
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    set_lyrics(&w, RICH);
    let seek = Rc::new(Cell::new(-1));
    let captured = seek.clone();
    w.on_seek_to(move |ms| captured.set(ms));
    let index = Rc::new(Cell::new(0));
    let completed = index.clone();
    let weak = w.as_weak();
    let stable = Rc::new(RefCell::new(Vec::new()));
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(900),move|| {
        let w=weak.upgrade().unwrap();
        let i=index.get();
        assert!(w.get_native_lyrics());
        assert!(!w.get_lyrics_web_ready() && !w.get_lyrics_web_required());
        match i {
            0=>{assert!(w.get_native_lyric_lines().row_count()>0); save(&w,"desktop-reduced"); w.set_lyrics_reduced_motion(false);}
            1=>{save(&w,"desktop-rich"); w.invoke_native_lyric_seek(0,1); assert_eq!(seek.get(),1500); w.set_lyrics_stage(true);}
            2=>{save(&w,"stage");w.set_lyrics_stage(false);w.set_position(5.);}
            3=>{
                let rows=w.get_native_lyric_lines();
                let on=(0..rows.row_count()).filter_map(|i|rows.row_data(i)).filter(|r|r.active && !r.secondary).map(|r|r.source_index).collect::<Vec<_>>();
                assert!(on.contains(&0) && on.contains(&1),"overlapping source lines remain active: {on:?}");
                save(&w,"overlap");w.set_mobile(true);w.window().set_size(slint::LogicalSize::new(390.,844.));w.set_position(2.6);
            }
            4=>{assert_eq!(w.get_native_active_line(),0,"reflow must refresh the visual anchor");
                assert!((w.get_native_content_y()+w.get_native_scroll_position()).abs()<1.,"native viewport must settle after reflow: y={} target={} extent={} height={}",w.get_native_content_y(),w.get_native_scroll_position(),w.get_native_content_height(),w.get_lyrics_viewport_height());
                assert!(w.get_native_content_height()>w.get_lyrics_viewport_height());
                save(&w,"mobile-rich");set_lyrics(&w,"[00:01.00]This line has only line timestamps and must wrap correctly\n[00:05.00]A second line");}
            5=>{
                let rows=w.get_native_lyric_lines();
                for i in 0..rows.row_count() {let r=rows.row_data(i).unwrap();assert_eq!(r.parts.row_count(),0,"no fabricated word cue");}
                save(&w,"mobile-line");w.set_lyrics_reduced_motion(true);
            }
            6=>{*stable.borrow_mut()=save(&w,"paused-reduced");}
            7=>{assert_eq!(*stable.borrow(),save(&w,"paused-reduced-stable"),"paused reduced-motion frame is stable");
                w.invoke_lyric_user_scroll();}
            8=>{assert!(w.get_lyrics_user_scrolling());w.invoke_lyric_resume();}
            9=>{assert!(!w.get_lyrics_user_scrolling()); w.set_lyrics_offset(150);w.set_lyrics_line_offset_ms(-50);}
            10=>{assert!((w.get_lyrics_position()-2700).abs()<=1);w.invoke_native_lyric_seek(0,-1);assert_eq!(seek.get(),900);
                w.set_lyrics_offset(0);w.set_lyrics_line_offset_ms(0);
                set_lyrics(&w,"[00:08.00]After an instrumental break");}
            11=>{
                assert!(w.get_native_instrumental().silhouette.size().width>0);
                let rows=w.get_native_lyric_lines();assert!((0..rows.row_count()).any(|i|rows.row_data(i).unwrap().instrumental));
                save(&w,"instrumental");set_lyrics(&w,"Plain first line\nPlain second line");}
            12=>{
                assert_eq!(w.get_native_active_line(),-1);save(&w,"plain");
                set_lyrics(&w,r#"{"lyrics":[{"words":"你好","startTimeMs":1000,"durationMs":3000,"parts":[{"words":"你","startTimeMs":1000,"durationMs":500},{"words":"好","startTimeMs":1500,"durationMs":1000}],"romanization":"nihao","timedRomanization":[{"words":"ni","startTimeMs":1100,"durationMs":400},{"words":"hao","startTimeMs":1600,"durationMs":900}]}]}"#);}
            13=>{
                w.invoke_native_lyric_seek(0,-3);assert_eq!(seek.get(),1600,"romanization uses its own cue");save(&w,"romanization");
                println!("native Braccato: 14 phases passed; native snapshots in target/native-lyrics-check");slint::quit_event_loop().unwrap();}
            _=>unreachable!(),
        }
        index.set(i+1);
    });
    w.run()?;
    assert_eq!(completed.get(), 14);
    Ok(())
}
