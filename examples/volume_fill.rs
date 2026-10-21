//! Check actual rendered volume fill, independently of pointer/input state.
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

slint::slint! {
    import { TonalSlider } from "../ui/controls.slint";
    export component VolumeFillWindow inherits Window {
        width: 360px; height: 160px;
        background: #10140f;
        in-out property <float> volume;
        TonalSlider { x: 24px; y: 38px; width: 312px; height: 44px; value <=> root.volume; label: "Volume"; }
        compact := TonalSlider { x: 24px; y: 90px; width: 84px; height: 44px; value <=> root.volume; label: "Compact volume"; }
        TonalSlider { x: 140px; y: 90px; width: 64px; height: 44px; value <=> root.volume; label: "Narrow volume"; }
        callback focus-compact();
        focus-compact => { compact.focus(); }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = VolumeFillWindow::new()?;
    let weak = window.as_weak();
    let step = Rc::new(Cell::new(0usize));
    let counter = step.clone();
    let samples = Rc::new(RefCell::new(Vec::<Vec<[u8; 4]>>::new()));
    let values = [0., 1., 0.25, 0.5, 0.75];
    let rails: [(f32, f32, f32, &[f32]); 3] = [
        (24., 38., 312., &[0.125, 0.375, 0.625, 0.875]),
        (24., 90., 84., &[0.25, 0.75]),
        (140., 90., 64., &[0.25, 0.75]),
    ];
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(180),
        move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let phase = counter.get();
            if phase / 2 >= values.len() {
                slint::quit_event_loop().unwrap();
                return;
            }
            let value = values[phase / 2];
            if phase % 2 == 0 {
                window.set_volume(value);
                window.invoke_focus_compact();
            } else {
                let snapshot = window.window().take_snapshot().unwrap();
                let scale = window.window().scale_factor();
                let probes: Vec<(f32, f32, f32, f32)> = rails
                    .iter()
                    .flat_map(|(x, y, width, positions)| {
                        positions.iter().map(move |position| {
                            (
                                x + 9. + (width - 18.) * position,
                                y + 22.,
                                *position,
                                width - 18.,
                            )
                        })
                    })
                    .collect();
                let pixel_at = |x: f32, y: f32| -> [u8; 4] {
                    let x = (x * scale).round() as usize;
                    let y = (y * scale).round() as usize;
                    let offset = (y * snapshot.width() as usize + x) * 4;
                    snapshot.as_bytes()[offset..offset + 4].try_into().unwrap()
                };
                let pixels: Vec<[u8; 4]> =
                    probes.iter().map(|(x, y, _, _)| pixel_at(*x, *y)).collect();
                let background = pixel_at(12., 90.);
                assert_eq!(pixel_at(66., 90.), background, "no top focus frame");
                assert_eq!(pixel_at(66., 133.), background, "no bottom focus frame");
                if phase / 2 >= 2 {
                    let baselines = samples.borrow();
                    for (index, (_, _, position, rail_width)) in probes.iter().enumerate() {
                        // Skip rail samples hidden by the thumb on compact sliders.
                        if (position - value).abs() * rail_width <= 10. {
                            continue;
                        }
                        let expected = if *position < value {
                            baselines[1][index]
                        } else {
                            baselines[0][index]
                        };
                        assert_eq!(
                            pixels[index], expected,
                            "volume {value}: fill must start at left, sample {position}"
                        );
                    }
                } else if phase / 2 == 1 {
                    assert!(
                        pixels
                            .iter()
                            .zip(&samples.borrow()[0])
                            .all(|(full, empty)| full != empty),
                        "empty and full rail must look different"
                    );
                }
                samples.borrow_mut().push(pixels);
                std::fs::create_dir_all("docs/screenshots").unwrap();
                image::save_buffer(
                    format!("docs/screenshots/volume-fill-{}.png", (value * 100.) as u32),
                    snapshot.as_bytes(),
                    snapshot.width(),
                    snapshot.height(),
                    image::ColorType::Rgba8,
                )
                .unwrap();
                println!("PASS volume {}%: left-to-right fill", (value * 100.) as u32);
            }
            counter.set(phase + 1);
        },
    );
    window.run()?;
    Ok(())
}
