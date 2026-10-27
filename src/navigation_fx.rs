//! Capture page content once per journey, then composite bounded blur textures.
//! The backdrop and chrome are excluded; no readback or CPU blur runs per frame.
use crate::MainWindow;
use image::{imageops, RgbaImage};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};
#[cfg(not(target_arch = "wasm32"))]
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

struct Capture {
    pixels: RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}
#[cfg(not(target_arch = "wasm32"))]
struct Ready {
    generation: u64,
    from: RgbaImage,
    to: RgbaImage,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn install(window: &MainWindow) {
    let (send, jobs) = mpsc::sync_channel::<(u64, Capture, Capture)>(1);
    let (done, ready) = mpsc::channel::<Ready>();
    std::thread::Builder::new()
        .name("vey-navigation-blur".into())
        .spawn(move || {
            while let Ok((generation, from, to)) = jobs.recv() {
                let result = Ready {
                    generation,
                    from: prepare(from),
                    to: prepare(to),
                };
                if done.send(result).is_err() {
                    break;
                }
            }
        })
        .expect("navigation blur worker");
    let generation = Rc::new(Cell::new(0u64));
    let busy = Rc::new(Cell::new(false));
    let epoch = generation.clone();
    let pending = busy.clone();
    let weak = window.as_weak();
    window.on_capture_navigation(move || {
        let Some(window) = weak.upgrade() else { return };
        epoch.set(epoch.get().wrapping_add(1));
        window.set_navigation_fx_ready(false);
        if !window.get_navigation_blur() || window.get_lyrics_reduced_motion() {
            window.set_navigation_from_image(Image::default());
            window.set_navigation_to_image(Image::default());
            return;
        }
        // Never queue expensive obsolete readbacks on rapid retargets.
        if pending.get() {
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
            pending.set(send.try_send((epoch.get(), from, to)).is_ok());
        }
    });
    let weak = window.as_weak();
    let owner: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));
    let keep = owner.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(window) = weak.upgrade() else {
                keep.borrow_mut().take();
                return;
            };
            let Ok(result) = ready.try_recv() else {
                return;
            };
            busy.set(false);
            if result.generation != generation.get()
                || !window.get_page_moving()
                || !window.get_navigation_blur()
                || window.get_lyrics_reduced_motion()
                || window.window().is_minimized()
            {
                return;
            }
            window.set_navigation_from_image(upload(result.from));
            window.set_navigation_to_image(upload(result.to));
            window.set_navigation_fx_ready(true);
        },
    );
    *owner.borrow_mut() = Some(timer);
}

// The browser has no native threads; retain its existing snapshot fallback.
#[cfg(target_arch = "wasm32")]
pub fn install(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_capture_navigation(move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        window.set_navigation_fx_ready(false);
        if !window.get_navigation_blur() || window.get_lyrics_reduced_motion() {
            window.set_navigation_from_image(Image::default());
            window.set_navigation_to_image(Image::default());
            return;
        }
        window.set_navigation_capture(true);
        window.set_navigation_capture_target(false);
        let from = capture(&window);
        window.set_navigation_capture_target(true);
        let to = capture(&window);
        window.set_navigation_capture_target(false);
        window.set_navigation_capture(false);
        if let (Some(from), Some(to)) = (from, to) {
            window.set_navigation_from_image(upload(prepare(from)));
            window.set_navigation_to_image(upload(prepare(to)));
            window.set_navigation_fx_ready(true);
        }
    });
}

fn upload(pixels: RgbaImage) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        pixels.as_raw(),
        pixels.width(),
        pixels.height(),
    ))
}

fn capture(window: &MainWindow) -> Option<Capture> {
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
    Some(Capture {
        pixels,
        x,
        y,
        width,
        height,
    })
}

fn prepare(capture: Capture) -> RgbaImage {
    let Capture {
        pixels,
        x,
        y,
        width,
        height,
    } = capture;
    let crop = imageops::crop_imm(&pixels, x, y, width, height);
    let ratio = (768. / width as f32).min(640. / height as f32).min(1.);
    let small = imageops::resize(
        &crop.to_image(),
        (width as f32 * ratio).round().max(1.) as u32,
        (height as f32 * ratio).round().max(1.) as u32,
        imageops::FilterType::Triangle,
    );
    imageops::fast_blur(&small, 3.)
}
