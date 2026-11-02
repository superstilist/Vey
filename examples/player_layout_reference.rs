//! Isolated snapshots of the player layouts; never loads user state or audio.
#[path = "support/native_window.rs"]
mod native_window;

use playervf::{
    braccato,
    lyrics::{self, LyricsAlternative, LyricsSyncType},
    MainWindow,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::Cell, path::PathBuf, rc::Rc, time::Duration};

fn save(window: &MainWindow, output: &std::path::Path, name: &str) {
    let frame = window.window().take_snapshot().expect("capture player");
    image::save_buffer(
        output.join(format!("{name}.png")),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .expect("write player capture");
    println!("Captured {name}");
}

fn click(window: &MainWindow, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        hook(info);
        std::process::exit(1);
    }));
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: player_layout_reference NEW_OUTPUT_DIRECTORY")?,
    );
    // create_dir fails if the directory already exists; captures never replace
    // an earlier run or tracked screenshots.
    std::fs::create_dir(&output)?;

    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    playervf::player_themes::install(&w);
    playervf::native_lyrics::install(&w);
    let _ambient = playervf::ambient_native::install(&w);

    // A generated fixture cover avoids any personal or copyrighted artwork.
    let art = image::RgbaImage::from_fn(384, 384, |x, y| {
        let dx = x as f32 - 192.;
        let dy = y as f32 - 192.;
        let circle = (dx * dx + dy * dy).sqrt() < 125.;
        if circle {
            image::Rgba([55 + (x / 6) as u8, 52 + (y / 8) as u8, 85, 255])
        } else {
            image::Rgba([12, 13, 20, 255])
        }
    });
    w.set_current_art(slint::Image::from_rgba8(
        slint::SharedPixelBuffer::clone_from_slice(art.as_raw(), 384, 384),
    ));
    w.set_current_title("Тиша між нотами".into());
    w.set_current_artist("Vey · приклад".into());
    w.set_current_album("Вечірні історії".into());
    w.set_player_state("paused".into());
    w.set_duration(190.);
    w.set_position(7.2);
    w.set_time_label("0:07 / 3:10".into());
    w.set_player_tab("lyrics".into());
    w.set_lyrics_reduced_motion(true);

    let (lines, format, sync_type) = lyrics::parse_payload(
        "[00:01.00]Вечір повільно лягає на місто\n\
         [00:06.00]Музика світиться у тиші\n\
         [00:11.00]Ми залишаємося поруч\n\
         [00:16.00]І слухаємо до світанку\n\
         [00:21.00]Нове звучання починається тут",
    )
    .ok_or("Fixture lyrics did not parse")?;
    let alternative = LyricsAlternative {
        provider: "Fixture".into(),
        format: format.into(),
        sync_type,
        lyrics: lines,
    };
    let rows = braccato::rows(&alternative, 190_000);
    w.set_lyrics_timed(braccato::effective_sync_type(&alternative) != LyricsSyncType::Plain);
    w.set_lyrics_word_timed(rows.iter().any(|row| row.words.row_count() > 0));
    w.set_lyric_lines(ModelRc::new(Rc::new(VecModel::from(rows))));
    w.set_active_view("lyrics".into());
    w.window().set_size(slint::LogicalSize::new(1440., 900.));

    let phase = Rc::new(Cell::new(0));
    let completed = phase.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(1200),
        move || {
            let Some(window) = weak.upgrade() else { return };
            if !native_window::ready(window.window()) {
                return;
            }
            match phase.get() {
                0 => {
                    save(&window, &output, "desktop-standard");
                    click(&window, 1328., 100.);
                    assert!(
                        window.get_expanded_player(),
                        "expand button must expand the player"
                    );
                }
                1 => {
                    save(&window, &output, "desktop-expanded");
                    click(&window, 1358., 38.);
                    assert!(
                        window.get_focus_player(),
                        "focus button must enter focus mode"
                    );
                }
                2 => {
                    save(&window, &output, "desktop-focus");
                    click(&window, 1358., 38.);
                    assert!(
                        !window.get_focus_player(),
                        "focus button must leave focus mode"
                    );
                    assert!(
                        !window.get_expanded_player(),
                        "focus exit must restore standard mode"
                    );
                }
                3 => {
                    save(&window, &output, "desktop-return");
                    window.set_mobile(true);
                    window
                        .window()
                        .set_size(slint::LogicalSize::new(390., 844.));
                }
                4 => {
                    save(&window, &output, "mobile-player");
                    println!("PASS pointer navigation: standard -> expanded -> focus -> standard");
                    slint::quit_event_loop().expect("quit fixture");
                }
                _ => unreachable!(),
            }
            phase.set(phase.get() + 1);
        },
    );
    w.run()?;
    if completed.get() != 5 {
        return Err("Player snapshot fixture ended early".into());
    }
    Ok(())
}
