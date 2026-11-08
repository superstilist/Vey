//! Isolated visual and input checks for the shared page background and dock.
use playervf::MainWindow;
use slint::{
    platform::{PointerEventButton, WindowEvent},
    ComponentHandle,
};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

fn click(w: &MainWindow, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    w.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    w.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    w.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: shared_playback_shell NEW_OUTPUT_DIRECTORY")?,
    );
    std::fs::create_dir(&out)?;
    let mobile = std::env::args().any(|arg| arg == "--mobile");
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    w.set_mobile(mobile);
    w.window().set_size(if mobile {
        slint::LogicalSize::new(390., 800.)
    } else {
        slint::LogicalSize::new(1200., 800.)
    });
    w.set_lyrics_reduced_motion(true);
    w.set_navigation_blur(false);
    w.set_kawarp_opacity(1.);
    w.set_duration(200.);
    w.set_position(50.);
    w.set_current_title("Shared playback fixture".into());
    w.set_current_artist("Vey".into());
    let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(1, 1);
    pixels.make_mut_bytes().copy_from_slice(&[45, 115, 85, 255]);
    w.set_ambient_frame(slint::Image::from_rgba8(pixels));
    let plays = Rc::new(Cell::new(0));
    w.on_play_toggle({
        let plays = plays.clone();
        move || plays.set(plays.get() + 1)
    });
    let seeks = Rc::new(RefCell::new(Vec::new()));
    w.on_seek_to({
        let seeks = seeks.clone();
        move |value| seeks.borrow_mut().push(value)
    });
    w.show()?;
    let step = Rc::new(Cell::new(0));
    let baseline = Rc::new(RefCell::new(None));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let w = weak.upgrade().unwrap();
            let n = step.get();
            let name = if mobile {
                ["home", "lyrics"][n]
            } else {
                ["home", "library", "lyrics"][n]
            };
            assert_eq!(w.get_displayed_view(), name);
            let frame = w.window().take_snapshot().unwrap();
            let scale = w.window().scale_factor();
            let sample = |x: f32, y: f32| {
                let offset = (((y * scale).round() as usize * frame.width() as usize)
                    + (x * scale).round() as usize)
                    * 4;
                frame.as_bytes()[offset..offset + 4].to_vec()
            };
            if !mobile {
                let pixels = (sample(1100., 680.), sample(5., 780.));
                assert!(
                    pixels.0[1] > pixels.0[0] + 20,
                    "ambient background on {name}"
                );
                let expected = baseline.borrow().clone();
                if let Some(expected) = expected {
                    assert_eq!(pixels, expected, "background and dock on {name}");
                } else {
                    *baseline.borrow_mut() = Some(pixels);
                }
            }
            image::save_buffer(
                out.join(format!("{name}.png")),
                frame.as_bytes(),
                frame.width(),
                frame.height(),
                image::ColorType::Rgba8,
            )
            .unwrap();
            if !mobile {
                click(&w, 99., 764.);
                click(&w, 600., 729.);
                assert_eq!(plays.get(), n + 1, "play control on {name}");
                assert_eq!(seeks.borrow().len(), n + 1, "seek control on {name}");
                assert!(
                    (seeks.borrow()[n] - 100_000).abs() < 1000,
                    "seek value on {name}"
                );
            }
            if n == if mobile { 1 } else { 2 } {
                slint::quit_event_loop().unwrap();
            } else {
                w.set_active_view(
                    if mobile {
                        "lyrics"
                    } else {
                        ["library", "lyrics"][n]
                    }
                    .into(),
                );
                step.set(n + 1);
            }
        },
    );
    slint::run_event_loop()?;
    Ok(())
}
