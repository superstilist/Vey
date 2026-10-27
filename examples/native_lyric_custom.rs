//! Native custom font/theme and fill-only glow regression, isolated from user data.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{
    braccato,
    lyrics::{self, LyricsAlternative},
    themes, MainWindow,
};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
fn capture(w: &MainWindow, name: &str) -> (Vec<u8>, u32, u32) {
    let frame = w.window().take_snapshot().unwrap();
    std::fs::create_dir_all("target/native-lyrics-check").unwrap();
    image::save_buffer(
        format!("target/native-lyrics-check/custom-{name}.png"),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
    (frame.as_bytes().to_vec(), frame.width(), frame.height())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1)
    }));
    let dir = std::env::current_dir()?.join("target/native-custom-font-check");
    std::env::set_var("PLAYERVF_DATA_DIR", &dir);
    let w = MainWindow::new()?;
    let family =
        playervf::native_lyric_fonts::import(std::path::Path::new("assets/fonts/Roboto.ttf"))?;
    assert_eq!(family, "Roboto");
    assert!(std::fs::read_dir(dir.join("lyric-fonts"))?.next().is_some());
    assert!(playervf::native_lyric_fonts::register(vec![0; 16]).is_err());
    playervf::i18n::install(&w);
    playervf::native_lyrics::install(&w);
    let css=":root {--blyrics-font-size:48px;--blyrics-glow-color:#66ccffff;--blyrics-highlight-glow-radius-from:18px;--blyrics-animate-letter-wave:0;--blyrics-animate-word-wobble:0;}";
    let settings = playervf::services::Settings {
        lyrics_theme_css: format!("{css} :root {{--blyrics-animate-highlight-glow:0;}}"),
        lyrics_font_family: family,
        ..Default::default()
    };
    themes::apply_settings(&w, &settings);
    w.set_lyric_font_import_supported(true);
    w.set_active_view("lyrics".into());
    w.set_position(2.4);
    w.set_duration(10.);
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let (lyrics,format,sync_type)=lyrics::parse_payload("<tt><body><p begin='1s' end='6s'><span begin='1s' end='5s'>Illuminate</span><span begin='5s' end='6s'> slowly</span></p><p begin='6s' end='9s'>Next line remains soft</p></body></tt>").unwrap();
    let alt = LyricsAlternative {
        lyrics,
        format: format.into(),
        sync_type,
        provider: "fixture".into(),
    };
    w.set_lyric_lines(Rc::new(VecModel::from(braccato::rows(&alt, 10000))).into());
    let tick = Rc::new(Cell::new(0));
    let done = tick.clone();
    let off = Rc::new(RefCell::new(None));
    let retries = Rc::new(Cell::new(0));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(900),move||{
        let w=weak.upgrade().unwrap();let phase=tick.get();
        if !native_window::ready(w.window()) { return; }
        if phase==0 {
            assert_eq!(w.get_native_lyric_lines().row_data(0).unwrap().font_family,"Roboto");
            assert!(w.get_native_lyric_lines().row_data(0).unwrap().parts.row_count()>0,"fixture needs real word timing");
            *off.borrow_mut()=Some(capture(&w,"glow-off"));
            let mut settings=settings.clone();settings.lyrics_theme_css=css.into();themes::apply_settings(&w,&settings);
        } else if phase==1 {
            let word=w.get_native_lyric_lines().row_data(0).unwrap().parts.row_data(0).unwrap();
            if word.glow_levels==0 {retries.set(retries.get()+1);assert!(retries.get()<12,"glow upload timed out");return;}
            let on=capture(&w,"filled-glow");let before=off.borrow();let before=before.as_ref().unwrap();
            let scale=w.window().scale_factor();
            // Locate the sharp sung ink in the actual frame. Native geometry
            // callbacks report local bounds, not the pane's window position.
            let mut left=on.1;let mut top=on.2;let mut bottom=0;
            for y in on.2/3..on.2*2/3 {for x in on.1/2..on.1 {
                let i=((y*on.1+x)*4) as usize;
                if before.0[i..i+3].iter().all(|v|*v>210) {left=left.min(x);top=top.min(y);bottom=bottom.max(y);}
            }}
            assert!(left<on.1 && bottom>top,"find sung ink in the frame");
            let left=left as f32;let cy=(top+bottom) as f32/2.;
            let width=playervf::native_lyric_fonts::measure_with("Illuminate",48.,"Roboto",700)*scale;
            let count=|from:f32,to:f32| {
                let mut changed=0;
                for y in ((cy-40.*scale).max(0.) as u32)..((cy+40.*scale) as u32).min(on.2) {
                    for x in ((left+width*from) as u32)..((left+width*to) as u32).min(on.1) {
                        let i=((y*on.1+x)*4) as usize;
                        if on.0[i..i+3]!=before.0[i..i+3] {changed+=1;}
                    }
                }changed
            };
            let lit=count(0.,0.3);let unsung=count(0.7,1.);
            assert!(lit>20,"filled text must gain glow, changed={lit}");
            assert_eq!(unsung,0,"unfilled letters must receive no glow");
            w.set_dialog("lyric-style".into());
        } else if phase==2 {
            capture(&w,"appearance-desktop");w.set_mobile(true);w.window().set_size(slint::LogicalSize::new(390.,844.));
        } else if phase==3 {
            capture(&w,"appearance-mobile");
            println!("custom lyrics: imported font, theme, filled-only glow and desktop/mobile appearance passed");
            slint::quit_event_loop().unwrap();
        }
        tick.set(phase+1);
    });
    w.run()?;
    assert_eq!(done.get(), 4);
    Ok(())
}
