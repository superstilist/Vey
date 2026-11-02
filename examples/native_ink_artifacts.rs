//! Test the actual lyric ink against solid glyph references at fractional transforms.
#[path = "support/native_window.rs"]
mod native_window;
use slint::ComponentHandle;
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};
slint::slint! {
    import { LyricInk } from "../third-party/braccato-rust/crates/slint/ui/lyrics.slint";
    import "../assets/fonts/Satoshi-700.otf";
    import "../assets/fonts/NotoSans.ttf";
    export component InkProbe inherits Window {
        width: 760px; height: 240px; background: #101210;
        in property <string> text: "into";
        in property <int> mode;
        in property <float> factor: 1.025;
        in property <float> origin: 40.4;
        in property <float> phase: 0.5;
        in property <bool> stable: true;
        out property <float> advance: metrics.preferred-width / 1px;
        metrics := Text { visible: false; text: root.text; font-family: "Satoshi"; font-weight: 700; font-size: 48px; wrap: no-wrap; }
        Rectangle {
            x: root.origin * 1px; y: 60.2px; width: metrics.preferred-width; height: 90px;
            transform-origin: { x: 0px, y: 0px };
            transform-scale: root.factor;
            cache-rendering-hint: !root.stable && root.mode == 2 && root.factor != 1;
            LyricInk {
                width: parent.width; height: parent.height; text: root.text;
                font-family: "Satoshi"; font-size: 48px; font-weight: 700;
                ink: white; base-ink: #ffffff55;
                reveal: root.mode == 0 ? 0 : 1;
                sweep-start: root.mode == 1 ? 1 : root.phase - 0.05;
                sweep-end: root.mode == 1 ? 1 : root.phase + 0.05;
                native-text-gradients: true;
                stable-fill: root.stable;
            }
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1)
    }));
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("NEW_OUTPUT_DIRECTORY required"),
    );
    std::fs::create_dir(&output)?;
    let legacy = std::env::args().any(|a| a == "--legacy");
    let enforce = std::env::args().any(|a| a == "--assert");
    let w = InkProbe::new()?;
    w.set_stable(!legacy);
    let cases = [
        ("into", 1.),
        ("into", 1.025),
        ("bedroom", 1.04),
        ("nnnn", 1.011),
        ("office", 1.025),
        ("AVATAR", 1.04),
        ("літери", 1.04),
        ("e\u{301}cole", 1.025),
    ];
    let state = Rc::new(RefCell::new((
        0usize,
        0usize,
        0usize,
        Vec::<image::RgbaImage>::new(),
        Vec::<serde_json::Value>::new(),
    )));
    let result = state.clone();
    let weak = w.as_weak();
    let out = output.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(70),move||{
        let Some(w)=weak.upgrade()else{return};if !native_window::ready(w.window()){return;}
        let mut s=state.borrow_mut();s.2+=1;if s.2<4{return;}
        let case=s.0;let mode=s.1;
        if mode==0 {w.set_text(cases[case].0.into());w.set_factor(cases[case].1);w.set_mode(0);s.1=1;return;}
        let p=w.window().take_snapshot().unwrap();
        let im=image::RgbaImage::from_raw(p.width(),p.height(),p.as_bytes().to_vec()).unwrap();
        if mode==1 {s.3.push(im);w.set_mode(1);s.1=2;return;}
        if mode==2 {s.3.push(im);w.set_mode(2);w.set_phase(0.15);s.1=3;return;}
        let phase=0.15+(mode-3) as f32*0.1;
        let scale=w.window().scale_factor();let mut cores=0;let mut holes=0;let mut max_error=0f32;let mut total=0f32;
        for y in 1..im.height()-1 {for x in 1..im.width()-1 {
            if !(y-1..=y+1).all(|yy|(x-1..=x+1).all(|xx|s.3[1].get_pixel(xx,yy)[0]>245)){continue;}
            let at=((x as f32+0.5)/scale-w.get_origin())/w.get_factor()/w.get_advance();
            let coverage=((phase+0.05-at)/0.1).clamp(0.,1.);
            let base=s.3[0].get_pixel(x,y)[0] as f32;let full=s.3[1].get_pixel(x,y)[0] as f32;
            let actual=im.get_pixel(x,y)[0] as f32;
            let error=(actual-(base+(full-base)*coverage)).abs();
            max_error=max_error.max(error);total+=error;cores+=1;
            if coverage>=0.999 && actual<full-8. || coverage<=0.001 && actual>base+8. {holes+=1;}
        }}
        assert!(cores>40,"missing glyph cores");
        let metric=serde_json::json!({"case":case,"text":cases[case].0,"factor":w.get_factor(),"phase":phase,"legacy":legacy,"cores":cores,"bad_core_pixels":holes,"max_error":max_error,"mean_error":total/cores as f32});
        println!("{metric}");s.4.push(metric);
        im.save(out.join(format!("{case}-{mode}.png"))).unwrap();
        if enforce {assert_eq!(holes,0,"partial fill damaged solid glyph interiors");}
        if mode==10 {s.0+=1;s.1=0;s.3.clear();if s.0==cases.len(){slint::quit_event_loop().unwrap();return;}}
        else{s.1+=1;w.set_phase(0.15+(s.1-3)as f32*0.1);}
    });
    w.run()?;
    std::fs::write(
        output.join("metrics.json"),
        serde_json::to_vec_pretty(&result.borrow().4)?,
    )?;
    Ok(())
}
