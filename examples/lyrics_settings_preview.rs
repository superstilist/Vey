//! Render the shared lyric-settings menu for visual inspection, without user data.
use slint::ComponentHandle;
use std::time::Duration;
slint::slint! {
    import "../assets/fonts/Roboto.ttf";
    import { LyricsSettings } from "../ui/lyrics-settings.slint";
    export component LyricsSettingsPreview inherits Window {
        in-out property <bool> compact;
        width: root.compact ? 360px : 600px; height: 620px;
        background: #101310;
        LyricsSettings { x: (parent.width - self.width) / 2; y: 20px; width: root.compact ? 328px : 520px; }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = LyricsSettingsPreview::new()?;
    let compact = std::env::args().any(|a| a == "--compact");
    window.set_compact(compact);
    window.show()?;
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(700), move || {
        if let Some(window) = weak.upgrade() {
            let pixels = window.window().take_snapshot().unwrap();
            std::fs::create_dir_all("target/lyrics-settings").unwrap();
            image::save_buffer(
                format!(
                    "target/lyrics-settings/{}.png",
                    if compact { "mobile" } else { "desktop" }
                ),
                pixels.as_bytes(),
                pixels.width(),
                pixels.height(),
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
