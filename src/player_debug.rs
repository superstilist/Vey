//! Bounded, asynchronous player timing diagnostics. Records no media metadata.
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Write},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, OnceLock,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const QUEUE_CAPACITY: usize = 2048;
static RECORDER: OnceLock<Option<Recorder>> = OnceLock::new();

#[derive(Clone)]
pub(crate) struct Recorder {
    sender: SyncSender<Sample>,
    started: Instant,
    dropped: Arc<AtomicU64>,
}

pub(crate) struct SpanGuard {
    recorder: Recorder,
    route: String,
    event: &'static str,
    started: Instant,
}

struct Sample {
    at_ms: u64,
    route: String,
    event: &'static str,
    micros: u64,
}

#[derive(Serialize)]
struct OutputSample<'a> {
    kind: &'static str,
    at_ms: u64,
    route: &'a str,
    event: &'a str,
    duration_ms: f64,
}

#[derive(Serialize)]
struct Summary<'a> {
    kind: &'static str,
    route: &'a str,
    event: &'a str,
    count: usize,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    max_ms: f64,
    over_16_7ms: usize,
    over_33ms: usize,
    over_100ms: usize,
    over_250ms: usize,
    over_500ms: usize,
    dropped_samples: u64,
}

impl Recorder {
    pub(crate) fn start() -> Option<Self> {
        RECORDER.get_or_init(Self::start_inner).clone()
    }

    fn start_inner() -> Option<Self> {
        let enabled = std::env::var_os("VEY_DEBUG_PLAYER").is_some()
            || std::env::var_os("VEY_TRACE_LYRIC_FRAMES").is_some();
        if !enabled {
            return None;
        }

        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let dropped = Arc::new(AtomicU64::new(0));
        let worker_dropped = dropped.clone();
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let path = crate::storage::data_dir()
            .join("debug")
            .join(format!("player-timing-{epoch_ms}.jsonl"));
        if let Err(error) = thread::Builder::new()
            .name("vey-player-debug-writer".into())
            .spawn(move || write_samples(receiver, worker_dropped, path))
        {
            eprintln!("[player-debug] cannot start diagnostic writer: {error}");
            return None;
        }

        let recorder = Self {
            sender,
            started: Instant::now(),
            dropped,
        };
        eprintln!("[player-debug] asynchronous timing capture enabled");
        Some(recorder)
    }

    pub(crate) fn record(&self, route: &str, event: &'static str, duration_ms: f64) {
        if !duration_ms.is_finite() || duration_ms < 0. {
            return;
        }
        let sample = Sample {
            at_ms: self.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            route: route.to_owned(),
            event,
            micros: (duration_ms * 1000.).round().min(u64::MAX as f64) as u64,
        };
        if matches!(self.sender.try_send(sample), Err(TrySendError::Full(_))) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(crate) fn record_for_window(window: &crate::MainWindow, event: &'static str, duration_ms: f64) {
    if let Some(recorder) = RECORDER.get().and_then(Option::as_ref) {
        recorder.record(
            &format!("{}/{}", window.get_active_view(), window.get_player_tab()),
            event,
            duration_ms,
        );
    }
}

pub(crate) fn begin_span(window: &crate::MainWindow, event: &'static str) -> Option<SpanGuard> {
    let recorder = RECORDER.get().and_then(Option::as_ref)?.clone();
    Some(SpanGuard {
        recorder,
        route: format!("{}/{}", window.get_active_view(), window.get_player_tab()),
        event,
        started: Instant::now(),
    })
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        self.recorder.record(
            &self.route,
            self.event,
            self.started.elapsed().as_secs_f64() * 1000.,
        );
    }
}

fn percentile(samples: &[u64], percentile: usize) -> f64 {
    if samples.is_empty() {
        return 0.;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[(sorted.len() - 1) * percentile / 100] as f64 / 1000.
}

fn write_line<T: Serialize>(writer: &mut BufWriter<File>, value: &T) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, value).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")
}

fn write_samples(receiver: Receiver<Sample>, dropped: Arc<AtomicU64>, path: std::path::PathBuf) {
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("[player-debug] cannot create diagnostic folder: {error}");
            return;
        }
    }
    let file = match File::create(&path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("[player-debug] cannot create timing report: {error}");
            return;
        }
    };
    let mut writer = BufWriter::new(file);
    let mut buckets = HashMap::<(String, &'static str), Vec<u64>>::new();
    let mut report_at = Instant::now();
    eprintln!("[player-debug] report={}", path.display());

    loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(sample) => {
                let value = sample.micros as f64 / 1000.;
                if write_line(
                    &mut writer,
                    &OutputSample {
                        kind: "sample",
                        at_ms: sample.at_ms,
                        route: &sample.route,
                        event: sample.event,
                        duration_ms: value,
                    },
                )
                .is_err()
                {
                    eprintln!("[player-debug] timing report write failed");
                    return;
                }
                buckets
                    .entry((sample.route, sample.event))
                    .or_default()
                    .push(sample.micros);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if report_at.elapsed() >= Duration::from_secs(1) {
            let dropped_samples = dropped.swap(0, Ordering::Relaxed);
            let mut dropped_left = dropped_samples;
            for ((route, event), samples) in buckets.drain() {
                let over = |limit_ms: u64| {
                    samples
                        .iter()
                        .filter(|sample| **sample > limit_ms * 1000)
                        .count()
                };
                let summary = Summary {
                    kind: "summary",
                    route: &route,
                    event,
                    count: samples.len(),
                    p50_ms: percentile(&samples, 50),
                    p95_ms: percentile(&samples, 95),
                    p99_ms: percentile(&samples, 99),
                    max_ms: percentile(&samples, 100),
                    over_16_7ms: samples.iter().filter(|sample| **sample > 16_700).count(),
                    over_33ms: over(33),
                    over_100ms: over(100),
                    over_250ms: over(250),
                    over_500ms: over(500),
                    dropped_samples: std::mem::take(&mut dropped_left),
                };
                eprintln!(
                    "[player-debug] route={} event={} n={} p50/p95/p99/max={:.2}/{:.2}/{:.2}/{:.2}ms over>16.7/33/100/250/500ms={}/{}/{}/{}/{} dropped={}",
                    summary.route, summary.event, summary.count,
                    summary.p50_ms, summary.p95_ms, summary.p99_ms, summary.max_ms,
                    summary.over_16_7ms, summary.over_33ms, summary.over_100ms,
                    summary.over_250ms, summary.over_500ms, summary.dropped_samples,
                );
                if write_line(&mut writer, &summary).is_err() {
                    eprintln!("[player-debug] timing report write failed");
                    return;
                }
            }
            if writer.flush().is_err() {
                eprintln!("[player-debug] timing report flush failed");
                return;
            }
            report_at = Instant::now();
        }
    }
    let _ = writer.flush();
}
