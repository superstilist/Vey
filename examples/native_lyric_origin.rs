//! Isolated proof of common-origin cluster rasterization; never overwrites artifacts.
#[path = "support/native_window.rs"]
mod native_window;
use slint::{ComponentHandle, VecModel};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

slint::slint! {
    export struct Cluster { text: string, start: float, advance: float }
    export component OriginProbe inherits Window {
        width: 640px; height: 240px; background: #101210;
        in property <string> text;
        in property <[Cluster]> clusters;
        in property <int> mode;
        in property <float> origin-x: 40.3;
        if root.mode == 0: Text {
            x: root.origin-x * 1px + 0.001phx; y: 80.2px; width: 540px; height: 80px;
            text: root.text; font-family: "Satoshi"; font-size: 48px; font-weight: 700;
            color: white; wrap: no-wrap; vertical-alignment: center;
        }
        if root.mode > 0: Rectangle {
            x: root.origin-x * 1px; y: 80.2px; width: 540px; height: 80px;
            for glyph in root.clusters: Rectangle {
                x: root.mode == 1 || root.mode == 4 || root.mode == 5 ? glyph.start * 1px : 0px;
                width: 540px; height: 80px;
                Text {
                    x: (root.mode == 4 || root.mode == 5 ? -glyph.start * 1px : 0px) + 0.001phx;
                    width: root.mode == 1 ? glyph.advance * 1px
                        : glyph.start * 1px + (root.mode == 3 || root.mode == 4 ? self.preferred-width : glyph.advance * 1px);
                    height: 80px; text: glyph.text;
                    horizontal-alignment: root.mode == 1 ? left : right;
                    font-family: "Satoshi"; font-size: 48px; font-weight: 700;
                    color: white; wrap: no-wrap; vertical-alignment: center;
                }
            }
        }
    }
}

const WORDS: [&str; 4] = ["AVATAR", "Toffee", "office", "e\u{301}cole"];
const MODES: [&str; 6] = [
    "whole",
    "independent",
    "common-harf",
    "common-preferred",
    "cancelled-parent",
    "cancelled-harf",
];
fn setup(w: &OriginProbe, index: usize) {
    let text = WORDS[index % WORDS.len()];
    // 40.4 logical pixels lands exactly on a physical half pixel at 1.25x.
    w.set_origin_x(if index < WORDS.len() { 40.3 } else { 40.4 });
    let clusters = playervf::native_lyric_fonts::letter_geometry(text, "Satoshi", 700).unwrap();
    let clusters = clusters
        .into_iter()
        .map(|c| Cluster {
            text: c.text.clone().into(),
            start: c.start_em * 48.,
            advance: playervf::native_lyric_fonts::shaped_advance_em(&c.text, "Satoshi", 700)
                .unwrap()
                * 48.,
        })
        .collect::<Vec<_>>();
    w.set_text(text.into());
    w.set_clusters(Rc::new(VecModel::from(clusters)).into());
    w.set_mode(0);
}
fn differences(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    assert_eq!(a.dimensions(), b.dimensions());
    a.pixels()
        .zip(b.pixels())
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 3))
        .count()
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
            .ok_or("Usage: native_lyric_origin NEW_OUTPUT_DIRECTORY")?,
    );
    if output.exists() {
        return Err("Output directory exists".into());
    }
    std::fs::create_dir(&output)?;
    playervf::native_lyric_fonts::register(
        include_bytes!("../assets/fonts/Satoshi-700.otf").to_vec(),
    )?;
    let w = OriginProbe::new()?;
    setup(&w, 0);
    let state = Rc::new(RefCell::new((
        0usize,
        0usize,
        0usize,
        None::<image::RgbaImage>,
        Vec::<serde_json::Value>::new(),
    )));
    let done = state.clone();
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(200),move|| {
        let w=weak.upgrade().unwrap(); if !native_window::ready(w.window()) { return; }
        w.window().request_redraw(); let mut s=state.borrow_mut();
        if s.2<5 {s.2+=1;return;}
        let f=w.window().take_snapshot().unwrap();
        let frame=image::RgbaImage::from_raw(f.width(),f.height(),f.as_bytes().to_vec()).unwrap();
        frame.save(output.join(format!("{}-{}.png",s.0,MODES[s.1]))).unwrap();
        if s.1==0 {s.3=Some(frame);} else {
            let changed=differences(s.3.as_ref().unwrap(),&frame);
            if s.1==2 || s.1==5 {assert!(changed<=8,"shared origin changed {changed} pixels");}
            let metric=serde_json::json!({"text":WORDS[s.0 % WORDS.len()],"origin_x":w.get_origin_x(),"mode":MODES[s.1],"changed_pixels":changed});
            println!("{metric}");s.4.push(metric);
        }
        s.1+=1;
        if s.1==MODES.len() {s.0+=1;s.1=0;if s.0==WORDS.len()*2{std::fs::write(output.join("metrics.json"),serde_json::to_vec_pretty(&s.4).unwrap()).unwrap();slint::quit_event_loop().unwrap();return;}setup(&w,s.0);}else{w.set_mode(s.1 as i32);}
    });
    w.run()?;
    assert_eq!(done.borrow().0, WORDS.len() * 2);
    Ok(())
}
