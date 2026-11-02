//! Offline preset selection, persistence and real native lyric/chooser captures.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{services::Settings, theme_presets, themes, I18n, LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, VecModel};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

fn capture(w: &MainWindow, output: &std::path::Path, name: &str) {
    let frame = w.window().take_snapshot().unwrap();
    image::save_buffer(
        output.join(format!("{name}.png")),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: lyric_themes NEW_OUTPUT_DIRECTORY [--mobile]")?,
    );
    if output.exists() {
        return Err("Choose a new output directory".into());
    }
    std::fs::create_dir(&output)?;
    let mobile = std::env::args().any(|a| a == "--mobile");
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    w.global::<I18n>().set_language("en".into());
    w.set_active_view("lyrics".into());
    w.set_player_tab("lyrics".into());
    w.set_current_title("Theme preview".into());
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_duration(10.);
    w.set_position(2.5);
    w.set_player_state("paused".into());
    w.set_lyric_font_import_supported(true);
    w.window().set_size(slint::LogicalSize::new(
        if mobile { 360. } else { 1440. },
        if mobile { 800. } else { 900. },
    ));
    let lines = vec![
        LyricRow {
            text: "Colors follow every word".into(),
            start_ms: 1000,
            end_ms: 9000,
            words: Rc::new(VecModel::from(vec![
                WordRow {
                    text: "Colors ".into(),
                    start_ms: 1000,
                    end_ms: 5000,
                    explicit: true,
                    ..Default::default()
                },
                WordRow {
                    text: "follow every word".into(),
                    start_ms: 5000,
                    end_ms: 9000,
                    explicit: true,
                    ..Default::default()
                },
            ]))
            .into(),
            ..Default::default()
        },
        LyricRow {
            text: "A quiet line waits ahead".into(),
            start_ms: 10000,
            end_ms: 14000,
            ..Default::default()
        },
    ];
    w.set_lyric_lines(Rc::new(VecModel::from(lines)).into());
    playervf::native_lyrics::install(&w);
    playervf::player_themes::install(&w);
    let saved = Rc::new(RefCell::new(Settings {
        language: "en".into(),
        ..Default::default()
    }));
    themes::apply_settings(&w, &saved.borrow());
    playervf::lyric_appearance::install(&w);
    let apply_saved = saved.clone();
    let apply_weak = w.as_weak();
    w.on_apply_lyric_style(move || {
        let w = apply_weak.upgrade().unwrap();
        let mut s = apply_saved.borrow_mut();
        s.lyrics_theme_css = w.get_lyric_theme_source().to_string();
        s.lyrics_font_family = w.get_lyric_font_family().to_string();
        s.theme_entire_player = w.get_theme_entire_player_draft();
        themes::apply_settings(&w, &s);
    });
    assert_eq!(
        w.get_lyric_theme_names().row_count(),
        theme_presets::all().len() + 1
    );
    let manifest=theme_presets::all().iter().map(|p|{let t=themes::convert(p.css);serde_json::json!({"id":p.id,"name":p.name,"css":p.css,"reference":p.reference,"family":t.appearance.family,"active":format!("#{:06x}",t.active&0xffffff),"wave":t.appearance.wave})}).collect::<Vec<_>>();
    std::fs::write(
        output.join("catalog.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let state = Rc::new(RefCell::new((0usize, 0usize, 0usize)));
    let done = state.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(400),
        move || {
            let w = weak.upgrade().unwrap();
            if !native_window::ready(w.window()) {
                return;
            }
            let mut s = state.borrow_mut();
            if s.2 < 3 {
                s.2 += 1;
                return;
            }
            let p = &theme_presets::all()[s.0];
            if s.1 == 0 {
                w.invoke_select_lyric_preset(s.0 as i32);
                assert_eq!(w.get_lyric_theme_source().as_str(), p.css);
                w.invoke_apply_lyric_style();
                assert_eq!(w.get_lyric_preset_index(), s.0 as i32);
                assert!(
                    w.get_lyric_style_status().is_empty(),
                    "{}: {}",
                    p.id,
                    w.get_lyric_style_status()
                );
                assert_eq!(saved.borrow().lyrics_theme_css, p.css);
                s.1 = 1;
                return;
            }
            if s.1 == 1 {
                assert!(
                    (w.get_lyrics_position() - 2500).abs() <= 2,
                    "theme changed media timing"
                );
                capture(&w, &output, p.id);
                if [3, 5, 9, 15].contains(&s.0) {
                    w.set_dialog("lyric-style".into());
                    s.1 = 2;
                    return;
                }
            }
            if s.1 == 2 {
                capture(&w, &output, &format!("{}-picker", p.id));
                w.set_dialog("".into());
            }
            println!("PASS preset {}", p.id);
            s.0 += 1;
            s.1 = 0;
            if s.0 == theme_presets::all().len() {
                let css = ".blyrics-container { --blyrics-lyric-active-color:#bada55; }";
                w.set_lyric_theme_source(css.into());
                w.invoke_apply_lyric_style();
                assert_eq!(
                    w.get_lyric_preset_index(),
                    theme_presets::all().len() as i32
                );
                let restored: Settings =
                    serde_json::from_str(&serde_json::to_string(&*saved.borrow()).unwrap())
                        .unwrap();
                themes::apply_settings(&w, &restored);
                assert_eq!(w.get_lyric_theme_source().as_str(), css);
                println!("PASS custom CSS restart restoration");
                w.invoke_select_lyric_preset(0);
                w.invoke_apply_lyric_style();
                assert!(w.get_lyric_theme_source().is_empty());
                assert_eq!(w.get_lyric_preset_index(), 0);
                assert_eq!(
                    w.get_lyric_active(),
                    slint::Color::from_argb_encoded(themes::NativeTheme::default().active)
                );
                println!("PASS reset to Braccato");
                slint::quit_event_loop().unwrap();
            }
        },
    );
    w.run()?;
    assert_eq!(done.borrow().0, theme_presets::all().len());
    Ok(())
}
