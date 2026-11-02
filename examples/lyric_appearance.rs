//! Real appearance-dialog input, draft isolation, diagnostics and native captures.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{lyric_appearance, services::Settings, theme_presets, themes, I18n, MainWindow};
use slint::{ComponentHandle, Model};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

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
fn click(w: &MainWindow, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    w.window()
        .dispatch_event(slint::platform::WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
    w.window()
        .dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1)
    }));
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: lyric_appearance NEW_OUTPUT_DIRECTORY [--mobile] [--uk]")?,
    );
    if output.exists() {
        return Err("Choose a new output directory".into());
    }
    std::fs::create_dir(&output)?;
    let mobile = std::env::args().any(|a| a == "--mobile");
    let language = if std::env::args().any(|a| a == "--uk") {
        "uk"
    } else {
        "en"
    };
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    w.global::<I18n>().set_language(language.into());
    playervf::native_lyrics::install(&w);
    playervf::player_themes::install(&w);
    lyric_appearance::install(&w);
    w.window().set_size(slint::LogicalSize::new(
        if mobile { 360. } else { 1440. },
        if mobile { 780. } else { 900. },
    ));
    let saved = Rc::new(RefCell::new(Settings {
        language: language.into(),
        ..Default::default()
    }));
    themes::apply_settings(&w, &saved.borrow());
    let apply_weak = w.as_weak();
    let apply_saved = saved.clone();
    w.on_apply_lyric_style(move || {
        let w = apply_weak.upgrade().unwrap();
        if let Err(error) = lyric_appearance::draft(&w) {
            w.set_lyric_style_status(error.into());
            w.set_lyric_draft_valid(false);
            return;
        }
        let mut saved = apply_saved.borrow_mut();
        saved.lyrics_theme_css = w.get_lyric_theme_source().into();
        saved.lyrics_font_family = w.get_lyric_font_family().into();
        saved.theme_entire_player = w.get_theme_entire_player_draft();
        themes::apply_settings(&w, &saved);
        w.set_dialog("".into());
    });
    let weak = w.as_weak();
    let step = Rc::new(Cell::new(0));
    let done = step.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(500),move || {
        let w=weak.upgrade().unwrap();
        if !native_window::ready(w.window()) {return;}
        let s=step.get();
        let size=w.window().size().to_logical(w.window().scale_factor());
        let panel_width=(size.width-32.).min(640.);
        let right=(size.width+panel_width)/2.;
        let footer_y=(size.height+(size.height-32.).min(840.))/2.-38.;
        match s {
            0 => {w.set_dialog("lyric-style".into());}
            2 => {
                capture(&w,&output,"default");
                // The arrows change only the draft, and can return to the first preset.
                let arrow_y = if mobile { 136. } else { 158. };
                let inset = if mobile { 38. } else { 46. };
                click(&w, right-inset, arrow_y);
                assert_eq!(w.get_lyric_preset_index(), 1, "next theme button");
                assert!(saved.borrow().lyrics_theme_css.is_empty());
                click(&w, (size.width-panel_width)/2.+inset, arrow_y);
                assert_eq!(w.get_lyric_preset_index(), 0, "previous theme button");
                w.invoke_select_lyric_preset(5);
            }
            4 => {
                assert!(w.get_lyric_draft_style().wave);
                assert_eq!(saved.borrow().lyrics_theme_css, "");
                assert_eq!(w.get_lyric_active(),slint::Color::from_argb_encoded(themes::NativeTheme::default().active));
                let parts=w.get_lyric_preview_rows().row_data(0).unwrap().parts;
                assert!(parts.row_data(0).unwrap().glow_levels>0,"preview must prepare real glow");
                assert!(parts.row_data(0).unwrap().glyphs.row_count()>0,"preview must prepare shaped letter waves");
                capture(&w,&output,"ocean-draft");
                click(&w,right-150.,footer_y);
                assert!(w.get_dialog().is_empty(),"Cancel must be reachable");
                w.set_dialog("lyric-style".into());
            }
            5 => {
                assert!(w.get_lyric_theme_source().is_empty(),"Cancel restores saved CSS on reopen");
                assert_eq!(w.get_lyric_preset_index(),0,"Cancel restores the saved preset selection");
                w.invoke_select_lyric_preset(3);
                w.invoke_adjust_lyric_style("size".into(),36.);
                w.invoke_adjust_lyric_style("spacing".into(),1.6);
                w.set_lyric_font_family("Noto Sans".into());
            }
            7 => {
                assert_eq!(w.get_lyric_draft_size(),36.);
                assert_eq!(w.get_lyric_draft_style().line_spacing,1.6);
                assert_eq!(w.get_lyric_draft_style().family,"Noto Sans");
                capture(&w,&output,"custom-draft");
                click(&w,right-58.,footer_y);
                assert!(w.get_dialog().is_empty(),"Apply must be reachable");
                assert_eq!(w.get_lyric_size(),36.); assert_eq!(saved.borrow().lyrics_font_family,"Noto Sans");
            }
            8 => { w.set_dialog("lyric-style".into()); }
            9 => { w.set_lyric_theme_source("$ink: $undefined; :root{color:$ink;}".into()); }
            11 => {
                assert!(!w.get_lyric_draft_valid()); assert!(!w.get_lyric_style_status().is_empty());
                w.invoke_apply_lyric_style(); assert_eq!(w.get_dialog(),"lyric-style");
                assert_eq!(w.get_lyric_size(),36.,"invalid draft must preserve saved theme");
                capture(&w,&output,"invalid-draft");
                click(&w, (size.width-panel_width)/2.+54., footer_y);
                assert!(w.get_lyric_draft_valid()); assert!(w.get_lyric_font_family().is_empty());
                w.invoke_apply_lyric_style();
                assert!(saved.borrow().lyrics_theme_css.is_empty());
                assert!(saved.borrow().lyrics_font_family.is_empty());
                assert_eq!(w.get_lyric_size(),48.);
                assert_eq!(theme_presets::all().len(),20);
            }
            12 => { w.set_dialog("lyric-style".into()); }
            14 => {
                // Wheel input scrolls the settings body; the footer stays clickable.
                for _ in 0..6 {w.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {position:slint::LogicalPosition::new(right-40.,size.height/2.),delta_x:0.,delta_y:-180.});}
            }
            16 => {capture(&w,&output,"effects-and-footer"); click(&w,right-150.,footer_y); assert!(w.get_dialog().is_empty());}
            17 => {w.set_dialog("lyric-style".into());}
            18 => {
                w.window().dispatch_event(slint::platform::WindowEvent::KeyPressed {text:slint::platform::Key::Escape.into()});
                w.window().dispatch_event(slint::platform::WindowEvent::KeyReleased {text:slint::platform::Key::Escape.into()});
            }
            20 => {
                assert!(w.get_dialog().is_empty(),"Escape must cancel the dialog");
                println!("PASS real preview, draft isolation, Cancel, Apply, reset, invalid RICS, scroll/footer ({language}, mobile={mobile})");
                slint::quit_event_loop().unwrap();}
            _ => {}
        }
        step.set(s+1);
    });
    w.run()?;
    assert!(done.get() >= 21);
    Ok(())
}
