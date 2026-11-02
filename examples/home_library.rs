//! Isolated Home/import/account-choice fixture. No real state or network.
use playervf::{
    services::{Settings, Store},
    I18n, MainWindow, PlaylistRow, TrackRow,
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: home_library NEW_OUTPUT_DIRECTORY [--mobile]")?,
    );
    if out.exists() {
        return Err("Choose a new output directory".into());
    }
    std::fs::create_dir_all(&out)?;
    let mobile = std::env::args().any(|a| a == "--mobile");
    let w = MainWindow::new()?;
    w.set_mobile(mobile);
    w.window().set_size(slint::LogicalSize::new(
        if mobile { 360. } else { 1440. },
        if mobile { 780. } else { 900. },
    ));
    playervf::i18n::install(&w);
    w.global::<I18n>().set_language("en".into());
    playervf::themes::apply_settings(&w, &Settings::default());
    let rows: Vec<_> = (0..12)
        .map(|i| {
            let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(80, 80);
            for (n, p) in pixels.make_mut_slice().iter_mut().enumerate() {
                *p = slint::Rgba8Pixel::new(
                    (40 + i * 15 + n % 80) as u8,
                    (60 + n / 80) as u8,
                    (160 - i * 8) as u8,
                    255,
                );
            }
            TrackRow {
                title: format!("Song {} · Home collection", i + 1).into(),
                artist: "Vey fixture".into(),
                path: format!("fixture://{i}").into(),
                duration: "3:42".into(),
                art: slint::Image::from_rgba8(pixels),
                ..Default::default()
            }
        })
        .collect();
    w.set_current_title("Find your next favorite".into());
    w.set_current_artist("Vey fixture".into());
    w.set_duration(200.);
    w.set_current_art(rows[0].art.clone());
    w.set_home_page_tracks(model(rows.clone()));
    w.set_tracks(model(rows));
    w.set_pages_seeded(true);
    w.set_player_state("paused".into());
    w.set_playlist_destinations(model(
        (0..3)
            .map(|i| PlaylistRow {
                id: format!("fixture-{i}").into(),
                name: ["Evening favorites", "A little energy", "Weekend collection"][i].into(),
                detail: "12 songs".into(),
                art: Default::default(),
            })
            .collect(),
    ));
    let store = Rc::new(RefCell::new(Store::default()));
    let get = {
        let s = store.clone();
        Rc::new(move || s.borrow().clone())
    };
    let apply = {
        let s = store.clone();
        let weak = w.as_weak();
        Rc::new(move |next: Store| {
            let ui = weak.upgrade().unwrap();
            ui.set_home_layout(next.settings.home_layout as i32);
            ui.set_home_rotate(next.settings.home_rotate);
            *s.borrow_mut() = next;
        })
    };
    playervf::home::install(&w, get, apply);
    w.set_music_supported(true);
    w.set_music_link("https://music.youtube.com/playlist?list=PL1234567890".into());
    w.show()?;
    let step = Rc::new(Cell::new(0));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(350),
        move || {
            let w = weak.upgrade().unwrap();
            let step_no = step.get();
            if step_no > 0 {
                let name = match step_no {
                    1..=6 => format!("home-{}", step_no - 1),
                    7 => "layout-picker".into(),
                    8 => "playlist-import".into(),
                    9 => "connections-vey".into(),
                    10 => "connections-youtube".into(),
                    _ => "connections-spotify".into(),
                };
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
            match step_no {
                0..=5 => w.invoke_home_choose(step_no),
                6 => w.set_dialog("home-layout".into()),
                7 => w.set_dialog("music-import".into()),
                8 => {
                    w.set_dialog("connections".into());
                    w.set_connection_provider(0);
                }
                9 => {
                    w.set_connection_provider(1);
                    w.set_music_provider(0);
                }
                10 => {
                    w.set_connection_provider(2);
                    w.set_music_provider(1);
                }
                _ => {
                    assert_eq!(w.get_home_layout(), 5);
                    slint::quit_event_loop().unwrap();
                }
            }
            step.set(step_no + 1);
        },
    );
    slint::run_event_loop()?;
    Ok(())
}
