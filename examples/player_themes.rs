//! Isolated whole-player visual/behavior fixture; never loads application state.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{
    services::Settings, theme_presets, themes, I18n, LyricRow, MainWindow, PlayerTheme, Tokens,
    TrackRow, WordRow,
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};
fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}
fn capture(w: &MainWindow, out: &std::path::Path, name: &str) {
    let frame = w.window().take_snapshot().unwrap();
    image::save_buffer(
        out.join(format!("{name}.png")),
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
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: player_themes NEW_OUTPUT_DIRECTORY [--mobile]")?,
    );
    if out.exists() {
        return Err("Choose a new output directory".into());
    }
    std::fs::create_dir(&out)?;
    let mobile = std::env::args().any(|a| a == "--mobile");
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    w.global::<I18n>().set_language("en".into());
    w.window().set_size(slint::LogicalSize::new(
        if mobile { 360. } else { 1440. },
        if mobile { 780. } else { 900. },
    ));
    playervf::player_themes::install(&w);
    playervf::native_lyrics::install(&w);
    playervf::lyric_appearance::install(&w);
    let _ambient = playervf::ambient_native::install(&w);
    let art = image::RgbaImage::from_fn(96, 96, |x, y| {
        image::Rgba([
            (42 + x * 2) as u8,
            (30 + y * 2) as u8,
            (160 + x / 2) as u8,
            255,
        ])
    });
    let cover = slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
        art.as_raw(),
        96,
        96,
    ));
    w.set_current_art(cover.clone());
    w.set_current_title("Colors in motion".into());
    w.set_current_artist("Vey theme fixture".into());
    w.set_duration(20.);
    w.set_time_label("0:02 / 0:20".into());
    w.set_position(2.5);
    w.set_player_state("paused".into());
    w.set_player_tab("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    let tracks = model(
        (0..5)
            .map(|i| TrackRow {
                title: format!("Track {} · theme sample", i + 1).into(),
                artist: "Vey".into(),
                album: "Color studies".into(),
                art: cover.clone(),
                path: format!("fixture://{i}").into(),
                duration: "3:24".into(),
                ..Default::default()
            })
            .collect(),
    );
    w.set_tracks(tracks.clone());
    w.set_home_page_tracks(tracks.clone());
    w.set_queue_page_tracks(tracks.clone());
    w.set_library_page_tracks(tracks);
    w.set_pages_seeded(true);
    w.set_lyric_lines(model(vec![
        LyricRow {
            text: "Feel the music".into(),
            start_ms: 0,
            end_ms: 8000,
            words: model(vec![
                WordRow {
                    text: "Feel ".into(),
                    start_ms: 0,
                    end_ms: 4000,
                    explicit: true,
                    ..Default::default()
                },
                WordRow {
                    text: "the music".into(),
                    start_ms: 4000,
                    end_ms: 8000,
                    explicit: true,
                    ..Default::default()
                },
            ]),
            ..Default::default()
        },
        LyricRow {
            text: "Every color has a rhythm".into(),
            start_ms: 9000,
            end_ms: 14000,
            ..Default::default()
        },
    ]));
    let saved = Rc::new(RefCell::new(Settings {
        language: "en".into(),
        ..Default::default()
    }));
    themes::apply_settings(&w, &saved.borrow());
    let weak = w.as_weak();
    let store = saved.clone();
    w.on_apply_lyric_style(move || {
        let w = weak.upgrade().unwrap();
        if playervf::lyric_appearance::draft(&w).is_err() {
            return;
        }
        let mut s = store.borrow_mut();
        s.lyrics_theme_css = w.get_lyric_theme_source().into();
        s.lyrics_font_family = w.get_lyric_font_family().into();
        s.theme_entire_player = w.get_theme_entire_player_draft();
        themes::apply_settings(&w, &s);
        w.set_dialog("".into());
    });
    let tick = Rc::new(Cell::new(0usize));
    let done = tick.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(300),move||{
        let w=weak.upgrade().unwrap();if !native_window::ready(w.window()){return;}let n=tick.get();
        let index=n/6;let phase=n%6;
        if index<theme_presets::all().len(){let preset=&theme_presets::all()[index];
            if phase==0 {w.set_active_view("lyrics".into());w.invoke_select_lyric_preset(index as i32);w.invoke_apply_lyric_style();}
            if phase==4 {
                let p=w.get_player_theme_style();assert_eq!(p.enabled,index!=0);assert_eq!(w.global::<PlayerTheme>().get_enabled(),p.enabled);
                assert!((w.get_lyrics_position()-2500).abs()<=2,"theme must not change media time");
                if p.enabled {assert_eq!(w.global::<Tokens>().get_neutral_0(),p.background);assert_eq!(w.global::<Tokens>().get_brand_500(),p.primary);assert_eq!(w.global::<PlayerTheme>().get_artwork_corner(),p.artwork_corner);}
                capture(&w,&out,preset.id);
                if [3,11,15].contains(&index){w.set_dialog("lyric-style".into());}
            }
            if phase==5 {if w.get_dialog()=="lyric-style" {capture(&w,&out,&format!("{}-editor",preset.id));w.set_dialog("".into());}println!("PASS full player {}",preset.id);}
        }else{
            match index-theme_presets::all().len(){
                0=>{
                    if phase==0 {w.set_dialog("lyric-style".into());}
                    if phase==1 {let before=w.global::<Tokens>().get_brand_500();w.invoke_select_lyric_preset(5);assert_eq!(w.global::<Tokens>().get_brand_500(),before,"draft must not repaint the shell");w.invoke_adjust_lyric_style("corner".into(),8.);w.invoke_adjust_player_color("accent".into(),slint::Color::from_rgb_u8(255,180,100));}
                    if phase==3 {assert_eq!(w.get_player_theme_draft().corner,8.);w.invoke_apply_lyric_style();}
                    if phase==5 {assert_eq!(w.get_player_theme_style().corner,8.);assert_eq!(w.get_player_theme_style().primary,slint::Color::from_rgb_u8(255,180,100));capture(&w,&out,"custom-player");w.set_dialog("lyric-style".into());}
                }
                1=>{
                    if phase==1 {w.set_theme_entire_player_draft(false);w.invoke_apply_lyric_style();}
                    if phase==5 {assert!(!w.get_player_theme_style().enabled);assert!(!saved.borrow().theme_entire_player);assert!(!w.global::<PlayerTheme>().get_enabled());capture(&w,&out,"lyrics-only");
                        let restored:Settings=serde_json::from_str(&serde_json::to_string(&*saved.borrow()).unwrap()).unwrap();assert!(!restored.theme_entire_player);themes::apply_settings(&w,&restored);}
                }
                2=>{
                    if phase==0 {w.set_dialog("lyric-style".into());}
                    if phase==1 {w.set_theme_entire_player_draft(true);w.invoke_select_lyric_preset(15);w.invoke_apply_lyric_style();w.set_active_view("home".into());}
                    if phase==5 {capture(&w,&out,"material3-home");w.set_active_view("queue".into());}
                }
                3=>{if phase==4 {capture(&w,&out,"material3-queue");w.set_active_view("settings".into());w.set_settings_section("appearance".into());}}
                4=>{if phase==4 {capture(&w,&out,"material3-settings");println!("PASS draft isolation, custom roles, scope persistence and all-page theme");slint::quit_event_loop().unwrap();}}
                _=>{}
            }
        }
        tick.set(n+1);
    });
    w.run()?;
    assert!(done.get() >= (theme_presets::all().len() + 4) * 6 + 5);
    Ok(())
}
