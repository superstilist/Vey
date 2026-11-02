//! Native scroll regression: regular playback samples, actual wheel input and resume.
use playervf::{LyricRow, MainWindow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::Cell, rc::Rc, time::Duration};

struct Counted {
    rows: VecModel<LyricRow>,
    reads: Rc<Cell<usize>>,
}
impl Model for Counted {
    type Data = LyricRow;
    fn row_count(&self) -> usize {
        self.rows.row_count()
    }
    fn row_data(&self, index: usize) -> Option<LyricRow> {
        self.reads.set(self.reads.get() + 1);
        self.rows.row_data(index)
    }
    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        self.rows.model_tracker()
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1)
    }));
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_lyrics_reduced_motion(true);
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(false);
    w.set_duration(1000.);
    w.set_position(0.1);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    let reads = Rc::new(Cell::new(0));
    let rows = (0..300)
        .map(|i| LyricRow {
            text: format!("Line {} follows the actual playback clock", i + 1).into(),
            start_ms: i * 1000,
            end_ms: (i + 1) * 1000,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    w.set_lyric_lines(ModelRc::new(Rc::new(Counted {
        rows: VecModel::from(rows),
        reads: reads.clone(),
    })));
    let weak = w.as_weak();
    let tick = Rc::new(Cell::new(0));
    let result = tick.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(100),move||{
        let w=weak.upgrade().unwrap();let t=tick.get();
        if t==10 {reads.set(0);}
        if (11..30).contains(&t) {w.set_position((t-10) as f32/10.);}
        if t==30 {
            assert!(w.get_native_scroll_position()>50.,"normal samples advance the scroll target");
            assert!((w.get_native_content_y()+w.get_native_scroll_position()).abs()<1.);
            println!("source row reads during 2 seconds of playback: {}",reads.get());
            assert!(reads.get()<100,"ordinary playback must not rescan the canonical document");
            w.window().dispatch_event(slint::platform::WindowEvent::PointerScrolled {
                // Target visible lyric ink, not the padding between wrapped sources.
                position:slint::LogicalPosition::new(1100.,400.),delta_x:0.,delta_y:-180.,
            });
        }
        if t==33 {
            assert!(w.get_lyrics_user_scrolling(),"real wheel scroll pauses follow");
            w.set_position(8.1);
        }
        if t==36 {w.invoke_lyric_resume();}
        if t==39 {
            assert!(!w.get_lyrics_user_scrolling());
            assert_eq!(w.get_active_lyric_index(),8);
            assert!((w.get_native_content_y()+w.get_native_scroll_position()).abs()<1.,"resume must restore follow after real wheel input");
            let frame=w.window().take_snapshot().unwrap();
            std::fs::create_dir_all("target/native-lyrics-check").unwrap();
            image::save_buffer("target/native-lyrics-check/autoscroll-resumed.png",frame.as_bytes(),frame.width(),frame.height(),image::ColorType::Rgba8).unwrap();
            w.set_mobile(true);w.window().set_size(slint::LogicalSize::new(390.,844.));
            w.set_position(0.1);w.set_lyrics_word_timed(true);
            let parts=(0..100).map(|n|playervf::WordRow {
                text:format!("vocal{} ",n+1).into(),start_ms:n*200,end_ms:(n+1)*200,
                ..Default::default()
            }).collect::<Vec<_>>();
            let text=parts.iter().map(|p|p.text.as_str()).collect::<String>();
            w.set_lyric_lines(Rc::new(VecModel::from(vec![LyricRow {
                start_ms:0,end_ms:20000,text:text.into(),words:Rc::new(VecModel::from(parts)).into(),
                ..Default::default()
            }])).into());
        }
        if t==43 {w.set_position(12.1);}
        if t==47 {
            assert!(w.get_native_active_line()>10,"follow the sung word into its wrapped row");
            assert!(w.get_native_scroll_position()>400.);
            assert!((w.get_native_content_y()+w.get_native_scroll_position()).abs()<1.);
            let frame=w.window().take_snapshot().unwrap();
            image::save_buffer("target/native-lyrics-check/autoscroll-wrapped.png",frame.as_bytes(),frame.width(),frame.height(),image::ColorType::Rgba8).unwrap();
            w.set_mobile(false);w.window().set_size(slint::LogicalSize::new(1440.,900.));
            w.set_lyrics_scroll_offset_ms(500);w.set_lyrics_lookahead_ms(540);
            w.set_lyrics_reduced_motion(false);w.set_lyrics_word_timed(false);
            w.set_lyric_lines(Rc::new(VecModel::from(vec![
                LyricRow { text:"Hold the current line".into(),start_ms:0,end_ms:4000,..Default::default() },
                LyricRow { text:"The next line arrives before it is sung".into(),start_ms:4000,end_ms:8000,..Default::default() },
            ])).into());
            w.set_position(3.0);w.set_player_state("playing".into());
        }
        if t==48 {w.set_position(3.1);}
        if t==49 {
            assert_eq!(w.get_native_active_line(),0,"do not scroll before the bounded lead");
            w.set_position(3.65);
        }
        if t==50 {
            assert_eq!(w.get_native_active_line(),0,"optional camera lead must not advance text focus");
            assert_eq!(w.get_active_lyric_index(),0,"scroll lead must not advance the sung lyric");
            let rows=w.get_native_lyric_lines();
            assert!(rows.row_data(0).unwrap().active);
            assert!(!rows.row_data(1).unwrap().active);
            let frame=w.window().take_snapshot().unwrap();
            image::save_buffer("target/native-lyrics-check/autoscroll-early.png",frame.as_bytes(),frame.width(),frame.height(),image::ColorType::Rgba8).unwrap();
            w.set_position(4.2);
        }
        if t==54 {
            assert_eq!(w.get_active_lyric_index(),1);
            println!("native autoscroll: normal samples, actual wheel, resume, wrapped cues and early handoff passed");
            slint::quit_event_loop().unwrap();
        }
        tick.set(t+1);
    });
    w.run()?;
    assert_eq!(result.get(), 55);
    Ok(())
}
