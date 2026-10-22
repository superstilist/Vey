//! Native Slint layout smoke with synthetic Android bars/cutouts; no device claim.
use playervf::{LyricRow, MainWindow, PlaylistRow, TrackRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::Cell, rc::Rc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = MainWindow::new()?;
    playervf::i18n::install(&window);
    window.set_android_platform(true);
    window.set_lyrics_reduced_motion(true);
    window.set_current_title("Android library fixture".into());
    window.set_current_artist("Local audio".into());
    window.set_current_album("Fixture album".into());
    window.set_playlist_destinations(
        Rc::new(VecModel::from(
            (0..12)
                .map(|index| PlaylistRow {
                    id: format!("playlist-{index}").into(),
                    name: format!(
                        "Мій список {} — довга назва для перевірки відступів",
                        index + 1
                    )
                    .into(),
                    detail: format!("{} композицій", index + 3).into(),
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    let rows: ModelRc<TrackRow> = Rc::new(VecModel::from(vec![
        TrackRow {
            title: "Music from device storage".into(),
            artist: "Fixture artist".into(),
            album: "Fixture album".into(),
            path: "/storage/emulated/0/Music/test.wav".into(),
            duration: "0:30".into(),
            ..Default::default()
        };
        8
    ]))
    .into();
    window.set_tracks(rows.clone());
    window.set_library_page_tracks(rows.clone());
    window.set_home_page_tracks(rows.clone());
    window.set_explore_page_tracks(rows.clone());
    window.set_search_page_tracks(rows.clone());
    window.set_liked_page_tracks(rows);
    window.set_lyric_lines(
        Rc::new(VecModel::from(vec![
            LyricRow {
                text: "Коли місто засинає".into(),
                active: true,
                start_ms: 0,
                end_ms: 4000,
                ..Default::default()
            },
            LyricRow {
                text: "Музика залишається з нами".into(),
                start_ms: 4000,
                end_ms: 8000,
                ..Default::default()
            },
            LyricRow {
                text: "І веде крізь ніч".into(),
                start_ms: 8000,
                end_ms: 12000,
                ..Default::default()
            },
        ]))
        .into(),
    );
    window.set_lyrics_timed(true);
    window.set_active_lyric_index(0);
    window.set_duration(180.);
    window.set_position(2.);
    let cases = [
        ("home", "home", "", 390., 844., 32., 24., 0., 0.),
        ("library", "library", "", 390., 844., 32., 24., 0., 0.),
        (
            "settings", "settings", "library", 320., 640., 40., 32., 0., 0.,
        ),
        ("search", "search", "", 390., 844., 32., 24., 0., 0.),
        ("search-narrow", "search", "", 320., 640., 40., 32., 0., 0.),
        ("search-wide", "search", "", 430., 932., 32., 24., 0., 0.),
        (
            "search-landscape",
            "search",
            "",
            844.,
            390.,
            0.,
            24.,
            44.,
            0.,
        ),
        ("player", "lyrics", "", 390., 844., 32., 24., 0., 0.),
        ("player-narrow", "lyrics", "", 320., 640., 40., 32., 0., 0.),
        ("player-desktop", "lyrics", "", 1440., 900., 0., 0., 0., 0.),
        ("dialog", "library", "", 320., 640., 40., 32., 0., 0.),
        ("landscape", "library", "", 844., 390., 0., 24., 44., 0.),
        (
            "landscape-settings",
            "settings",
            "library",
            844.,
            390.,
            0.,
            24.,
            44.,
            0.,
        ),
        (
            "language", "settings", "language", 320., 640., 40., 32., 0., 0.,
        ),
        ("tablet", "library", "", 900., 900., 32., 24., 0., 0.),
        ("audio", "settings", "audio", 390., 844., 32., 24., 0., 0.),
        (
            "appearance",
            "settings",
            "appearance",
            320.,
            740.,
            32.,
            24.,
            0.,
            0.,
        ),
        ("playlist", "library", "", 320., 640., 40., 32., 0., 0.),
        (
            "playlist-desktop",
            "library",
            "",
            1440.,
            900.,
            0.,
            0.,
            0.,
            0.,
        ),
    ];
    let weak = window.as_weak();
    let phase = Rc::new(Cell::new(0));
    let index = phase.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(350),
        move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let step = index.get();
            if step / 2 >= cases.len() {
                slint::quit_event_loop().unwrap();
                return;
            }
            let (name, view, section, width, height, top, bottom, left, right) = cases[step / 2];
            if step % 2 == 0 {
                window
                    .global::<playervf::I18n>()
                    .set_language(if name == "language" { "ja" } else { "uk" }.into());
                window.set_dialog("".into());
                window.set_search_query(
                    if view == "search" {
                        "Вечірня музика"
                    } else {
                        ""
                    }
                    .into(),
                );
                window.set_safe_top(top);
                window.set_safe_bottom(bottom);
                window.set_safe_left(left);
                window.set_safe_right(right);
                window
                    .window()
                    .set_size(slint::LogicalSize::new(width, height));
                window.set_active_view(view.into());
                window.set_settings_section(section.into());
                window.set_color_mode(if section == "appearance" {
                    "background".into()
                } else {
                    "dynamic".into()
                });
                window.set_status(if view == "library" {
                    "Дозвольте доступ до музики й аудіо в запиті Android.".into()
                } else {
                    "".into()
                });
                if name == "dialog" {
                    window.set_dialog("source".into());
                }
                if name.starts_with("playlist") {
                    window.set_dialog("add".into());
                }
            } else {
                let size = window
                    .window()
                    .size()
                    .to_logical(window.window().scale_factor());
                assert!(
                    (size.width - width).abs() < 2. && (size.height - height).abs() < 2.,
                    "unexpected viewport for {name}: {size:?}"
                );
                assert!(
                    window.get_stage_x() >= left && window.get_stage_y() >= top,
                    "content overlaps cutout: {name}"
                );
                assert!(
                    window.get_stage_x() + window.get_stage_width() <= size.width - right,
                    "content overlaps right inset: {name}"
                );
                assert!(
                    window.get_stage_y() + window.get_stage_height() <= size.height - bottom,
                    "content overlaps bottom inset: {name}"
                );
                assert_eq!(
                    window.get_mobile(),
                    width - left - right < 800. || height < 600.,
                    "incorrect responsive mode: {name}"
                );
                let image = window.window().take_snapshot().unwrap();
                if window.get_mobile() && view != "lyrics" && window.get_dialog().is_empty() {
                    let reserve = if height - top - bottom < 500. {
                        112.
                    } else {
                        120.
                    };
                    let dock_top = height - bottom - reserve;
                    let scale = window.window().scale_factor();
                    let pixel = |y: f32| {
                        let x = ((left + 10.) * scale).round() as usize;
                        let y = (y * scale).round() as usize;
                        let offset = (y * image.width() as usize + x) * 4;
                        image.as_bytes()[offset..offset + 4].to_vec()
                    };
                    let color = window.global::<playervf::Tokens>().get_neutral_100();
                    let expected = vec![color.red(), color.green(), color.blue(), color.alpha()];
                    assert_eq!(
                        pixel(dock_top + 54.),
                        expected,
                        "transport must fill the joined dock: {name}"
                    );
                    assert_eq!(
                        pixel(dock_top + 58.),
                        expected,
                        "navigation must fill the joined dock: {name}"
                    );
                }
                std::fs::create_dir_all("docs/screenshots").unwrap();
                image::save_buffer(
                    format!("docs/screenshots/android-insets-{name}.png"),
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image::ColorType::Rgba8,
                )
                .unwrap();
                println!("PASS {name}: {width}x{height}, insets {top}/{bottom}/{left}/{right}");
            }
            index.set(step + 1);
        },
    );
    window.run()?;
    Ok(())
}
