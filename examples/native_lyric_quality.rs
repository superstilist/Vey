//! Native ink/letter handoff pixels. Outputs are isolated in a new directory.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

const WORDS: [&str; 4] = ["AVATAR", "Toffee", "office", "e\u{301}cole"];
const STAGES: [&str; 13] = [
    "plain-start",
    "wave-start",
    "wave-last",
    "wave-return",
    "plain-end",
    "wave-end",
    "motion-start-10",
    "motion-start-20",
    "motion-start-30",
    "wave-middle",
    "motion-end-minus-30",
    "motion-end-minus-20",
    "motion-end-minus-10",
];

fn position_ms(w: &MainWindow, milliseconds: f32) {
    // f32 1.4 seconds is slightly below 1400ms when the adapter truncates its
    // f64 clock. Round a boundary up to a media millisecond and use the next
    // positive f32, so exact cue-end captures really sample the cue end.
    let seconds = milliseconds.ceil() / 1000.;
    w.set_position(f32::from_bits(seconds.to_bits() + 1));
}

fn setup(w: &MainWindow, text: &str, duration: i32, color_alpha: Option<u8>) {
    let mut s = w.get_lyric_style();
    s.glow = false;
    s.fade = false;
    s.wave = false;
    s.wobble = false;
    s.swipe = color_alpha.is_some();
    s.inactive_blur = 0.;
    s.active_scale = 1.;
    s.inactive_scale = 1.;
    w.set_lyric_style(s);
    w.set_lyric_active(if color_alpha.is_some() {
        slint::Color::from_rgb_u8(242, 78, 38)
    } else {
        slint::Color::from_rgb_u8(255, 255, 255)
    });
    w.set_lyric_inactive(if let Some(alpha) = color_alpha {
        slint::Color::from_argb_u8(alpha, 40, 80, 255)
    } else {
        slint::Color::from_rgb_u8(80, 80, 80)
    });
    w.set_lyric_lines(
        Rc::new(VecModel::from(vec![LyricRow {
            text: text.into(),
            start_ms: 0,
            end_ms: 10000,
            words: Rc::new(VecModel::from(vec![WordRow {
                text: text.into(),
                start_ms: 1000,
                end_ms: 1000 + duration,
                explicit: true,
                ..Default::default()
            }]))
            .into(),
            ..Default::default()
        }]))
        .into(),
    );
    position_ms(w, if color_alpha.is_some() { 500. } else { 1000. });
}
fn capture(w: &MainWindow, output: &Path, name: &str) -> image::RgbaImage {
    let f = w.window().take_snapshot().unwrap();
    let full = image::RgbaImage::from_raw(f.width(), f.height(), f.as_bytes().to_vec()).unwrap();
    // Snapshot dimensions are physical pixels. Keep this logical test region
    // fixed when SLINT_SCALE_FACTOR changes the renderer's pixel density.
    let sx = full.width() as f64 / 1440.;
    let sy = sx;
    let x = (740. * sx).round() as u32;
    let y = (250. * sy).round() as u32;
    let width = ((660. * sx).round() as u32).min(full.width() - x);
    let height = ((400. * sy).round() as u32).min(full.height() - y);
    let crop = image::imageops::crop_imm(&full, x, y, width, height).to_image();
    crop.save(output.join(format!("{name}.png"))).unwrap();
    crop
}
fn differences(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    assert_eq!(
        a.dimensions(),
        b.dimensions(),
        "fixture window resized during capture"
    );
    a.pixels()
        .zip(b.pixels())
        .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) > 3))
        .count()
}
fn bounds(a: &image::RgbaImage) -> Option<[u32; 4]> {
    let mut x0 = a.width();
    let mut y0 = a.height();
    let mut x1 = 0;
    let mut y1 = 0;
    for (x, y, p) in a.enumerate_pixels() {
        if p[0] > 200 && p[1] > 200 && p[2] > 200 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    (x0 <= x1 && y0 <= y1).then_some([x0, y0, x1, y1])
}
fn color_metrics(images: &[image::RgbaImage]) -> serde_json::Value {
    let full = &images[2];
    let mut points = vec![];
    for y in 1..full.height() - 1 {
        for x in 1..full.width() - 1 {
            if (y - 1..=y + 1).all(|yy| (x - 1..=x + 1).all(|xx| full.get_pixel(xx, yy)[0] >= 238))
            {
                points.push((x, y));
            }
        }
    }
    assert!(points.len() > 50, "need solid colored glyph interiors");
    let x0 = points.iter().map(|p| p.0).min().unwrap();
    let x1 = points.iter().map(|p| p.0).max().unwrap();
    let middle = points
        .iter()
        .copied()
        .filter(|(x, _)| (*x as f32 - (x0 + x1) as f32 / 2.).abs() < (x1 - x0) as f32 * 0.08)
        .collect::<Vec<_>>();
    let mean = |image: &image::RgbaImage, pixels: &[(u32, u32)]| -> Vec<f64> {
        (0..3)
            .map(|c| {
                pixels
                    .iter()
                    .map(|p| image.get_pixel(p.0, p.1)[c] as f64)
                    .sum::<f64>()
                    / pixels.len().max(1) as f64
            })
            .collect()
    };
    serde_json::json!({"core_pixels":points.len(),"edge_core_pixels":middle.len(),"off_mean_rgb":mean(&images[0],&points),"middle_edge_mean_rgb":mean(&images[1],&middle),"full_mean_rgb":mean(full,&points)})
}

fn endpoint_metrics(plain: &image::RgbaImage, sample: &image::RgbaImage) -> serde_json::Value {
    let [left, _, right, _] = bounds(plain).expect("visible plain ink");
    let middle_left = left + (right - left) / 4;
    let middle_right = right - (right - left) / 4;
    let mut core = Vec::new();
    for y in 1..plain.height() - 1 {
        for x in middle_left..=middle_right {
            if (y - 1..=y + 1).all(|yy| (x - 1..=x + 1).all(|xx| plain.get_pixel(xx, yy)[0] >= 250))
            {
                core.push(sample.get_pixel(x, y)[0] as f64);
            }
        }
    }
    let background = plain.get_pixel(0, 0)[0] as f64;
    let mass = |im: &image::RgbaImage| {
        im.pixels()
            .map(|p| (p[0] as f64 - background).max(0.))
            .sum::<f64>()
    };
    serde_json::json!({
        "changed_pixels": differences(plain, sample),
        "bounds": bounds(sample),
        "ink_mass_ratio": mass(sample) / mass(plain),
        "middle_core_pixels": core.len(),
        "middle_core_mean": core.iter().sum::<f64>() / core.len().max(1) as f64,
        "middle_core_dark_pixels": core.iter().filter(|v| **v < 240.).count(),
    })
}
#[derive(Default)]
struct State {
    case: usize,
    stage: usize,
    warm: usize,
    retry: usize,
    window: f32,
    glyphs: usize,
    frames: Vec<image::RgbaImage>,
    metrics: Vec<serde_json::Value>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1);
    }));
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Usage: native_lyric_quality NEW_OUTPUT_DIRECTORY")?,
    );
    if output.exists() {
        return Err("Output directory already exists; choose a new directory".into());
    }
    std::fs::create_dir(&output)?;
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_player_tab("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    w.set_duration(10.);
    w.set_player_state("paused".into());
    let scale = w.window().scale_factor().max(1.);
    // Keep the physical height within a desktop work area at fractional DPI;
    // Windows can otherwise clamp an oversized window during the capture run.
    w.window()
        .set_size(slint::LogicalSize::new(1440., 900. / scale));
    setup(&w, WORDS[0], 400, None);
    let state = Rc::new(RefCell::new(State::default()));
    let done = state.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(250),move||{
        let w=weak.upgrade().unwrap();if !native_window::ready(w.window()){return;}w.window().request_redraw();
        let mut s=state.borrow_mut();if s.warm<5 {s.warm+=1;return;}
        if w.get_lyrics_position() != (w.get_position()*1000.).round() as i32 {s.retry+=1;assert!(s.retry<30,"clock did not settle");return;}
        let case=s.case;let stage=s.stage;
        if case<8 {
            let duration=if case<4 {400}else{4000};let word=WORDS[case%4];
            if stage==1 || stage==5 {
                let rows=w.get_native_lyric_lines();let part=rows.row_data(0).and_then(|r|r.parts.row_data(0)).unwrap();
                if part.glyphs.row_count()==0 {s.retry+=1;assert!(s.retry<30,"letter plan did not warm");return;}
                if stage==1 {let first=part.glyphs.row_data(0).unwrap();let last=part.glyphs.row_data(part.glyphs.row_count()-1).unwrap();let plan_start=first.delay_ms.min(0.);s.window=(last.delay_ms+last.wave_ms-plan_start).min(duration as f32);s.glyphs=part.glyphs.row_count();}
            }
            s.retry=0;s.frames.push(capture(&w,&output,&format!("shape-{case}-{}",STAGES[stage])));
            match stage {
                0=>{let mut style=w.get_lyric_style();style.wave=true;w.set_lyric_style(style);}
                1=>position_ms(&w,1000.+s.window-1.),
                2=>position_ms(&w,1000.+s.window),
                3=>{let mut style=w.get_lyric_style();style.wave=false;w.set_lyric_style(style);position_ms(&w,(1000+duration) as f32);}
                4=>{let mut style=w.get_lyric_style();style.wave=true;w.set_lyric_style(style);}
                5=>position_ms(&w,1010.),
                6=>position_ms(&w,1020.),
                7=>position_ms(&w,1030.),
                8=>position_ms(&w,1000.+s.window/2.),
                9=>position_ms(&w,1000.+s.window-30.),
                10=>position_ms(&w,1000.+s.window-20.),
                11=>position_ms(&w,1000.+s.window-10.),
                _=>{
                    let metric=serde_json::json!({"kind":"shape","case":case,"text":word,"duration_ms":duration,"letter_window_ms":s.window,"glyphs":s.glyphs,"start_changed_pixels":differences(&s.frames[0],&s.frames[1]),"start_plain_bounds":bounds(&s.frames[0]),"start_wave_bounds":bounds(&s.frames[1]),"return_changed_pixels":differences(&s.frames[2],&s.frames[3]),"end_changed_pixels":differences(&s.frames[4],&s.frames[5])});
                    println!("{metric}");s.metrics.push(metric);
                    for index in 6..STAGES.len() {
                        let mut endpoint=endpoint_metrics(&s.frames[0],&s.frames[index]);
                        endpoint["kind"]=serde_json::json!("endpoint");endpoint["case"]=serde_json::json!(case);endpoint["text"]=serde_json::json!(word);endpoint["duration_ms"]=serde_json::json!(duration);endpoint["stage"]=serde_json::json!(STAGES[index]);
                        println!("{endpoint}");s.metrics.push(endpoint);
                    }
                    s.frames.clear();s.case+=1;s.stage=0;
                    if s.case<8 {setup(&w,WORDS[s.case%4],if s.case<4 {400}else{4000},None);} else {setup(&w,"HHHHH",4000,Some(64));}return;
                }
            }
            s.stage+=1;
        } else {
            let alpha=if case==8 {64}else{128};
            s.frames.push(capture(&w,&output,&format!("color-alpha-{alpha}-{}",["off","middle","full"][stage])));
            if stage<2 {position_ms(&w,if stage==0 {3000.}else{5000.});s.stage+=1;} else {
                let mut metric=color_metrics(&s.frames);metric["kind"]=serde_json::json!("color");metric["requested_inactive_alpha"]=serde_json::json!(alpha);
                println!("{metric}");s.metrics.push(metric);s.frames.clear();s.case+=1;s.stage=0;
                if s.case==10 {std::fs::write(output.join("metrics.json"),serde_json::to_vec_pretty(&s.metrics).unwrap()).unwrap();slint::quit_event_loop().unwrap();} else {setup(&w,"HHHHH",4000,Some(128));}
            }
        }
    });
    w.run()?;
    let done = done.borrow();
    assert_eq!(done.case, 10);
    for metric in &done.metrics {
        if metric["kind"] == "shape" {
            for key in ["start_changed_pixels", "return_changed_pixels"] {
                assert!(
                    metric[key].as_u64().unwrap() <= 8,
                    "neutral handoff changed ink: {metric}"
                );
            }
            assert_eq!(
                metric["end_changed_pixels"], 0,
                "cue end changed ink: {metric}"
            );
        } else if metric["kind"] == "endpoint" && metric["stage"] != "wave-middle" {
            let mass = metric["ink_mass_ratio"].as_f64().unwrap();
            assert!(
                (0.97..=1.03).contains(&mass),
                "endpoint ink flashed or dimmed: {metric}"
            );
        }
    }
    Ok(())
}
