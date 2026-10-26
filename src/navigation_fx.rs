//! Capture page content once per journey, then composite bounded blur textures.
//! The backdrop and chrome are excluded; no readback or CPU blur runs per frame.
use crate::MainWindow;
use image::{imageops, RgbaImage};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};

pub fn install(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_capture_navigation(move || {
        let Some(window) = weak.upgrade() else { return };
        window.set_navigation_fx_ready(false);
        if !window.get_navigation_blur() || window.get_lyrics_reduced_motion() {
            window.set_navigation_from_image(Image::default());
            window.set_navigation_to_image(Image::default());
            return;
        }
        // take_snapshot renders offscreen. Both temporary layouts are restored
        // before returning to the event loop, so neither is presented onscreen.
        window.set_navigation_capture(true);
        window.set_navigation_capture_target(false);
        let from = capture(&window);
        window.set_navigation_capture_target(true);
        let to = capture(&window);
        window.set_navigation_capture_target(false);
        window.set_navigation_capture(false);
        if let (Some(from), Some(to)) = (from, to) {
            window.set_navigation_from_image(from);
            window.set_navigation_to_image(to);
            window.set_navigation_fx_ready(true);
        }
    });
}

fn capture(window: &MainWindow) -> Option<Image> {
    let snapshot = window.window().take_snapshot().ok()?;
    let scale = window.window().scale_factor();
    let x = (window.get_stage_x() * scale).round().max(0.) as u32;
    let y = (window.get_stage_y() * scale).round().max(0.) as u32;
    let width = (window.get_stage_width() * scale).round().max(1.) as u32;
    let height = (window.get_stage_height() * scale).round().max(1.) as u32;
    let width = width.min(snapshot.width().checked_sub(x)?);
    let height = height.min(snapshot.height().checked_sub(y)?);
    if width == 0 || height == 0 {
        return None;
    }
    let pixels = RgbaImage::from_raw(
        snapshot.width(),
        snapshot.height(),
        snapshot.as_bytes().to_vec(),
    )?;
    let crop = imageops::crop_imm(&pixels, x, y, width, height);
    let ratio = (768. / width as f32).min(640. / height as f32).min(1.);
    let small = imageops::resize(
        &crop.to_image(),
        (width as f32 * ratio).round().max(1.) as u32,
        (height as f32 * ratio).round().max(1.) as u32,
        imageops::FilterType::Triangle,
    );
    let blurred = imageops::fast_blur(&small, 3.);
    Some(Image::from_rgba8(
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            blurred.as_raw(),
            blurred.width(),
            blurred.height(),
        ),
    ))
}
