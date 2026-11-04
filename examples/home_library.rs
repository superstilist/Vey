//! Isolated Home/import/account-choice fixture. No real state or network.
use playervf::{
    services::{Settings, Store, Track},
    I18n, MainWindow, PlaylistRow, TrackRow,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
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
    if std::env::args().any(|a| a == "--live") {
        let catalog = tokio::runtime::Runtime::new()?
            .block_on(playervf::native::ytm_home_catalog(Some("onokami".into())))?;
        std::fs::write(
            out.join("catalog.json"),
            serde_json::to_vec_pretty(&catalog)?,
        )?;
        for section in catalog.sections {
            println!("{}: {} items", section.kind, section.items.len());
        }
        return Ok(());
    }
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
            image::save_buffer(
                out.join(format!("cover-{i}.png")),
                pixels.as_bytes(),
                80,
                80,
                image::ColorType::Rgba8,
            )
            .unwrap();
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
    let mut fixture = Store::default();
    fixture.home_catalog.updated_at = playervf::services::now_ms();
    fixture.tracks = w
        .get_home_page_tracks()
        .iter()
        .enumerate()
        .map(|(i, row)| Track {
            path: row.path.to_string(),
            title: row.title.to_string(),
            artist: row.artist.to_string(),
            duration_secs: 222.0,
            thumbnail_url: format!("https://fixtures.vey.test/cover-{i}.png"),
            ..Default::default()
        })
        .collect();
    fixture.recommendations.tracks = fixture.tracks.iter().rev().take(8).cloned().collect();
    for (n, kind) in ["video", "album", "playlist", "artist"]
        .into_iter()
        .enumerate()
    {
        fixture
            .home_catalog
            .sections
            .push(playervf::discovery::Section {
                key: kind.into(),
                kind: kind.into(),
                title: ["Музичні відео для вас", "Альбоми", "Плейлісти", "Виконавці"][n].into(),
                items: fixture
                    .tracks
                    .iter()
                    .take(8)
                    .enumerate()
                    .map(|(i, track)| {
                        let mut item = playervf::discovery::Item::song(track);
                        item.kind = kind.into();
                        item.key = format!("{kind}-{i}");
                        item.title = format!(
                            "{} {}",
                            ["Live session", "After hours", "Weekend mix", "Artist"][n],
                            i + 1
                        );
                        item.subtitle = "Vey · Independent music".into();
                        item.browse_id = format!(
                            "{}fixture{i}",
                            if kind == "album" {
                                "MPRE"
                            } else if kind == "artist" {
                                "UC"
                            } else {
                                "VL"
                            }
                        );
                        if kind == "video" {
                            // The same track can occur on both Listen again and Video.
                            item.key = track.path.clone();
                            item.video_id = format!("fixture{i:04}");
                        }
                        item
                    })
                    .collect(),
                ..Default::default()
            });
    }
    std::fs::write(out.join("store.json"), serde_json::to_vec_pretty(&fixture)?)?;
    let weak = w.as_weak();
    w.on_request_discovery_art(move |json| {
        if let (Some(w), Ok(track)) = (weak.upgrade(), serde_json::from_str::<Track>(&json)) {
            if let Some(row) = w
                .get_home_page_tracks()
                .iter()
                .find(|row| row.path == track.path)
            {
                playervf::discovery::update_art(&w, &track.path, &row.art);
            }
        }
    });
    let store = Rc::new(RefCell::new(fixture));
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
            ui.set_home_revision(ui.get_home_revision() + 1);
            *s.borrow_mut() = next;
        })
    };
    playervf::discovery::install(&w, get.clone(), apply.clone());
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
                    _ => "connections-ytm".into(),
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
