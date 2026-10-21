//! Native visual preview of the shared song row and rounded download corner.
use slint::ComponentHandle;
use std::time::Duration;
slint::slint! {
    export { CoverBadges } from "../ui/controls.slint";
    import { SongRow, Artwork } from "../ui/controls.slint";
    export component SongLayoutPreview inherits Window {
        width: root.reference ? 324px : root.compact ? 360px : 880px; height: root.reference ? 324px : root.compact ? 470px : 430px; background: #323232;
        in-out property <image> art;
        in-out property <bool> compact;
        in-out property <bool> reference;
        if root.reference: Artwork { art: root.art; downloaded: true; x: 8px; y: 8px; width: 308px; height: 308px; border-radius: 16px; }
        if !root.reference: VerticalLayout {
            padding: 24px; spacing: 8px;
            Text { text: "Vey · downloads"; color: #e5eadf; font-size: 22px; font-weight: 700; }
            SongRow { title: "A song saved for later"; artist: "The Artist"; album: "An album"; duration: "3:42"; art: root.art; downloaded: true; current: true; mobile: root.compact; }
            SongRow { title: "A longer song title that should leave room for the controls"; artist: "Featured artist · Another artist"; album: "A longer album title"; duration: "4:18"; art: root.art; mobile: root.compact; downloading: true; }
            SongRow { title: "No cover image"; artist: "Local artist"; duration: "2:56"; downloaded: true; mobile: root.compact; }
            HorizontalLayout {
                spacing: 16px; alignment: start;
                Artwork { art: root.art; width: 88px; height: 88px; border-radius: 16px; downloaded: true; }
                Artwork { art: root.art; width: 48px; height: 48px; border-radius: 12px; downloaded: true; }
                Rectangle { horizontal-stretch: 1; }
            }
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = SongLayoutPreview::new()?;
    let compact = std::env::args().any(|a| a == "--compact");
    window.set_compact(compact);
    let reference = std::env::args().any(|a| a == "--reference");
    let hidden = std::env::args().any(|a| a == "--hidden");
    let icon = std::env::args().any(|a| a == "--icon");
    window.set_reference(reference);
    window.global::<CoverBadges>().set_shown(!hidden);
    if icon {
        window.global::<CoverBadges>().set_style("icon".into());
    }
    if compact {
        window
            .window()
            .set_size(slint::LogicalSize::new(360., 470.));
    }
    let mut image = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(128, 128);
    for (i, pixel) in image.make_mut_slice().iter_mut().enumerate() {
        let x = (i % 128) as u8;
        let y = (i / 128) as u8;
        *pixel = if reference {
            slint::Rgba8Pixel::new(25, 118, 210, 255)
        } else {
            slint::Rgba8Pixel::new(90 + x, 40 + y, 80 + x / 2, 255)
        };
    }
    window.set_art(slint::Image::from_rgba8(image));
    window.show()?;
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::from_millis(700), move || {
        if let Some(window) = weak.upgrade() {
            let snapshot = window.window().take_snapshot().unwrap();
            std::fs::create_dir_all("target/song-layout").unwrap();
            image::save_buffer(
                format!(
                    "target/song-layout/{}.png",
                    if reference {
                        "reference"
                    } else if hidden {
                        "hidden"
                    } else if icon {
                        "icon"
                    } else if compact {
                        "mobile"
                    } else {
                        "desktop"
                    }
                ),
                snapshot.as_bytes(),
                snapshot.width(),
                snapshot.height(),
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
        slint::quit_event_loop().unwrap();
    });
    slint::run_event_loop()?;
    Ok(())
}
