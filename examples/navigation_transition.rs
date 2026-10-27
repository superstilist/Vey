//! Native visual fixture: real page blur, direction changes and rounded clipping.
use playervf::{MainWindow, TrackRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::Cell, rc::Rc, time::Duration};

fn save(window: &MainWindow, name: &str) {
    let frame = window.window().take_snapshot().unwrap();
    std::fs::create_dir_all("docs/screenshots").unwrap();
    image::save_buffer(
        format!("docs/screenshots/navigation-{name}.png"),
        frame.as_bytes(),
        frame.width(),
        frame.height(),
        image::ColorType::Rgba8,
    )
    .unwrap();
}

fn check_corners(window: &MainWindow) {
    let frame = window.window().take_snapshot().unwrap();
    let scale = window.window().scale_factor();
    let pixel = |x: f32, y: f32| {
        let offset = (((y * scale).round() as u32 * frame.width() + (x * scale).round() as u32) * 4)
            as usize;
        frame.as_bytes()[offset..offset + 4].to_vec()
    };
    let x = window.get_stage_x();
    let y = window.get_stage_y();
    assert_eq!(
        pixel(x + 1., y + 1.),
        pixel(x - 2., y + 1.),
        "inner corner clips the ambient surface"
    );
    assert_ne!(
        pixel(x + window.get_stage_radius() + 3., y + 2.),
        pixel(x + 1., y + 1.),
        "stage remains visible inside its rounded edge"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = MainWindow::new()?;
    playervf::i18n::install(&window);
    playervf::navigation_fx::install(&window);
    let mut backdrop = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(1, 1);
    backdrop
        .make_mut_bytes()
        .copy_from_slice(&[32, 84, 70, 255]);
    window.set_ambient_frame(slint::Image::from_rgba8(backdrop));
    window.set_kawarp_opacity(0.6);
    window
        .window()
        .set_size(slint::LogicalSize::new(1100., 760.));
    let tracks: ModelRc<TrackRow> = Rc::new(VecModel::from(
        (0..12)
            .map(|i| TrackRow {
                title: format!("Navigation fixture track {}", i + 1).into(),
                artist: "Content stays inside rounded corners".into(),
                album: "Fixture".into(),
                duration: "3:24".into(),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))
    .into();
    window.set_tracks(tracks.clone());
    window.set_home_page_tracks(tracks.clone());
    window.set_library_page_tracks(tracks);
    let step = Rc::new(Cell::new(0u32));
    let result = step.clone();
    let retries = Rc::new(Cell::new(0u32));
    let blur_verified = Rc::new(Cell::new(false));
    let weak = window.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(32), move || {
        let Some(w) = weak.upgrade() else { return };
        retries.set(retries.get() + 1);
        assert!(retries.get() < 220, "navigation fixture timed out at {}, moving={}, p={}, ready={}", step.get(), w.get_page_moving(), w.get_navigation_progress(), w.get_navigation_fx_ready());
        let p = w.get_navigation_progress();
        match step.get() {
            0 if retries.get() > 12 => {
                check_corners(&w);
                save(&w, "settled"); w.set_active_view("library".into()); step.set(1);
            }
            1 if (w.get_camera_travelling() && p > 0.2) || !w.get_page_moving() => {
                if !w.get_navigation_fx_ready() && w.get_camera_travelling() && p<0.65 {return;}
                if w.get_navigation_fx_ready() {
                    let pixels = w.get_navigation_to_image().to_rgba8().unwrap();
                    assert!(pixels.width() <= 768 && pixels.height() <= 640);
                    assert!(pixels.as_slice().iter().any(|p| p.a == 0), "background excluded from blur texture");
                    blur_verified.set(true);
                }
                save(&w, "diagonal");
                w.set_active_view("settings".into()); step.set(2);
            }
            2 if w.get_camera_travelling() && p > 0.3 && p < 0.9 && w.get_camera_end_y() == -1. => {
                assert!(w.get_navigation_pulse() > 0.);
                save(&w, "retarget"); step.set(3);
            }
            3 if !w.get_page_moving() => {
                assert_eq!(w.get_navigation_pulse(), 0.);
                w.set_navigation_blur(false); w.set_active_view("home".into()); step.set(4);
            }
            4 if w.get_camera_travelling() && p > 0.25 => {
                assert!(!w.get_navigation_fx_ready());
                save(&w, "blur-disabled");
                w.set_lyrics_reduced_motion(true); w.set_active_view("liked".into()); step.set(5);
            }
            5 if !w.get_page_moving() => {
                assert_eq!(w.get_navigation_pulse(), 0.);
                save(&w, "reduced");
                w.window().set_size(slint::LogicalSize::new(390., 760.)); step.set(6);
            }
            6 => {
                check_corners(&w); save(&w, "mobile-rounded");
                println!("PASS native navigation: blur pixels verified={}, live fallback when late, diagonal travel, retarget, blur switch, reduced motion, desktop/mobile corners",blur_verified.get());
                slint::quit_event_loop().unwrap(); step.set(7);
            }
            _ => {}
        }
    });
    window.run()?;
    assert_eq!(result.get(), 7);
    Ok(())
}
