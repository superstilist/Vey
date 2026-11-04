//! Isolated snapshots for library categories and the compact player queue.
//! Uses generated rows and artwork; never reads user state or audio.
use playervf::{I18n, LibraryCollectionRow, MainWindow, TrackRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{rc::Rc, time::Duration};

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}

fn cover(seed: u8) -> slint::Image {
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(72, 72);
    for (i, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        let x = i % 72;
        let y = i / 72;
        *pixel = slint::Rgba8Pixel::new(
            seed.wrapping_add((x * 2) as u8),
            48_u8.wrapping_add((y * 2) as u8),
            150_u8.wrapping_add(seed / 3),
            255,
        );
    }
    slint::Image::from_rgba8(pixels)
}

fn save(window: &MainWindow, output: &std::path::Path, name: &str) {
    let frame = window.window().take_snapshot().expect("capture Vey screen");
    image::save_buffer(
        output.join(format!("{name}.png")),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .expect("write Vey screen capture");
    println!("Captured {name}");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or(
        "Usage: cargo run --locked --example music_library_layout -- target/NEW_OUTPUT_DIRECTORY",
    )?;
    let output = std::path::PathBuf::from(output);
    std::fs::create_dir(&output)?;

    let window = MainWindow::new()?;
    playervf::i18n::install(&window);
    window.global::<I18n>().set_language("uk".into());
    window.set_active_view("library".into());
    window.set_pages_seeded(true);
    window.set_current_title("Тиша між нотами".into());
    window.set_current_artist("Vey · приклад".into());
    window.set_current_track_key("fixture://0".into());
    window.set_player_state("paused".into());
    window.set_duration(190.);
    window.set_position(7.2);
    window.set_time_label("0:07 / 3:10".into());

    let tracks: Vec<_> = (0..12)
        .map(|index| TrackRow {
            title: format!("Композиція {} · вечірня добірка", index + 1).into(),
            artist: ["Марта Лісова", "Тихий Берег", "Марта Лісова"][index % 3].into(),
            album: ["Дорога додому", "Синє світло", "Нічне місто"][index % 3].into(),
            path: format!("fixture://{index}").into(),
            duration: "3:10".into(),
            art: cover((index * 17) as u8),
            liked: index % 3 == 0,
            downloaded: index % 2 == 0,
            selected: index < 3,
            ..Default::default()
        })
        .collect();
    let collections: Vec<_> = (0..8)
        .map(|index| LibraryCollectionRow {
            kind: if index % 2 == 0 { "album" } else { "artist" }.into(),
            key: format!("collection:{index}").into(),
            title: if index % 2 == 0 {
                format!("Альбом {}", index + 1).into()
            } else {
                format!("Виконавець {}", index + 1).into()
            },
            subtitle: if index % 2 == 0 {
                "Марта Лісова".into()
            } else {
                String::new().into()
            },
            count: "8".into(),
            art: cover((index * 23) as u8),
        })
        .collect();
    let playlists: Vec<_> = (0..4)
        .map(|index| LibraryCollectionRow {
            kind: "playlist".into(),
            key: format!("playlist:{index}").into(),
            title: format!("Мій список {}", index + 1).into(),
            subtitle: String::new().into(),
            count: (index * 3).to_string().into(),
            art: cover((index * 31) as u8),
        })
        .collect();

    window.set_tracks(model(tracks.clone()));
    window.set_library_page_tracks(model(tracks.clone()));
    window.set_library_collections(model(collections.clone()));
    window.set_library_total_count(tracks.len() as i32);
    window.set_library_selected_count(0);
    window.set_library_source_status("Джерело готове · сканування завершено".into());
    window.set_library_scanning(false);

    let mut queue = tracks.clone();
    for track in queue.iter_mut().skip(1) {
        track.selected = false;
    }
    window.set_queue_tracks(model(queue));
    window.set_queue_current_index(0);

    let steps = Rc::new(std::cell::Cell::new(0));
    let completed = steps.clone();
    let weak = window.as_weak();
    let output_for_timer = output.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(550),
        move || {
            let Some(window) = weak.upgrade() else { return };
            match steps.get() {
                0 => {
                    window
                        .window()
                        .set_size(slint::LogicalSize::new(1440., 900.));
                    window.set_mobile(false);
                    window.set_library_category_index(0);
                    save(&window, &output_for_timer, "library-songs-desktop");
                }
                1 => {
                    window.set_library_category_index(1);
                    window.set_library_page_tracks(model(Vec::<TrackRow>::new()));
                    window.set_library_collections(model(collections.clone()));
                    save(&window, &output_for_timer, "library-albums-desktop");
                }
                2 => {
                    window.set_library_category_index(0);
                    window.set_library_page_tracks(model(tracks.clone()));
                    window.set_library_selected_count(3);
                    save(&window, &output_for_timer, "library-bulk-desktop");
                }
                3 => {
                    window
                        .window()
                        .set_size(slint::LogicalSize::new(360., 780.));
                    window.set_mobile(true);
                    window.set_library_category_index(0);
                    window.set_library_selected_count(0);
                    save(&window, &output_for_timer, "library-songs-mobile");
                }
                4 => {
                    window
                        .window()
                        .set_size(slint::LogicalSize::new(320., 640.));
                    save(&window, &output_for_timer, "library-songs-mobile-narrow");
                }
                5 => {
                    window.set_library_selected_count(3);
                    save(&window, &output_for_timer, "library-bulk-mobile-narrow");
                }
                6 => {
                    window
                        .window()
                        .set_size(slint::LogicalSize::new(360., 780.));
                    window.set_library_category_index(3);
                    window.set_library_selected_count(0);
                    window.set_library_page_tracks(model(Vec::<TrackRow>::new()));
                    window.set_library_collections(model(playlists.clone()));
                    save(&window, &output_for_timer, "library-playlists-mobile");
                }
                7 => {
                    window.set_active_view("lyrics".into());
                    window.set_player_tab("queue".into());
                }
                8 => {
                    save(&window, &output_for_timer, "player-queue-mobile");
                }
                _ => {
                    slint::quit_event_loop().expect("quit snapshot fixture");
                    return;
                }
            }
            steps.set(steps.get() + 1);
        },
    );
    window.run()?;
    if completed.get() != 9 {
        return Err("Library snapshot fixture ended early".into());
    }
    Ok(())
}
