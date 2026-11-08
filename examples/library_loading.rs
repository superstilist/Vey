//! Isolated visual check of the Library's asynchronous loading state.
use playervf::MainWindow;
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).ok_or("Output PNG path required")?;
    let mobile = std::env::args().any(|arg| arg == "--mobile");
    if std::path::Path::new(&output).exists() {
        return Err("Choose a new output file".into());
    }
    let window = MainWindow::new()?;
    playervf::i18n::install(&window);
    playervf::navigation_fx::install(&window);
    window.set_mobile(mobile);
    window.window().set_size(slint::LogicalSize::new(
        if mobile { 390. } else { 1100. },
        760.,
    ));
    window.set_pages_seeded(true);
    window.set_library_category_index(1);
    window.set_library_collections_loading(true);
    window.set_active_view("library".into());

    let weak = window.as_weak();
    let ticks = Rc::new(Cell::new(0));
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let Some(window) = weak.upgrade() else { return };
            ticks.set(ticks.get() + 1);
            if ticks.get() < 30 {
                return;
            }
            let frame = window.window().take_snapshot().expect("library snapshot");
            image::save_buffer(
                &output,
                frame.as_bytes(),
                frame.width(),
                frame.height(),
                image::ColorType::Rgba8,
            )
            .expect("save library snapshot");
            slint::quit_event_loop().expect("exit fixture");
        },
    );
    window.run()?;
    Ok(())
}
