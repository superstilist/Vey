//! Repeatable native scene workload; records event-loop and snapshot costs, not audio FPS.
use playervf::{BraccatoWord, LyricRow, MainWindow, WordRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
struct Counted {
    inner: ModelRc<BraccatoWord>,
    reads: Rc<Cell<usize>>,
}
impl Model for Counted {
    type Data = BraccatoWord;
    fn row_count(&self) -> usize {
        self.inner.row_count()
    }
    fn row_data(&self, i: usize) -> Option<Self::Data> {
        self.reads.set(self.reads.get() + 1);
        self.inner.row_data(i)
    }
    fn model_tracker(&self) -> &dyn slint::ModelTracker {
        self.inner.model_tracker()
    }
    fn set_row_data(&self, i: usize, row: Self::Data) {
        self.inner.set_row_data(i, row);
    }
}
fn percentile(values: &mut [f64], q: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * q) as usize]
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        hook(p);
        std::process::exit(1)
    }));
    let label = std::env::args().nth(1).unwrap_or("sample".into());
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.window().set_size(slint::LogicalSize::new(1440., 900.));
    w.set_active_view("lyrics".into());
    w.set_duration(1200.);
    w.set_position(100.2);
    w.set_lyrics_timed(true);
    w.set_lyrics_word_timed(true);
    let mut style = w.get_lyric_style();
    style.wave = false;
    style.wobble = false;
    style.glow_radius = 8.;
    style.glow_color = slint::Color::from_argb_encoded(0x80ffffff);
    w.set_lyric_style(style);
    let rows = (0..300)
        .map(|line| {
            let parts = (0..8)
                .map(|word| WordRow {
                    text: format!("vocal{word} ").into(),
                    start_ms: line * 4000 + word * 500,
                    end_ms: line * 4000 + (word + 1) * 500,
                    explicit: true,
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            LyricRow {
                text: parts
                    .iter()
                    .map(|p| p.text.as_str())
                    .collect::<String>()
                    .into(),
                start_ms: line * 4000,
                end_ms: (line + 1) * 4000,
                words: Rc::new(VecModel::from(parts)).into(),
                ..Default::default()
            }
        })
        .collect::<Vec<_>>();
    w.set_lyric_lines(Rc::new(VecModel::from(rows)).into());
    let reads = Rc::new(Cell::new(0));
    let attached = Rc::new(Cell::new(false));
    let samples = Rc::new(RefCell::new(Vec::<f64>::new()));
    let snapshots = Rc::new(RefCell::new(Vec::<f64>::new()));
    let tick = Rc::new(Cell::new(0));
    let origin = Instant::now();
    let previous = Rc::new(Cell::new(0.));
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(16),move||{
        let w=weak.upgrade().unwrap();let now=origin.elapsed().as_secs_f64();
        if now<2. {return;}
        if !attached.get() {
            let rows=w.get_native_lyric_lines();assert!(rows.row_count()>300);
            for i in 0..rows.row_count() {let mut row=rows.row_data(i).unwrap();row.parts=ModelRc::new(Rc::new(Counted {inner:row.parts.clone(),reads:reads.clone()}));rows.set_row_data(i,row);}
            attached.set(true);w.set_player_state("playing".into());return;
        }
        let n=tick.get();tick.set(n+1);
        if n<15 {previous.set(now);reads.set(0);return;}
        samples.borrow_mut().push((now-previous.replace(now))*1000.);
        if n%6==0 {w.set_position(100.2+(n-15) as f32*0.016);}
        if n%4==0 {let at=Instant::now();let _=w.window().take_snapshot().unwrap();snapshots.borrow_mut().push(at.elapsed().as_secs_f64()*1000.);}
        if n==94 {
            let mut ticks=samples.borrow_mut();let mut frames=snapshots.borrow_mut();
            let report=serde_json::json!({"lines":300,"words":2400,"samples":ticks.len(),"word_reads":reads.get(),
                "tick_p50_ms":percentile(&mut ticks,0.5),"tick_p95_ms":percentile(&mut ticks,0.95),
                "snapshot_p50_ms":percentile(&mut frames,0.5),"snapshot_p95_ms":percentile(&mut frames,0.95)});
            std::fs::create_dir_all("target/native-lyrics-check").unwrap();
            std::fs::write(format!("target/native-lyrics-check/performance-{label}.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
            println!("{report}");slint::quit_event_loop().unwrap();
        }
    });
    w.run()?;
    Ok(())
}
