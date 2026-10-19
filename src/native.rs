//! PlayerVF native audio core.
//!
//! rodio's OutputStream is !Send on Windows (WASAPI), so we spawn a dedicated
//! audio thread that owns OutputStream + Sink, and all commands are sent via
//! a channel. Tauri state holds only the channel sender + a Mutex for status
//! queries, both of which are Send + Sync.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use lofty::prelude::*;
use lofty::probe::Probe;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use walkdir::WalkDir;

const AUDIO_EXTS: &[&str] = &[
    "aif", "aiff", "caf", "flac", "m4a", "m4b", "mka", "mkv", "mov", "mp1", "mp2", "mp3", "mp4",
    "oga", "ogg", "opus", "wav",
];

// ── Shared types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackMeta {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<i32>,
    pub track_no: Option<u32>,
    pub duration_secs: f64,
    pub file_size: u64,
    pub format: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibrarySourceKind {
    System,
    Folder,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySource {
    pub id: String,
    pub kind: LibrarySourceKind,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceScanState {
    Ready,
    Empty,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceScanStatus {
    pub source_id: String,
    pub state: SourceScanState,
    pub track_count: usize,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanResult {
    pub tracks: Vec<TrackMeta>,
    pub source_statuses: Vec<SourceScanStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatus {
    pub state: PlaybackState,
    pub current_path: Option<String>,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
    pub rate: f32,
    pub ended: bool,
}

// ── Audio thread commands ─────────────────────────────────────────────────────

enum AudioCmd {
    Load {
        path: String,
        autoplay: bool,
        reply: std::sync::mpsc::Sender<Result<(), String>>,
    },
    Play,
    Pause,
    Stop,
    Seek(f64),
    Volume(f32),
    Rate(f32),
}

// ── Shared status (audio thread writes, command handlers read) ────────────────

#[derive(Debug, Default, Clone)]
struct SharedStatus {
    state: PlaybackState,
    current_path: Option<String>,
    position_secs: f64,
    duration_secs: f64,
    volume: f32,
    rate: f32,
    ended: bool,
}

impl Default for PlaybackState {
    fn default() -> Self {
        PlaybackState::Stopped
    }
}

// ── Tauri-visible state (Send + Sync) ─────────────────────────────────────────

/// Instantaneous playback loudness for the ambient background. Written by
/// the audio thread as samples flow, read (with expiry) by `player_level`.
#[derive(Debug, Default)]
struct LevelMeter {
    value: f32,
    updated: Option<std::time::Instant>,
}

pub struct AudioHandle {
    tx: std::sync::mpsc::Sender<AudioCmd>,
    status: Arc<Mutex<SharedStatus>>,
    level: Arc<Mutex<LevelMeter>>,
}

// SAFETY: tx is Send; Arc<Mutex<>> is Send+Sync.
unsafe impl Send for AudioHandle {}
unsafe impl Sync for AudioHandle {}

pub type CoreState = Mutex<AudioHandle>;

// ── Audio thread ──────────────────────────────────────────────────────────────

type AudioSource = Box<dyn rodio::Source + Send>;

/// Samples averaged per window before folding into the meter.
const METER_WINDOW_SAMPLES: u32 = 2048;

/// Wraps any decoded source and taps its samples into a shared loudness
/// meter (RMS per window, fast attack / slow release). Pass-through for
/// everything the sink needs, so decoding behaviour is unchanged.
struct MeteredSource {
    inner: AudioSource,
    meter: Arc<Mutex<LevelMeter>>,
    sum_sq: f32,
    count: u32,
}

impl MeteredSource {
    fn new(inner: AudioSource, meter: Arc<Mutex<LevelMeter>>) -> Self {
        MeteredSource {
            inner,
            meter,
            sum_sq: 0.0,
            count: 0,
        }
    }

    fn flush(&mut self) {
        if self.count == 0 {
            return;
        }
        let rms = (self.sum_sq / self.count as f32).sqrt();
        self.sum_sq = 0.0;
        self.count = 0;
        if let Ok(mut meter) = self.meter.lock() {
            let target = (rms * 3.0).clamp(0.0, 1.0);
            meter.value = (meter.value * 0.65 + target * 0.35).clamp(0.0, 1.0);
            meter.updated = Some(std::time::Instant::now());
        }
    }
}

impl Iterator for MeteredSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let sample = if sample.is_finite() { sample } else { 0.0 };
        self.sum_sq += sample * sample;
        self.count += 1;
        if self.count >= METER_WINDOW_SAMPLES {
            self.flush();
        }
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl rodio::Source for MeteredSource {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.sum_sq = 0.0;
        self.count = 0;
        self.inner.try_seek(pos)
    }
}

fn track_label(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

fn decode_track(path: &str) -> Result<(AudioSource, f64), String> {
    use rodio::Source;

    let file =
        File::open(path).map_err(|error| format!("Cannot open {}: {error}", track_label(path)))?;
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Streaming buffered decoder with 64KB read chunks instead of loading whole file into memory
    let buffered = BufReader::with_capacity(64 * 1024, file);

    // Some malformed media makes third-party demuxers panic while probing. Keep
    // the audio service alive and return an actionable error to the player UI.
    let native_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut builder = rodio::Decoder::builder()
            .with_data(buffered)
            .with_seekable(true)
            .with_gapless(true);
        if len > 0 {
            builder = builder.with_byte_len(len);
        }
        if !ext.is_empty() {
            builder = builder.with_hint(&ext);
        }
        builder.build()
    }));
    match native_result {
        Ok(Ok(decoder)) => {
            let duration_secs = decoder
                .total_duration()
                .map(|duration| duration.as_secs_f64())
                .unwrap_or(0.0);
            return Ok((Box::new(decoder), duration_secs));
        }
        Ok(Err(error)) => {
            let opus_error = error.to_string();
            return decode_opus_track(path).map_err(|fallback_error| {
                format!(
                    "Cannot decode {}: {opus_error}; Opus fallback: {fallback_error}",
                    track_label(path)
                )
            });
        }
        Err(_) => {
            return decode_opus_track(path).map_err(|fallback_error| {
                format!(
                    "Cannot decode {}: invalid audio; Opus fallback: {fallback_error}",
                    track_label(path)
                )
            });
        }
    }
}

/// Packet-by-packet Opus stream: decodes on demand as rodio pulls samples,
/// so playback starts instantly instead of after a full-file pre-decode.
/// The old fallback decoded the entire file into a `SamplesBuffer` up front,
/// which blocked `player_load` for seconds on long tracks and spiked memory.
struct OpusStreamSource {
    reader: opus_pure::OggOpusReader<BufReader<File>>,
    decoder: opus_pure::OpusDecoder,
    trim: opus_pure::Trim,
    block: Vec<f32>,
    pending: std::collections::VecDeque<f32>,
    channels: usize,
    duration: Duration,
    path: PathBuf,
    finished: bool,
}

impl OpusStreamSource {
    fn open(path: &Path, duration_secs: f64) -> Result<Self, String> {
        let file = File::open(path).map_err(|error| error.to_string())?;
        let reader = opus_pure::OggOpusReader::new(BufReader::with_capacity(64 * 1024, file))
            .map_err(|error| error.to_string())?;
        let head = reader.head().clone();
        let channels = usize::from(head.channel_count);
        if channels == 0 {
            return Err("the Opus stream declares zero channels".into());
        }
        let decoder = head.decoder(48_000).map_err(|error| error.to_string())?;
        let trim =
            opus_pure::Trim::new(&head, 48_000, channels).map_err(|error| error.to_string())?;
        let block = vec![0.0f32; opus_pure::MAX_PACKET_SAMPLES * channels];
        let duration = Duration::from_secs_f64(duration_secs.max(0.0)) + Duration::from_nanos(1);
        let mut source = OpusStreamSource {
            reader,
            decoder,
            trim,
            block,
            pending: std::collections::VecDeque::new(),
            channels,
            duration,
            path: path.to_path_buf(),
            finished: false,
        };
        // Validate eagerly: a stream with zero decodable audio must fail load
        // (preserving the old fallback's contract) rather than play silence.
        if !source.decode_next_packet() && source.pending.is_empty() {
            return Err("the Opus stream contains no playable audio".into());
        }
        Ok(source)
    }

    /// Decode one packet into `pending`. Returns false on end of stream.
    /// Corrupt packets are skipped so one bad frame cannot kill playback.
    fn decode_next_packet(&mut self) -> bool {
        if self.finished {
            return false;
        }
        let packet = match self.reader.read_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) | Err(_) => {
                self.finished = true;
                return false;
            }
        };
        let decoded =
            match self
                .decoder
                .decode(&packet.data, opus_pure::MAX_PACKET_SAMPLES, &mut self.block)
            {
                Ok(decoded) => decoded,
                Err(_) => return true, // skip corrupt packet, stream continues
            };
        let end = decoded.saturating_mul(self.channels);
        if end <= self.block.len() {
            let kept = self.trim.keep(&packet, &self.block[..end]);
            self.pending.extend(kept.iter().copied());
        }
        true
    }
}

impl Iterator for OpusStreamSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(sample) = self.pending.pop_front() {
                return Some(sample);
            }
            if !self.decode_next_packet() {
                return self.pending.pop_front();
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.pending.len(), None)
    }
}

impl rodio::Source for OpusStreamSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels as u16
    }

    fn sample_rate(&self) -> u32 {
        48_000
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        use rodio::source::SeekError;
        let fail = |message: String| {
            SeekError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                message,
            )))
        };
        let target_frame = (pos.as_secs_f64() * 48_000.0).max(0.0) as u64;
        // Opus decoder state is sequential history: rebuild from the file head
        // and decode-and-discard forward to the target frame. `pending` keeps
        // every decoded sample, so after the loop it is trimmed down to the
        // overshoot tail past the target.
        let duration_secs = self.duration.as_secs_f64();
        let mut fresh = OpusStreamSource::open(&self.path.clone(), duration_secs).map_err(fail)?;
        let mut guard = 0usize;
        while fresh.trim.samples_emitted() < target_frame {
            if !fresh.decode_next_packet() {
                break; // end of stream: saturate
            }
            guard += 1;
            if guard > 1_000_000 {
                return Err(fail("seek did not converge".to_string()));
            }
        }
        let emitted = fresh.trim.samples_emitted();
        if emitted > target_frame {
            let keep_values = (emitted - target_frame) as usize * fresh.channels;
            let drop_values = fresh.pending.len().saturating_sub(keep_values);
            fresh.pending.drain(..drop_values);
        } else {
            // Seek at/past the end: nothing left to play.
            fresh.pending.clear();
            fresh.finished = true;
        }
        *self = fresh;
        Ok(())
    }
}

/// Duration without decoding: the last page's granule minus the pre-skip is
/// the playable length (RFC 7845 §4.4). Page-header IO only — milliseconds
/// even for long files, versus seconds for a full decode.
fn opus_duration_secs(path: &Path) -> f64 {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return 0.0,
    };
    let mut reader = match opus_pure::OggOpusReader::new(BufReader::with_capacity(64 * 1024, file))
    {
        Ok(reader) => reader,
        Err(_) => return 0.0,
    };
    let pre_skip = u64::from(reader.head().pre_skip);
    let mut last_granule: Option<u64> = None;
    loop {
        match reader.read_packet() {
            Ok(Some(packet)) => {
                if packet.page_granule >= 0 {
                    last_granule = Some(packet.page_granule as u64);
                }
            }
            _ => break,
        }
    }
    match last_granule {
        Some(granule) => granule.saturating_sub(pre_skip) as f64 / 48_000.0,
        None => 0.0,
    }
}

fn decode_opus_track(path: &str) -> Result<(AudioSource, f64), String> {
    let fs_path = Path::new(path);
    let duration_secs = opus_duration_secs(fs_path);
    let source = OpusStreamSource::open(fs_path, duration_secs)?;
    let duration_secs = if duration_secs > 0.0 {
        duration_secs
    } else {
        source.duration.as_secs_f64()
    };
    Ok((Box::new(source), duration_secs))
}

fn audio_thread(
    rx: std::sync::mpsc::Receiver<AudioCmd>,
    status_arc: Arc<Mutex<SharedStatus>>,
    level_arc: Arc<Mutex<LevelMeter>>,
) {
    use rodio::{OutputStreamBuilder, Sink};

    let stream = match OutputStreamBuilder::open_default_stream() {
        Ok(stream) => stream,
        Err(_) => return,
    };
    let mut sink = Sink::connect_new(stream.mixer());
    let mut volume: f32 = 1.0;
    let mut rate: f32 = 1.0;
    let mut current_path: Option<String> = None;
    let mut duration_secs: f64 = 0.0;

    loop {
        // Wake periodically so the UI receives a new position while audio plays.
        let command = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(command) => Some(command),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };

        if let Some(cmd) = command {
            match cmd {
                AudioCmd::Load {
                    path,
                    autoplay,
                    reply,
                } => {
                    let result = match decode_track(&path) {
                        Ok((source, track_duration_secs)) => {
                            // Replace the sink only after decoding succeeds, so a bad file
                            // cannot interrupt the song that was already playing.
                            sink.stop();
                            let next_sink = Sink::connect_new(stream.mixer());
                            next_sink.append(MeteredSource::new(source, Arc::clone(&level_arc)));
                            next_sink.set_volume(volume);
                            next_sink.set_speed(rate);
                            if autoplay {
                                next_sink.play();
                            } else {
                                next_sink.pause();
                            }
                            sink = next_sink;
                            current_path = Some(path);
                            duration_secs = track_duration_secs;
                            Ok(())
                        }
                        Err(error) => Err(error),
                    };
                    let _ = reply.send(result);
                }
                AudioCmd::Play => sink.play(),
                AudioCmd::Pause => sink.pause(),
                AudioCmd::Stop => {
                    sink.stop();
                    sink.clear();
                    current_path = None;
                    duration_secs = 0.0;
                }
                AudioCmd::Seek(secs) => {
                    let _ = sink.try_seek(Duration::from_secs_f64(secs.max(0.0)));
                }
                AudioCmd::Volume(v) => {
                    volume = v.clamp(0.0, 1.0);
                    sink.set_volume(volume);
                }
                AudioCmd::Rate(r) => {
                    rate = r.clamp(0.5, 2.0);
                    sink.set_speed(rate);
                }
            }
        }

        // Update shared status
        let empty = sink.empty();
        let state = if empty || (current_path.is_none()) {
            PlaybackState::Stopped
        } else if sink.is_paused() {
            PlaybackState::Paused
        } else {
            PlaybackState::Playing
        };
        let pos = sink.get_pos().as_secs_f64();
        let pos = if duration_secs > 0.0 {
            pos.min(duration_secs)
        } else {
            pos
        };
        let ended = !empty
            && state == PlaybackState::Playing
            && pos >= duration_secs
            && duration_secs > 0.0;
        let ended = ended || (current_path.is_some() && empty);

        if let Ok(mut s) = status_arc.lock() {
            s.state = state;
            s.current_path = current_path.clone();
            s.position_secs = pos;
            s.duration_secs = duration_secs;
            s.volume = volume;
            s.rate = rate;
            s.ended = ended;
        }
    }
}

// ── Library helpers ────────────────────────────────────────────────────────────

fn read_track(path: &Path) -> Option<TrackMeta> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if !AUDIO_EXTS.contains(&ext.as_str()) {
        return None;
    }
    // Metadata is optional. A playable file without readable tags still belongs
    // in the library and will use the filename plus fallback labels.
    let tagged = Probe::open(path).ok().and_then(|probe| probe.read().ok());
    let (title, artist, album, album_artist, genre, year, track_no, duration) = match tagged {
        Some(tf) => {
            let props = tf.properties();
            let dur = props.duration().as_secs_f64();
            let tag = tf.primary_tag().or_else(|| tf.first_tag());
            match tag {
                Some(t) => (
                    t.title().as_deref().unwrap_or("").to_string(),
                    t.artist()
                        .as_deref()
                        .unwrap_or("Unknown artist")
                        .to_string(),
                    t.album().as_deref().unwrap_or("Unknown album").to_string(),
                    t.get_string(&lofty::tag::ItemKey::AlbumArtist)
                        .unwrap_or("Unknown artist")
                        .to_string(),
                    t.genre().as_deref().unwrap_or("").to_string(),
                    t.year(),
                    t.track(),
                    dur,
                ),
                None => (
                    String::new(),
                    "Unknown artist".into(),
                    "Unknown album".into(),
                    "Unknown artist".into(),
                    String::new(),
                    None,
                    None,
                    dur,
                ),
            }
        }
        None => (
            String::new(),
            "Unknown artist".into(),
            "Unknown album".into(),
            "Unknown artist".into(),
            String::new(),
            None,
            None,
            0.0,
        ),
    };
    let stem = path.file_stem()?.to_str().unwrap_or("Unknown title");
    let title = if title.trim().is_empty() {
        stem.to_string()
    } else {
        title
    };
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Some(TrackMeta {
        path: path.to_string_lossy().to_string(),
        title,
        artist,
        album,
        album_artist,
        genre,
        year: year.map(|y| y as i32),
        track_no,
        duration_secs: duration,
        file_size,
        format: ext,
    })
}

// ── Tauri commands: library ────────────────────────────────────────────────────

fn unavailable_status(source: &LibrarySource, detail: impl Into<String>) -> SourceScanStatus {
    SourceScanStatus {
        source_id: source.id.clone(),
        state: SourceScanState::Unavailable,
        track_count: 0,
        detail: Some(detail.into()),
    }
}

fn canonical_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn add_track(path: &Path, seen: &mut HashSet<PathBuf>, tracks: &mut Vec<TrackMeta>) -> bool {
    let canonical = canonical_path(path);
    let Some(track) = read_track(&canonical) else {
        return false;
    };
    if !seen.insert(canonical) {
        return false;
    }
    tracks.push(track);
    true
}

fn scan_file_source(
    source: &LibrarySource,
    path: &Path,
    seen: &mut HashSet<PathBuf>,
    tracks: &mut Vec<TrackMeta>,
) -> SourceScanStatus {
    match std::fs::metadata(path) {
        Err(error) => return unavailable_status(source, format!("Unable to access file: {error}")),
        Ok(metadata) if !metadata.is_file() => {
            return unavailable_status(source, "This source is no longer a file");
        }
        Ok(_) => {}
    }

    let track_count = usize::from(add_track(path, seen, tracks));
    SourceScanStatus {
        source_id: source.id.clone(),
        state: if track_count == 0 {
            SourceScanState::Empty
        } else {
            SourceScanState::Ready
        },
        track_count,
        detail: if track_count == 0 {
            Some("No supported audio file found".into())
        } else {
            None
        },
    }
}

fn scan_folder_source(
    source: &LibrarySource,
    root: &Path,
    seen: &mut HashSet<PathBuf>,
    tracks: &mut Vec<TrackMeta>,
) -> SourceScanStatus {
    match std::fs::metadata(root) {
        Err(error) => {
            return unavailable_status(source, format!("Unable to access folder: {error}"))
        }
        Ok(metadata) if !metadata.is_dir() => {
            return unavailable_status(source, "This source is no longer a folder");
        }
        Ok(_) => {}
    }

    let mut track_count = 0;
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if entry.file_type().is_file() && add_track(entry.path(), seen, tracks) {
            track_count += 1;
        }
    }

    SourceScanStatus {
        source_id: source.id.clone(),
        state: if track_count == 0 {
            SourceScanState::Empty
        } else {
            SourceScanState::Ready
        },
        track_count,
        detail: if track_count == 0 {
            Some("No supported audio files found".into())
        } else {
            None
        },
    }
}

fn scan_sources(
    sources: Vec<LibrarySource>,
    system_music_folder: Option<PathBuf>,
) -> LibraryScanResult {
    let mut tracks = Vec::new();
    let mut source_statuses = Vec::new();
    let mut seen = HashSet::new();

    // System Music always establishes the primary catalog before custom locations.
    for source in sources
        .iter()
        .filter(|source| source.kind == LibrarySourceKind::System)
        .chain(
            sources
                .iter()
                .filter(|source| source.kind != LibrarySourceKind::System),
        )
    {
        let status = match source.kind {
            LibrarySourceKind::System => match system_music_folder.as_deref() {
                Some(path) => scan_folder_source(source, path, &mut seen, &mut tracks),
                None => unavailable_status(source, "System Music could not be resolved"),
            },
            LibrarySourceKind::Folder => {
                scan_folder_source(source, Path::new(&source.path), &mut seen, &mut tracks)
            }
            LibrarySourceKind::File => {
                scan_file_source(source, Path::new(&source.path), &mut seen, &mut tracks)
            }
        };
        source_statuses.push(status);
    }

    tracks.sort_by(|a, b| {
        a.artist
            .to_lowercase()
            .cmp(&b.artist.to_lowercase())
            .then(a.album.to_lowercase().cmp(&b.album.to_lowercase()))
            .then(a.track_no.unwrap_or(999).cmp(&b.track_no.unwrap_or(999)))
    });
    LibraryScanResult {
        tracks,
        source_statuses,
    }
}

fn sources_with_system_music(sources: Vec<LibrarySource>) -> Vec<LibrarySource> {
    let mut normalized = Vec::with_capacity(sources.len() + 1);
    normalized.push(LibrarySource {
        id: "system-music".into(),
        kind: LibrarySourceKind::System,
        path: String::new(),
    });

    // The native scanner owns the System Music default. Caller-provided system
    // records cannot duplicate it, move it, or remove it from a scan.
    normalized.extend(
        sources
            .into_iter()
            .filter(|source| source.kind != LibrarySourceKind::System),
    );
    normalized
}

pub fn get_system_music_folder() -> Option<String> {
    if let Some(path) = std::env::var_os("PLAYERVF_MUSIC_DIR") {
        return Some(PathBuf::from(path).to_string_lossy().into_owned());
    }

    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(|p| {
            PathBuf::from(p)
                .join("Music")
                .to_string_lossy()
                .into_owned()
        })
}
pub fn scan_library(sources: Vec<LibrarySource>) -> LibraryScanResult {
    scan_sources(
        sources_with_system_music(sources),
        get_system_music_folder().map(PathBuf::from),
    )
}

pub fn get_artwork(path: String) -> Option<String> {
    let tf = Probe::open(&path).ok()?.read().ok()?;
    let tag = tf.primary_tag().or_else(|| tf.first_tag())?;
    let pic = tag.pictures().first()?;
    let mime = pic.mime_type().map(|m| m.as_str()).unwrap_or("image/jpeg");
    Some(format!("data:{mime};base64,{}", B64.encode(pic.data())))
}

// ── Tauri commands: playback ───────────────────────────────────────────────────

pub fn player_load(state: &CoreState, path: String, autoplay: bool) -> Result<(), String> {
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    let tx = {
        let h = state.lock().map_err(|e| e.to_string())?;
        h.tx.clone()
    };

    tx.send(AudioCmd::Load {
        path,
        autoplay,
        reply: reply_tx,
    })
    .map_err(|_| "Audio service is unavailable".to_string())?;
    // Decoding a freshly downloaded YouTube Music m4a (tens of MB) plus the
    // symphonia probe can outlast 5s on a cold cache — don't report a failure
    // for a load that is still progressing.
    reply_rx
        .recv_timeout(Duration::from_secs(20))
        .map_err(|_| "Audio service did not confirm the track load".to_string())?
}

pub fn player_play(state: &CoreState) -> Result<(), String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    h.tx.send(AudioCmd::Play).map_err(|e| e.to_string())
}

pub fn player_pause(state: &CoreState) -> Result<(), String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    h.tx.send(AudioCmd::Pause).map_err(|e| e.to_string())
}

pub fn player_toggle(state: &CoreState) -> Result<String, String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    let s = h.status.lock().map_err(|e| e.to_string())?;
    let (cmd, next) = if s.state == PlaybackState::Playing {
        (AudioCmd::Pause, "paused")
    } else {
        (AudioCmd::Play, "playing")
    };
    drop(s);
    h.tx.send(cmd).map_err(|e| e.to_string())?;
    Ok(next.to_string())
}

pub fn player_stop(state: &CoreState) -> Result<(), String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    h.tx.send(AudioCmd::Stop).map_err(|e| e.to_string())
}

pub fn player_seek(state: &CoreState, secs: f64) -> Result<(), String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    h.tx.send(AudioCmd::Seek(secs)).map_err(|e| e.to_string())
}

pub fn player_volume(state: &CoreState, volume: f32) -> Result<(), String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    h.tx.send(AudioCmd::Volume(volume))
        .map_err(|e| e.to_string())
}

pub fn player_rate(state: &CoreState, rate: f32) -> Result<(), String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    h.tx.send(AudioCmd::Rate(rate)).map_err(|e| e.to_string())
}

pub fn player_status(state: &CoreState) -> Result<PlayerStatus, String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    let s = h.status.lock().map_err(|e| e.to_string())?;
    Ok(PlayerStatus {
        state: s.state,
        current_path: s.current_path.clone(),
        position_secs: s.position_secs,
        duration_secs: s.duration_secs,
        volume: s.volume,
        rate: s.rate,
        ended: s.ended,
    })
}

pub fn player_ended(state: &CoreState) -> Result<bool, String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    let s = h.status.lock().map_err(|e| e.to_string())?;
    Ok(s.ended)
}

/// Current playback loudness 0..1 for the ambient background. Stale
/// readings expire fast so the glow calms ~0.3s after pause/stop.

pub fn player_level(state: &CoreState) -> Result<f32, String> {
    let h = state.lock().map_err(|e| e.to_string())?;
    let meter = h.level.lock().map_err(|e| e.to_string())?;
    let age = meter
        .updated
        .map(|t| t.elapsed().as_secs_f32())
        .unwrap_or(10.0);
    Ok((meter.value * (-age * 9.0).exp()).clamp(0.0, 1.0))
}

// ── Dacubeking cf-api v2 proxy ───────────────────────────────────────────────
// Spoofs Origin https://music.youtube.com (required CORS origin) and carries
// cf_clearance cookies automatically via reqwest cookie store.

fn lyrics_http_client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> =
        std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .cookie_store(true)
                .timeout(Duration::from_secs(18))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub async fn verify_turnstile(token: String, base_url: Option<String>) -> Result<String, String> {
    let base = base_url.unwrap_or_else(|| "https://lyrics.api.dacubeking.com".to_string());
    let url = format!("{}/verify-turnstile", base.trim_end_matches('/'));
    let client = lyrics_http_client()?;
    let res = client
        .post(&url)
        .header("Origin", "https://music.youtube.com")
        .header("Referer", "https://music.youtube.com/")
        .json(&serde_json::json!({ "token": token }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("verify-turnstile HTTP {}", res.status()));
    }
    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let jwt = data
        .get("jwt")
        .or_else(|| data.get("token"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if jwt.is_empty() {
        return Err("verify-turnstile returned no jwt".to_string());
    }
    Ok(jwt)
}

pub async fn fetch_lyrics_get(
    query_params: std::collections::HashMap<String, String>,
    base_url: Option<String>,
) -> Result<Option<String>, String> {
    // cf-api v2: POST {base}/v2/lyrics with a form body
    // (videoId, song, artist, duration, album, alwaysFetchMetadata, token).
    // The legacy boidu.dev GET /getLyrics?s=&a=&d=&al= endpoint now answers
    // 401, so old base URLs are routed to the v2 host. Origin/Referer
    // https://music.youtube.com is required (v2 CORS only allows that origin).
    let base = base_url.unwrap_or_else(|| "https://lyrics.api.dacubeking.com".to_string());
    let base = base.trim_end_matches('/');
    let url = if base.contains("boidu.dev") {
        "https://lyrics.api.dacubeking.com/v2/lyrics".to_string()
    } else if base.ends_with("/v2/lyrics") {
        base.to_string()
    } else {
        format!("{base}/v2/lyrics")
    };
    let client = lyrics_http_client()?;
    // Hand-rolled urlencoding (no `form` feature on reqwest): the v2 API
    // expects application/x-www-form-urlencoded, same shape as the browser
    // URLSearchParams body.
    let mut pairs: Vec<(&str, &str)> = Vec::with_capacity(query_params.len());
    for (key, value) in query_params.iter() {
        pairs.push((key.as_str(), value.as_str()));
    }
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let body = pairs
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding_fallback(key),
                urlencoding_fallback(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    let res = client
        .post(&url)
        .header("Origin", "https://music.youtube.com")
        .header("Referer", "https://music.youtube.com/")
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded;charset=UTF-8",
        )
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status().as_u16();
    if status == 401 {
        return Err("v2/lyrics HTTP 401".into());
    }
    if status == 404 || status == 204 {
        return Ok(None);
    }
    if !res.status().is_success() {
        return Err(format!("v2/lyrics HTTP {}", res.status()));
    }
    let body = res.text().await.map_err(|e| e.to_string())?;
    let trimmed = body.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "{}" {
        return Ok(None);
    }
    Ok(Some(body))
}

// ── YouTube captions (independent fallback à la upstream yt-captions) ────────
// The watch page embeds captionTracks JSON; each track's baseUrl serves
// timedtext in several formats. json3 is the cheapest to parse natively.

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionLine {
    pub start_ms: u64,
    pub dur_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeCaptions {
    pub track_name: String,
    pub language: String,
    pub kind: String,
    pub lines: Vec<CaptionLine>,
}

/// Extract a JSON array/object starting at `from` (must point at `[`/`{`),
/// respecting strings and backslash escapes.
fn extract_json_bracket(html: &str, from: usize) -> Option<String> {
    let bytes = html.as_bytes();
    if from >= bytes.len() {
        return None;
    }
    let open = bytes[from];
    let close = if open == b'[' {
        b']'
    } else if open == b'{' {
        b'}'
    } else {
        return None;
    };
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escape = false;
    let mut i = from;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                // from/i both sit on ASCII brackets: char boundaries are safe.
                return Some(html[from..=i].to_string());
            }
        }
        i += 1;
    }
    None
}

fn extract_caption_tracks(html: &str) -> Option<Vec<serde_json::Value>> {
    let marker = "\"captionTracks\":";
    let pos = html.find(marker)?;
    let rest = &html[pos + marker.len()..];
    let trimmed = rest.trim_start();
    if !trimmed.starts_with('[') {
        return None;
    }
    let abs = pos + marker.len() + (rest.len() - trimmed.len());
    let raw = extract_json_bracket(html, abs)?;
    serde_json::from_str::<Vec<serde_json::Value>>(&raw).ok()
}

fn caption_track_str<'a>(track: &'a serde_json::Value, key: &str) -> &'a str {
    track.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn caption_track_name(track: &serde_json::Value) -> String {
    if let Some(simple) = track
        .get("name")
        .and_then(|n| n.get("simpleText"))
        .and_then(|s| s.as_str())
    {
        return simple.to_string();
    }
    track
        .get("name")
        .and_then(|n| n.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|runs| runs.first())
        .and_then(|run| run.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string()
}

/// Pick (base_url, name, language, is_asr): manual tracks first, preferred
/// language first, auto-generated ("asr") only as a last resort.
fn pick_caption_track(
    tracks: &[serde_json::Value],
    prefer_lang: Option<&str>,
) -> Option<(String, String, String, bool)> {
    let want = prefer_lang.unwrap_or("en").to_lowercase();
    let mut entries: Vec<(String, String, String, bool)> = Vec::new();
    for track in tracks {
        let url = caption_track_str(track, "baseUrl");
        if url.is_empty() {
            continue;
        }
        let lang = caption_track_str(track, "languageCode").to_string();
        let asr = caption_track_str(track, "kind") == "asr";
        entries.push((url.to_string(), caption_track_name(track), lang, asr));
    }
    let by = |asr: bool, lang_match: bool| {
        entries
            .iter()
            .find(|entry| entry.3 == asr && (!lang_match || entry.2.to_lowercase() == want))
    };
    by(false, true)
        .or_else(|| by(false, false))
        .or_else(|| by(true, true))
        .or_else(|| by(true, false))
        .cloned()
}

fn json_u64(value: Option<&serde_json::Value>) -> u64 {
    match value {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.trim().parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

fn parse_json3_captions(body: &str) -> Vec<CaptionLine> {
    let parsed: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let events = parsed
        .get("events")
        .and_then(|events| events.as_array())
        .cloned()
        .unwrap_or_default();
    let mut lines = Vec::new();
    for event in &events {
        if lines.len() >= 1500 {
            break;
        }
        let text: String = event
            .get("segs")
            .and_then(|segs| segs.as_array())
            .map(|segs| {
                segs.iter()
                    .filter_map(|seg| seg.get("utf8").and_then(|text| text.as_str()))
                    .collect::<String>()
            })
            .unwrap_or_default()
            .replace('\n', " ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            continue;
        }
        lines.push(CaptionLine {
            start_ms: json_u64(event.get("tStartMs")),
            dur_ms: json_u64(event.get("dDurationMs")),
            text,
        });
    }
    lines
}

pub async fn fetch_youtube_captions(
    video_id: String,
    lang: Option<String>,
) -> Result<Option<YoutubeCaptions>, String> {
    let id = video_id.trim().to_string();
    if id.len() != 11
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Ok(None);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let watch = client
        .get(format!("https://www.youtube.com/watch?v={id}"))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !watch.status().is_success() {
        return Ok(None);
    }
    let html = watch.text().await.map_err(|e| e.to_string())?;
    let tracks = extract_caption_tracks(&html).unwrap_or_default();
    if tracks.is_empty() {
        return Ok(None);
    }
    let Some((base_url, name, language, asr)) = pick_caption_track(&tracks, lang.as_deref()) else {
        return Ok(None);
    };
    let separator = if base_url.contains('?') { "&" } else { "?" };
    let captions = client
        .get(format!("{base_url}{separator}fmt=json3"))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !captions.status().is_success() {
        return Ok(None);
    }
    let body = captions.text().await.map_err(|e| e.to_string())?;
    let lines = parse_json3_captions(&body);
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(YoutubeCaptions {
        track_name: name,
        language,
        kind: if asr {
            "asr".to_string()
        } else {
            "standard".to_string()
        },
        lines,
    }))
}

// ── YouTube Music songs + streams (get/play only) ───────────────────────────
// Ported from yt_music_api (search/get-players; charts/albums/playlists/
// lyrics/download-to-disk deliberately excluded). No new dependencies.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmBrowseItem {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Square album-art crop (preferred still for "cover" view).
    #[serde(default)]
    pub thumbnail_url: String,
    /// Wide 16:9 video still for the same videoId (preferred for "clip" view).
    /// Empty when InnerTube only offered square crops — the UI then falls back
    /// to `i.ytimg.com/vi/<id>/maxresdefault.jpg` for the videoId it already has.
    #[serde(default)]
    pub clip_url: String,
    /// Music video to show for the clip view, when paired during search.
    #[serde(default)]
    pub clip_video_id: String,
    pub duration_secs: u64,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmSong {
    pub video_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Square album-art crop (preferred still for "cover" view).
    pub thumbnail_url: String,
    /// Wide 16:9 video still (preferred for "clip" view). See `YtmBrowseItem`.
    pub clip_url: String,
    /// What the YT Music shelf this row came from actually was: `song` |
    /// `video` | `album` | `playlist` | `artist` | `top` | `other`.
    pub kind: String,
    /// Raw InnerTube `videoType` (e.g. `MUSIC_VIDEO_TYPE_ATV` for an audio-only
    /// track, `MUSIC_VIDEO_TYPE_OMV` for an official music video).
    pub video_type: String,
    /// True only when YouTube says there is a real video behind this id, or a
    /// music video was paired to it (see `pair_song_clips`). This is what the
    /// UI uses to decide whether the "clip" control is meaningful.
    pub has_video: bool,
    /// Music video to show for the clip view, when it differs from
    /// `video_id`. A song is played from its own audio id while the clip may
    /// come from the matching official video found in the same search.
    #[serde(default)]
    pub clip_video_id: String,
    pub duration_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmStream {
    pub url: String,
    pub mime_type: String,
    pub bitrate: u64,
    pub duration_secs: u64,
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmCachedTrack {
    pub path: String,
    pub duration_secs: u64,
    pub format: String,
    pub title: String,
    pub artist: String,
}

#[derive(Clone)]
struct YtmAudio {
    url: String,
    mime: String,
    bitrate: u64,
    duration_secs: u64,
    title: String,
    artist: String,
}

/// Full detail of one musicResponsiveListItemRenderer row (songs).
fn ytm_row_song(renderer: &serde_json::Value, kind: &str) -> Option<YtmSong> {
    let columns = renderer
        .get("flexColumns")
        .and_then(|columns| columns.as_array())?;
    let column_text = |index: usize| {
        columns
            .get(index)
            .and_then(|column| column.get("musicResponsiveListItemFlexColumnRenderer"))
            .and_then(|column| column.get("text"))
            .map(ytm_text)
            .unwrap_or_default()
    };
    let title = column_text(0);
    if title.trim().is_empty() {
        return None;
    }
    let texts = columns
        .get(1)
        .and_then(|column| column.get("musicResponsiveListItemFlexColumnRenderer"))
        .and_then(|column| column.get("text"))
        .map(|text| ytm_subtitle_texts(text))
        .unwrap_or_default();
    let artist = texts.first().cloned().unwrap_or_default();
    let album = texts.get(1).cloned().unwrap_or_default();
    if !ytm_row_is_available(renderer) {
        return None;
    }
    let video_type = ytm_row_video_type(renderer);
    // A shelf we couldn't classify by heading may still be identifiable by the
    // shape of its navigation endpoint (album/playlist/artist browse ids).
    let kind = if kind == "other" {
        ytm_row_browse_id(renderer)
            .as_deref()
            .and_then(ytm_kind_from_browse_id)
            .unwrap_or(kind)
    } else {
        kind
    };
    let id = renderer
        .get("overlay")
        .and_then(|overlay| overlay.get("musicItemThumbnailOverlayRenderer"))
        .and_then(|overlay| overlay.get("content"))
        .and_then(|content| content.get("musicPlayButtonRenderer"))
        .and_then(|button| button.get("playNavigationEndpoint"))
        .and_then(ytm_watch_id)?;
    let duration_secs = renderer
        .get("fixedColumns")
        .and_then(|columns| columns.as_array())
        .and_then(|columns| columns.first())
        .and_then(|column| column.get("musicResponsiveListItemFixedColumnRenderer"))
        .and_then(|column| column.get("text"))
        .map(ytm_text)
        .and_then(|text| parse_yt_length(&text))
        .unwrap_or(0.0) as u64;
    Some(YtmSong {
        video_id: id,
        title: title.trim().to_string(),
        artist: artist.trim().to_string(),
        album: album.trim().to_string(),
        thumbnail_url: ytm_thumbnail(renderer),
        clip_url: ytm_clip_thumbnail(renderer),
        kind: kind.to_string(),
        has_video: ytm_row_is_video(kind, &video_type),
        video_type,
        clip_video_id: String::new(),
        duration_secs,
    })
}

/// Full detail of one musicTwoRowItemRenderer tile (Videos / Albums shelves).
fn ytm_tile_song(renderer: &serde_json::Value, kind: &str) -> Option<YtmSong> {
    let title = ytm_text(renderer.get("title").unwrap_or(&serde_json::Value::Null));
    if title.trim().is_empty() {
        return None;
    }
    let texts = ytm_subtitle_texts(renderer.get("subtitle").unwrap_or(&serde_json::Value::Null));
    let artist = texts.first().cloned().unwrap_or_default();
    let video_type = ytm_row_video_type(renderer);
    let id = ytm_watch_id(
        renderer
            .get("navigationEndpoint")
            .unwrap_or(&serde_json::Value::Null),
    )
    .or_else(|| {
        renderer
            .get("overlay")
            .and_then(|overlay| overlay.get("musicItemThumbnailOverlayRenderer"))
            .and_then(|overlay| overlay.get("content"))
            .and_then(|content| content.get("musicPlayButtonRenderer"))
            .and_then(|button| button.get("playNavigationEndpoint"))
            .and_then(ytm_watch_id)
    })?;
    Some(YtmSong {
        video_id: id,
        title: title.trim().to_string(),
        artist: artist.trim().to_string(),
        album: String::new(),
        thumbnail_url: ytm_thumbnail(renderer),
        clip_url: ytm_clip_thumbnail(renderer),
        kind: kind.to_string(),
        has_video: ytm_row_is_video(kind, &video_type),
        video_type,
        clip_video_id: String::new(),
        duration_secs: 0,
    })
}

/// True when InnerTube explicitly says this is an audio-only upload.
///
/// `MUSIC_VIDEO_TYPE_ATV` is "audio track video" — a still image with a song
/// over it, so there is no clip worth showing. `PODCAST_EPISODE` likewise.
///
/// Note the deliberate asymmetry: an *absent* `videoType` is NOT audio-only.
/// Song rows on the Songs shelf simply omit the field, and treating that as
/// "has a video" made every song claim a clip it did not have — which both lit
/// up the clip control unconditionally and stopped pairing from ever running.
fn ytm_is_audio_only(video_type: &str) -> bool {
    matches!(
        video_type,
        "MUSIC_VIDEO_TYPE_ATV" | "MUSIC_VIDEO_TYPE_PODCAST_EPISODE"
    )
}

/// Whether a row is a real, playable video rather than an audio-only upload.
/// Only meaningful for rows already known to be videos; for songs use
/// `kind` (see `ytm_row_is_video`).
fn ytm_is_real_video(video_type: &str) -> bool {
    !ytm_is_audio_only(video_type) && video_type != "MUSIC_VIDEO_TYPE_UGC"
}

/// Whether a row should be treated as a video for clip purposes.
///
/// A row from the Videos shelf is a video unless InnerTube says otherwise. A
/// song row is not — even when `videoType` is missing, because that field is
/// simply absent there. Songs earn a clip through `pair_song_clips`, never by
/// default.
fn ytm_row_is_video(kind: &str, video_type: &str) -> bool {
    match kind {
        "video" => ytm_is_real_video(video_type),
        // The top-result card is a single videoId that may be either; trust an
        // explicit audio-only marker and otherwise let pairing decide.
        "top" => !ytm_is_audio_only(video_type),
        _ => false,
    }
}

/// `videoType` of a row, read from the play button's navigation endpoint.
fn ytm_row_video_type(renderer: &serde_json::Value) -> String {
    renderer
        .pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoType")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string()
}

/// InnerTube greys out rows it cannot serve (region-locked, removed, or
/// unplayable in this region) via a display policy. The web UI shows those
/// greyed out; offering them as playable results just produces a dead row, so
/// we drop them.
fn ytm_row_is_available(renderer: &serde_json::Value) -> bool {
    renderer
        .get("musicItemRendererDisplayPolicy")
        .and_then(|value| value.as_str())
        != Some("MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT")
}

/// Result type from a `browseId` prefix, which is language-independent.
///
/// This is the robust counterpart to matching a localized shelf title: the
/// prefixes are stable, so a row is still correctly identified when the shelf
/// heading is in a language we don't list.
fn ytm_kind_from_browse_id(browse_id: &str) -> Option<&'static str> {
    const PREFIXES: &[(&str, &str)] = &[
        ("MPRE", "album"),
        ("MPLA", "artist"),
        ("MPSP", "podcast"),
        ("MPED", "episode"),
        ("VL", "playlist"),
        ("VM", "playlist"),
        ("RD", "playlist"),
        ("UC", "artist"),
    ];
    PREFIXES
        .iter()
        .find(|(prefix, _)| browse_id.starts_with(prefix))
        .map(|(_, kind)| *kind)
}

/// `browseId` carried by a row's navigation endpoint, if any.
fn ytm_row_browse_id(renderer: &serde_json::Value) -> Option<String> {
    let endpoint = renderer
        .get("navigationEndpoint")
        .or_else(|| {
            renderer.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint")
        })?;
    endpoint
        .get("browseId")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .or_else(|| {
            endpoint
                .pointer("/browseEndpoint/browseId")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
}

/// Map a localized YT Music shelf title to a stable result kind.
///
/// Shelf titles arrive translated ("Пісні", "Відео", "Альбоми", "Songs", …), so
/// matching on English alone silently mislabels every non-English search. We
/// recognise a handful of the most common localizations per kind and fall back
/// to `other`, which the UI treats as "cover only, clip unknown".
fn ytm_shelf_kind(title: &str) -> &'static str {
    let lowered = title.trim().to_lowercase();
    if lowered.is_empty() {
        return "other";
    }
    const SONGS: &[&str] = &[
        "songs",
        "song",
        "пісні",
        "пісня",
        "песни",
        "песня",
        "lieder",
        "titres",
        "canciones",
        "canzoni",
        "songs",
        "música",
        "musica",
        "números",
        "musiques",
        "musique",
        "songs",
        "numere",
        "ut songs",
        "låtar",
        "sangtek",
        "şarkılar",
        "songs",
        "πλακοτάκια",
    ];
    const VIDEOS: &[&str] = &[
        "videos",
        "video",
        "відео",
        "видео",
        "vidéos",
        "videos",
        "clips",
        "клипи",
        "klipy",
        "videoclips",
        "vídeos",
        "musikvideos",
        "köp",
        "videolar",
        "klip",
        "klipów",
    ];
    const ALBUMS: &[&str] = &[
        "albums",
        "album",
        "альбоми",
        "альбом",
        "albüm",
        "albumy",
        "álbumes",
        "alben",
        "musique et",
        "альбомы",
    ];
    const PLAYLISTS: &[&str] = &[
        "playlists",
        "playlist",
        "плейлисти",
        "плейлист",
        "playlists",
        "співлисти",
        "listeler",
        "listy",
        "antologies",
        "mixes",
        "плейлисты",
    ];
    const ARTISTS: &[&str] = &[
        "artists",
        "artist",
        "виконавці",
        "исполнители",
        "artistas",
        "interpreten",
        "sanatçı",
        "артисты",
        "архісти",
        "künstler",
    ];
    for word in SONGS {
        if lowered.contains(word) {
            return "song";
        }
    }
    for word in VIDEOS {
        if lowered.contains(word) {
            return "video";
        }
    }
    for word in ALBUMS {
        if lowered.contains(word) {
            return "album";
        }
    }
    for word in PLAYLISTS {
        if lowered.contains(word) {
            return "playlist";
        }
    }
    for word in ARTISTS {
        if lowered.contains(word) {
            return "artist";
        }
    }
    "other"
}

/// Every playable entry for the UI: top card, card rows, song shelves,
/// video tiles, album/playlist tiles. No text filtering — the user picks.
///
/// Walks *every* tab of `tabbedSearchResultsRenderer`, not just index 0: the
/// unfiltered "All results" response the web player sends is what puts a top
/// result, a Songs shelf and a Videos shelf side by side, and each section's
/// localized title tells us which of the two artwork halves applies to it.
fn parse_ytm_songs(json: &serde_json::Value, limit: usize) -> Vec<YtmSong> {
    let mut out: Vec<YtmSong> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |song: YtmSong| {
        if out.len() >= limit {
            return;
        }
        if seen.insert(song.video_id.clone()) {
            out.push(song);
        }
    };
    // Continuation pages carry the same rows outside any tab/section wrapper.
    if let Some(continuation) = json
        .pointer("/continuationContents/musicShelfContinuation")
        .or_else(|| json.pointer("/continuationContents/musicPlaylistShelfContinuation"))
    {
        let kind = ytm_shelf_kind(
            &continuation
                .pointer("/header/musicShelfHeaderBasicRenderer/title")
                .map(ytm_text)
                .unwrap_or_default(),
        );
        if let Some(items) = continuation
            .get("contents")
            .and_then(|contents| contents.as_array())
        {
            for item in items {
                if let Some(row) = item.get("musicResponsiveListItemRenderer") {
                    if let Some(song) = ytm_row_song(row, kind) {
                        push(song);
                    }
                } else if let Some(tile) = item.get("musicTwoRowItemRenderer") {
                    if let Some(song) = ytm_tile_song(tile, kind) {
                        push(song);
                    }
                }
            }
        }
    }
    let tabs = json
        .pointer("/contents/tabbedSearchResultsRenderer/tabs")
        .and_then(|tabs| tabs.as_array());
    let Some(tabs) = tabs else {
        return out;
    };
    for tab in tabs {
        let sections = tab
            .pointer("/tabRenderer/content/sectionListRenderer/contents")
            .and_then(|contents| contents.as_array());
        let Some(sections) = sections else {
            continue;
        };
        for section in sections {
            if let Some(card) = section.get("musicCardShelfRenderer") {
                let title = card.get("title").map(ytm_text).unwrap_or_default();
                let texts =
                    ytm_subtitle_texts(card.get("subtitle").unwrap_or(&serde_json::Value::Null));
                let artist = texts.first().cloned().unwrap_or_default();
                let album = texts.get(1).cloned().unwrap_or_default();
                // A top-result card has no shelf title, so classify it from its
                // own subtitle (YT puts "Song" / "Video" there) and default to
                // the card position's usual meaning.
                let kind = ytm_shelf_kind(
                    &card
                        .get("subtitle")
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                );
                let kind = if kind == "other" { "top" } else { kind };
                // The top-result card's play target lives on `onTap`, not on a
                // play-button overlay, so read it from both.
                let id = card
                    .get("title")
                    .and_then(|title| title.get("runs"))
                    .and_then(|runs| runs.as_array())
                    .and_then(|runs| runs.iter().filter_map(|run| ytm_watch_id(run)).next())
                    .or_else(|| {
                        ytm_watch_id(card.pointer("/onTap").unwrap_or(&serde_json::Value::Null))
                    })
                    .or_else(|| ytm_watch_id(card));
                let video_type = card
                    .pointer("/onTap/watchEndpoint/videoType")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(id) = id {
                    if !title.trim().is_empty() {
                        push(YtmSong {
                            video_id: id,
                            title: title.trim().to_string(),
                            artist: artist.trim().to_string(),
                            album: album.trim().to_string(),
                            thumbnail_url: ytm_thumbnail(card),
                            clip_url: ytm_clip_thumbnail(card),
                            kind: kind.to_string(),
                            has_video: ytm_row_is_video(kind, &video_type),
                            video_type,
                            clip_video_id: String::new(),
                            duration_secs: 0,
                        });
                    }
                }
                if let Some(contents) = card
                    .get("contents")
                    .and_then(|contents| contents.as_array())
                {
                    for item in contents {
                        if let Some(row) = item.get("musicResponsiveListItemRenderer") {
                            if let Some(song) = ytm_row_song(row, "song") {
                                push(song);
                            }
                        }
                    }
                }
            }
            if let Some(shelf) = section.get("musicShelfRenderer") {
                let kind = ytm_shelf_kind(&shelf.get("title").map(ytm_text).unwrap_or_default());
                let items = shelf
                    .get("contents")
                    .and_then(|contents| contents.as_array());
                let Some(items) = items else {
                    continue;
                };
                for item in items {
                    if let Some(row) = item.get("musicResponsiveListItemRenderer") {
                        if let Some(song) = ytm_row_song(row, kind) {
                            push(song);
                        }
                    } else if let Some(tile) = item.get("musicTwoRowItemRenderer") {
                        if let Some(song) = ytm_tile_song(tile, kind) {
                            push(song);
                        }
                    }
                }
            }
            // Carousels (Albums / Singles / Community playlists) hold two-row
            // tiles that the web player shows as a horizontal row.
            if let Some(carousel) = section.get("musicCarouselShelfRenderer") {
                let kind = ytm_shelf_kind(
                    &carousel
                        .get("header")
                        .map(ytm_header_text)
                        .unwrap_or_default(),
                );
                let items = carousel
                    .get("contents")
                    .and_then(|contents| contents.as_array());
                let Some(items) = items else {
                    continue;
                };
                for item in items {
                    let tile = item
                        .get("musicTwoRowItemRenderer")
                        .or_else(|| item.get("musicResponsiveListItemRenderer"));
                    let Some(tile) = tile else { continue };
                    let song = if tile.get("flexColumns").is_some() {
                        ytm_row_song(tile, kind)
                    } else {
                        ytm_tile_song(tile, kind)
                    };
                    if let Some(song) = song {
                        push(song);
                    }
                }
            }
        }
    }
    out
}

/// Title of a `musicCarouselShelfRenderer` header, across the shapes InnerTube
/// uses for it (`header` as a plain object or as a wrapper).
fn ytm_header_text(value: &serde_json::Value) -> String {
    let direct = ytm_text(value);
    if !direct.trim().is_empty() {
        return direct;
    }
    ytm_text(
        value
            .pointer("/musicCarouselShelfBasicHeaderRenderer/title")
            .unwrap_or(&serde_json::Value::Null),
    )
}

/// Next continuation token in a search/browse response, if the page offered one.
fn ytm_next_continuation(json: &serde_json::Value) -> Option<String> {
    let pointer = |path: &str| {
        json.pointer(path)
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .filter(|token| !token.is_empty())
    };
    pointer("/continuationContents/musicShelfContinuation/nextContinuationData/continuation")
        .or_else(|| pointer("/continuationContents/musicShelfContinuation/nextContinuationData/continuationCommand"))
        .or_else(|| pointer("/continuationContents/musicPlaylistShelfContinuation/nextContinuationData/continuation"))
        .or_else(|| {
            json.pointer("/continuations/0/nextContinuationData/continuation")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .filter(|token| !token.is_empty())
        })
}

/// One InnerTube POST against music.youtube.com, exactly as the web client
/// issues it.
///
/// The web app calls `…/youtubei/v1/<endpoint>?prettyPrint=false` with **no**
/// `key=` - the request is authorized by the visitor id and the WEB_REMIX
/// client headers alone. We try that form first and only fall back to the
/// keyed URL if the keyless one is rejected, so a rotated public key can never
/// break search.
///
/// `continuation` is passed as a URL query parameter because that is where the
/// web client puts it; it is mirrored into the body as well since InnerTube has
/// accepted both shapes across client versions.
async fn ytm_innertube_post(
    client: &reqwest::Client,
    endpoint: &str,
    body: &serde_json::Value,
    auth: &YtApiAuth,
    version: &str,
    continuation: Option<&str>,
) -> Option<serde_json::Value> {
    let base = format!("https://music.youtube.com/youtubei/v1/{endpoint}");
    // Keyless first (what the web player sends), keyed as a fallback.
    let urls = [
        match continuation {
            Some(token) => format!("{base}?prettyPrint=false&continuation={token}"),
            None => format!("{base}?prettyPrint=false"),
        },
        match continuation {
            Some(token) => {
                format!(
                    "{base}?key={}&prettyPrint=false&continuation={token}",
                    auth.api_key
                )
            }
            None => format!("{base}?key={}&prettyPrint=false", auth.api_key),
        },
    ];

    for (attempt, url) in urls.iter().enumerate() {
        let mut req = client
            .post(url)
            .header("User-Agent", YT_UA)
            .header("Content-Type", "application/json")
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .header("x-youtube-client-name", "67")
            .header("x-youtube-client-version", version)
            .header("sec-ch-ua-full-version", YT_SEC_CH_UA_FULL);
        if let Some(data) = auth.visitor.as_deref() {
            req = req.header("X-Goog-Visitor-Id", data);
        }
        match req.json(body).send().await {
            Ok(res) => {
                let status = res.status();
                if status.is_success() {
                    return res.json().await.ok();
                }
                eprintln!(
                    "[yt-inner] {endpoint} HTTP {status} (keyless={})",
                    attempt == 0
                );
            }
            Err(error) => {
                eprintln!("[yt-inner] {endpoint} transport error: {error}");
            }
        }
    }
    None
}

/// Current YT Music search filter params.
///
/// These rotate; the composition is `EgWKAQ` + a two-letter type code + a
/// trailing flags segment. Songs/video/albums are fetched separately because
/// that is exactly what the web "All results" page does - one request per
/// shelf - and it is what makes both cover-art songs and video clips appear.
const SEARCH_PARAMS_SONGS: &str = "EgWKAQIIAWoMEA4QChADEAQQCRAF";
const SEARCH_PARAMS_VIDEOS: &str = "EgWKAQIQAWoMEA4QChADEAQQCRAF";
const SEARCH_PARAMS_ALBUMS: &str = "EgWKAQIYAWoMEA4QChADEAQQCRAF";

async fn ytm_search_pages_json(client: &reqwest::Client, query: &str) -> Vec<serde_json::Value> {
    /// How many continuation pages to chase when a query returns few rows.
    const MAX_CONTINUATIONS: usize = 2;
    let auth = youtube_auth(client).await;
    let version = YT_REMIX_VERSION;
    let mut pages = Vec::new();
    let mut tokens: Vec<String> = Vec::new();

    // Unfiltered ("All results", no `params` at all) first - it carries the
    // top-result card - then the Songs, Videos and Albums shelves.
    let filters: [Option<&str>; 4] = [
        None,
        Some(SEARCH_PARAMS_SONGS),
        Some(SEARCH_PARAMS_VIDEOS),
        Some(SEARCH_PARAMS_ALBUMS),
    ];
    for params in filters {
        let mut body = serde_json::json!({
            "context": innertube_context("WEB_REMIX", &version, auth.visitor.as_deref()),
            "query": query,
        });
        if let Some(filter) = params {
            body["params"] = serde_json::Value::String(filter.to_string());
        }
        let Some(json) = ytm_innertube_post(client, "search", &body, &auth, &version, None).await
        else {
            continue;
        };
        if let Some(token) = ytm_next_continuation(&json) {
            if !tokens.contains(&token) {
                tokens.push(token);
            }
        }
        pages.push(json);
    }

    // Follow continuations so a sparse page still fills the requested limit,
    // mirroring the web UI's infinite scroll.
    for _ in 0..MAX_CONTINUATIONS {
        let Some(token) = tokens.first().cloned() else {
            break;
        };
        tokens.remove(0);
        let body = serde_json::json!({
            "context": innertube_context("WEB_REMIX", &version, auth.visitor.as_deref()),
            "continuation": token,
        });
        let Some(json) =
            ytm_innertube_post(client, "search", &body, &auth, &version, Some(&token)).await
        else {
            continue;
        };
        if let Some(next) = ytm_next_continuation(&json) {
            if !tokens.contains(&next) {
                tokens.push(next);
            }
        }
        pages.push(json);
    }
    pages
}

/// Normalize a title for fuzzy song↔video matching.
///
/// YouTube's two shelves spell the same track differently — the Songs row is
/// usually the bare title while the video adds "(Official Video)", "HD",
/// "Lyrics", "Remix" or a channel suffix. Fold case, strip accents where they
/// are decomposed, drop punctuation and collapse whitespace so both sides land
/// on the same key. Cyrillic passes through unchanged apart from case and
/// punctuation, which is what we need for the localized catalogue.
fn ytm_match_key(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            for lowered in ch.to_lowercase() {
                out.push(lowered);
            }
        } else if ch.is_whitespace() {
            out.push(' ');
        }
        // Everything else (punctuation, decoration) is a separator we drop.
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip the trailing decorations YouTube bolts onto video titles so a fuzzy
/// match isn't defeated by a single "official video" suffix.
fn ytm_base_title(title: &str) -> String {
    const NOISE: &[&str] = &[
        "official video",
        "official music video",
        "official audio",
        "official lyric video",
        "official visualizer",
        "music video",
        "lyric video",
        "lyrics video",
        "visualizer",
        "official",
        "video",
        "audio",
        "lyrics",
        "lyric",
        "hd",
        "hq",
        "4k",
        "mv",
        "m/v",
        "remastered",
        "remaster",
        "explicit",
        "full album",
        "full song",
        "with lyrics",
    ];
    let mut key = ytm_match_key(title);
    // Longest phrases first so "official music video" wins over "official".
    let mut sorted: Vec<&str> = NOISE.to_vec();
    sorted.sort_by_key(|phrase| std::cmp::Reverse(phrase.len()));
    for phrase in sorted {
        loop {
            let Some(found) = key.find(phrase) else { break };
            // Strip only whole-word occurrences. Without this, "videography"
            // loses its prefix to the noise word "video" and can no longer match
            // a song actually titled "Videography".
            let end = found + phrase.len();
            let bytes = key.as_bytes();
            let starts_word = found == 0 || bytes[found - 1] == b' ';
            let ends_word = end == key.len() || bytes[end] == b' ';
            if !(starts_word && ends_word) {
                break;
            }
            let mut next = String::with_capacity(key.len());
            next.push_str(&key[..found]);
            next.push(' ');
            next.push_str(key[end..].trim_start());
            key = next.split_whitespace().collect::<Vec<_>>().join(" ");
        }
    }
    key
}

/// Attach a music video to every song that has one, the way the web client's
/// results imply.
///
/// A song row and its music video are different uploads: the Songs shelf
/// usually hands back an audio-only id (`MUSIC_VIDEO_TYPE_ATV`) while the
/// Videos shelf, which the same search already returns, holds the official
/// video. Matching them by title means the clip view can play a real video for
/// essentially every track instead of showing a static frame.
///
/// Only pairs a song whose own id is audio-only, and only when nothing better
/// was matched. A song that is already a video keeps its own id, and a song
/// with no counterpart is left without a clip so the UI can hide the control.
fn pair_song_clips(songs: &mut [YtmSong]) -> usize {
    // Index the playable videos by bare title, then by full normalized title.
    let mut by_base: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut by_full: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (index, song) in songs.iter().enumerate() {
        // Only rows from the Videos shelf can lend a clip, and never a row that
        // has already been paired.
        if song.kind != "video" || !song.has_video || !song.clip_video_id.is_empty() {
            continue;
        }
        let full = ytm_match_key(&song.title);
        let base = ytm_base_title(&song.title);
        if base.is_empty() {
            continue;
        }
        // First occurrence wins: the Videos shelf is already relevance-ordered.
        by_base.entry(base).or_insert(index);
        by_full.entry(full).or_insert(index);
    }

    let mut paired = 0usize;
    for index in 0..songs.len() {
        if songs[index].has_video && songs[index].clip_video_id.is_empty() {
            continue; // already a playable video in its own right
        }
        if songs[index].video_id.is_empty() {
            continue;
        }
        let base = ytm_base_title(&songs[index].title);
        let full = ytm_match_key(&songs[index].title);
        // Exact bare-title match first, then a looser full-title match.
        let candidate = by_base
            .get(&base)
            .or_else(|| by_full.get(&full))
            .copied()
            .filter(|candidate| *candidate != index);
        let Some(candidate) = candidate else { continue };
        let clip_id = songs[candidate].video_id.clone();
        if clip_id.is_empty() || clip_id == songs[index].video_id {
            continue;
        }
        // Re-point the still at the paired video, which is a real frame rather
        // than the song's static cover.
        let still = format!("https://i.ytimg.com/vi/{clip_id}/maxresdefault.jpg");
        songs[index].clip_video_id = clip_id;
        songs[index].has_video = true;
        songs[index].clip_url = still;
        paired += 1;
    }
    paired
}

/// Whether a video found by a targeted follow-up search is an acceptable clip
/// for a song.
///
/// The follow-up query already targets the track (`artist title official
/// video`), so this is a guard rather than a ranking: reject non-videos,
/// the song's own id, and titles that share nothing with the song once both
/// sides are reduced to their bare form. Short bare titles (under three
/// characters) never match — "я", "up" and friends would pair everything.
fn ytm_clip_fallback_accept(song: &YtmSong, video: &YtmSong) -> bool {
    if video.kind != "video" || !video.has_video || video.video_id.is_empty() {
        return false;
    }
    if video.video_id == song.video_id {
        return false;
    }
    let song_base = ytm_base_title(&song.title);
    let video_base = ytm_base_title(&video.title);
    if song_base.len() < 3 || video_base.is_empty() {
        return false;
    }
    video_base.contains(&song_base) || song_base.contains(&video_base)
}

/// Find a clip with a direct YouTube search when the mixed search paired
/// nothing.
///
/// The normal search pairs from the Videos shelf of the *same* query, but for
/// an obscure track that shelf can hold remixes while the official video sits
/// one query away. This issues one extra videos-filtered request —
/// `artist title official video` — and takes the first acceptable hit, so a
/// song that would otherwise show no clip gets a real one without flooding the
/// API.
async fn ytm_search_clip_fallback(
    client: &reqwest::Client,
    auth: &YtApiAuth,
    version: &str,
    song: &YtmSong,
) -> Option<(String, String)> {
    let title = song.title.trim();
    if title.is_empty() {
        return None;
    }
    let mut query = String::with_capacity(title.len() + song.artist.len() + 24);
    if !song.artist.trim().is_empty() {
        query.push_str(song.artist.trim());
        query.push(' ');
    }
    query.push_str(title);
    query.push_str(" official video");
    let body = serde_json::json!({
        "context": innertube_context("WEB_REMIX", version, auth.visitor.as_deref()),
        "query": query,
        "params": SEARCH_PARAMS_VIDEOS,
    });
    let json = ytm_innertube_post(client, "search", &body, auth, version, None).await?;
    for candidate in parse_ytm_songs(&json, 8) {
        if ytm_clip_fallback_accept(song, &candidate) {
            let clip_id = candidate.video_id.clone();
            let still = format!("https://i.ytimg.com/vi/{clip_id}/maxresdefault.jpg");
            return Some((clip_id, still));
        }
    }
    None
}

pub async fn ytm_search_songs(query: String, limit: Option<usize>) -> Result<Vec<YtmSong>, String> {
    let limit = limit.unwrap_or(8).clamp(1, 15);
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Gather from *every* page before applying the limit. The Videos shelf is
    // what the clip pairing draws on, and it is the third request — stopping at
    // the limit would have filled the list with Songs and never read it.
    for page in ytm_search_pages_json(&client, &query).await {
        for song in parse_ytm_songs(&page, limit) {
            if seen.insert(song.video_id.clone()) {
                out.push(song);
            }
        }
    }
    let paired = pair_song_clips(&mut out);
    out.truncate(limit);
    // Songs still without a clip get one targeted YouTube search each. Capped
    // so one user query can cost at most three extra videos-filtered requests,
    // and skipped for non-songs (albums, artists, playlists have no clip).
    // Reuses the client and auth from above — no extra connections.
    let mut fallback_clips = 0usize;
    let auth = youtube_auth(&client).await;
    let version = YT_REMIX_VERSION;
    for song in out
        .iter_mut()
        .take(limit)
        .filter(|song| song.kind == "song" && !song.has_video && song.clip_video_id.is_empty())
    {
        if fallback_clips >= 3 {
            break;
        }
        if let Some((clip_id, still)) =
            ytm_search_clip_fallback(&client, &auth, &version, song).await
        {
            song.clip_video_id = clip_id;
            song.has_video = true;
            song.clip_url = still;
            fallback_clips += 1;
        }
    }
    eprintln!(
        "[ytm-search] q=\"{query}\" songs={} paired_clips={} fallback_clips={} with_video={}",
        out.len(),
        paired,
        fallback_clips,
        out.iter().filter(|song| song.has_video).count()
    );
    Ok(out)
}

/// A clip found for one specific track.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmClip {
    pub video_id: String,
    pub still_url: String,
}

/// Find a music-video clip for a single track that has none yet.
///
/// The now-playing song often never went through search pairing (local files,
/// history, playlists, recommendations), so it carries no `clipVideoId`. This
/// runs the same targeted videos-filtered query the search-time fallback uses —
/// `artist title official video` — and returns the first acceptable hit. One
/// request, no continuations, so it stays cheap enough to call per track.

pub async fn ytm_find_clip(title: String, artist: String) -> Result<Option<YtmClip>, String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Ok(None);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let auth = youtube_auth(&client).await;
    let version = YT_REMIX_VERSION;
    // A probe, not a playable row: empty id so the accept guard can never
    // match the song with itself.
    let probe = YtmSong {
        video_id: String::new(),
        title,
        artist: artist.trim().to_string(),
        album: String::new(),
        thumbnail_url: String::new(),
        clip_url: String::new(),
        kind: "song".to_string(),
        video_type: String::new(),
        has_video: false,
        clip_video_id: String::new(),
        duration_secs: 0,
    };
    let found = ytm_search_clip_fallback(&client, &auth, &version, &probe).await;
    match found {
        Some((video_id, still)) => {
            eprintln!("[ytm-find-clip] \"{}\" -> {video_id}", probe.title);
            Ok(Some(YtmClip {
                video_id,
                still_url: still,
            }))
        }
        None => {
            eprintln!("[ytm-find-clip] \"{}\" -> none", probe.title);
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmSuggestion {
    /// The full query to run when the suggestion is picked.
    pub text: String,
    /// Segments of the suggestion, so the UI can bold the part the user typed
    /// exactly as the web typeahead does. Empty `bold` = remainder.
    pub runs: Vec<YtmSuggestionRun>,
    /// True when the entry came from the signed-in user's search history.
    pub from_history: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmSuggestionRun {
    pub text: String,
    pub bold: bool,
}

/// Typeahead suggestions, exactly as the web client's search box calls them.
///
/// This is the `music/get_search_suggestions` endpoint: it takes `input` (not
/// `query`) and returns *no* tracks — just refined query strings plus, for
/// history entries, a feedback token. Picking one re-runs the normal search.

pub async fn ytm_search_suggestions(query: String) -> Result<Vec<YtmSuggestion>, String> {
    let input = query.trim().to_string();
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let auth = youtube_auth(&client).await;
    let version = YT_REMIX_VERSION;
    let body = serde_json::json!({
        "context": innertube_context("WEB_REMIX", &version, auth.visitor.as_deref()),
        "input": input,
    });
    let Some(json) = ytm_innertube_post(
        &client,
        "music/get_search_suggestions",
        &body,
        &auth,
        &version,
        None,
    )
    .await
    else {
        return Ok(Vec::new());
    };
    Ok(parse_ytm_suggestions(&json, 10))
}

fn parse_ytm_suggestions(json: &serde_json::Value, limit: usize) -> Vec<YtmSuggestion> {
    let Some(contents) = json
        .pointer("/contents/0/searchSuggestionsSectionRenderer/contents")
        .and_then(|contents| contents.as_array())
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in contents {
        // History entries use a different renderer but the same inner shape.
        let node = entry
            .get("searchSuggestionRenderer")
            .or_else(|| entry.get("historySuggestionRenderer"));
        let Some(node) = node else { continue };
        let text = node
            .pointer("/navigationEndpoint/searchEndpoint/query")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if text.is_empty() || !seen.insert(text.clone()) {
            continue;
        }
        let runs = node
            .pointer("/suggestion/runs")
            .and_then(|runs| runs.as_array())
            .map(|runs| {
                runs.iter()
                    .map(|run| YtmSuggestionRun {
                        text: run
                            .get("text")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                            .to_string(),
                        bold: run
                            .get("bold")
                            .and_then(|value| value.as_bool())
                            .unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![YtmSuggestionRun {
                    text: text.clone(),
                    bold: false,
                }]
            });
        let from_history = node
            .pointer("/serviceEndpoint/feedbackEndpoint/feedbackToken")
            .is_some();
        out.push(YtmSuggestion {
            text,
            runs,
            from_history,
        });
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// Width/height declared by a YT Music thumbnail entry, when present.
fn ytm_thumb_size(entry: &serde_json::Value) -> (u32, u32) {
    let read = |key: &str| {
        entry
            .get(key)
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or(0)
    };
    (read("width"), read("height"))
}

/// Rewrite a YouTube cover URL to the highest quality bytes available.
///
/// `lh3.googleusercontent.com` sizing suffixes (`=w544-h544-l90-rj`) *replace*
/// the stored image with a re-encoded crop, so the API's own URL is already
/// downscaled and blurry on HiDPI displays. `=s0` returns the **original**
/// uploaded bytes, which is the real album art at full resolution — the
/// browser downscales it per `srcset` at paint time, so it stays sharp on
/// every display density. Non-Google hosts pass through untouched.
fn upgrade_cover_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() || !trimmed.contains("googleusercontent.com") {
        return trimmed.to_string();
    }
    // Strip whatever sizing suffix is present (`=w544-h544-l90-rj`, `=s0`,
    // `=h720`) so the request is always built from a clean base. Appending
    // unconditionally keeps this idempotent: an existing `=s0` is stripped and
    // re-added rather than doubling up.
    let base = match trimmed.rfind('=') {
        Some(idx) if idx > trimmed.len().saturating_sub(24) => &trimmed[..idx],
        _ => trimmed,
    };
    format!("{base}=s0")
}

/// True when a googleusercontent sizing suffix is present (`=w…`, `=s0`, `=h…`).
fn has_size_suffix(url: &str) -> bool {
    match url.rfind('=') {
        Some(idx) => {
            let tail = &url[idx + 1..];
            !tail.is_empty() && tail.len() <= 24 && !tail.contains('/')
        }
        None => false,
    }
}

/// The `thumbnails` array a YT Music renderer carries, across every shape
/// InnerTube uses (music item, tile, carousel card, plain video renderer).
fn ytm_thumb_list(value: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
    value
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .or_else(|| value.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"))
        .or_else(|| value.pointer("/thumbnail/thumbnails"))
        .or_else(|| value.get("thumbnails"))
        .and_then(|t| t.as_array())
}

/// YouTube Music cover art, not a letterboxed video frame.
///
/// YT returns a `thumbnails` array ascending by size, and the LAST entry is
/// usually a wide 16:9 `hq720` video still with black bars. Album art lives
/// in the `lh3.googleusercontent.com/…=w544-h544-l90-rj` square crops, so we
/// prefer the widest square crop and upgrade it to the original bytes. When
/// only `i.ytimg.com` stills exist we take the largest square-ish one
/// (`hqdefault`/`sddefault`) instead of the letterboxed `maxresdefault`.
fn ytm_thumbnail(value: &serde_json::Value) -> String {
    let Some(list) = ytm_thumb_list(value) else {
        return String::new();
    };

    let url_of = |entry: &serde_json::Value| -> String {
        entry
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };

    // Album art: square-ish, highest declared resolution.
    let mut best_square: Option<(u32, String)> = None;
    // Fallback: any shape, highest declared resolution.
    let mut best_any: Option<(u32, String)> = None;
    // Last resort: an entry with no declared size is still a usable URL, so
    // keep it rather than dropping the artwork entirely.
    let mut sizeless: Option<String> = None;
    for entry in list {
        let url = url_of(entry);
        if url.is_empty() {
            continue;
        }
        let (w, h) = ytm_thumb_size(entry);
        let longest = w.max(h);
        if longest == 0 {
            if sizeless.is_none() {
                sizeless = Some(url);
            }
            continue;
        }
        let is_square = w > 0 && h > 0 && (w as i64 - h as i64).abs() <= 2;
        if is_square && best_square.as_ref().is_none_or(|(size, _)| longest > *size) {
            best_square = Some((longest, url.clone()));
        }
        if best_any.as_ref().is_none_or(|(size, _)| longest > *size) {
            best_any = Some((longest, url.clone()));
        }
    }

    let chosen = match (best_square, best_any, sizeless) {
        (Some((_, url)), _, _) => url,
        (None, Some((_, url)), _) => url,
        (None, None, Some(url)) => url,
        (None, None, None) => return String::new(),
    };
    if has_size_suffix(&chosen) {
        upgrade_cover_url(&chosen)
    } else {
        chosen
    }
}

/// The 16:9 video still for the same item — the "clip" half of the pair.
///
/// This is the mirror image of `ytm_thumbnail`: it wants the *widest* entry and
/// skips square crops entirely, so the two functions can never return the same
/// bytes. Returns an empty string when the response carried no landscape still
/// (a pure audio result often has none), which the UI reads as "no clip for
/// this track" and leaves the cover button as the only option.
fn ytm_clip_thumbnail(value: &serde_json::Value) -> String {
    let Some(list) = ytm_thumb_list(value) else {
        return String::new();
    };
    let mut best_wide: Option<(u32, String)> = None;
    for entry in list {
        let url = entry
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .trim();
        if url.is_empty() {
            continue;
        }
        let (w, h) = ytm_thumb_size(entry);
        if w == 0 || h == 0 || w <= h {
            continue; // square or portrait crop — that's cover art, not a clip
        }
        // Wide stills are letterboxed far more than they are cropped, so score
        // on width and require a genuinely landscape aspect ratio.
        if w * 100 < h * 130 {
            continue;
        }
        if best_wide.as_ref().is_none_or(|(width, _)| w > *width) {
            best_wide = Some((w, url.to_string()));
        }
    }
    match best_wide {
        Some((_, url)) if has_size_suffix(&url) => upgrade_cover_url(&url),
        Some((_, url)) => url,
        None => String::new(),
    }
}

fn ytm_browse_item_from_row(
    row: &serde_json::Value,
    default_category: &str,
) -> Option<YtmBrowseItem> {
    let columns = row.get("flexColumns").and_then(|c| c.as_array())?;
    let col_text = |idx: usize| {
        columns
            .get(idx)
            .and_then(|col| col.get("musicResponsiveListItemFlexColumnRenderer"))
            .and_then(|col| col.get("text"))
            .map(ytm_text)
            .unwrap_or_default()
    };
    let title = col_text(0);
    if title.trim().is_empty() {
        return None;
    }
    let subtitle_texts = columns
        .get(1)
        .and_then(|col| col.get("musicResponsiveListItemFlexColumnRenderer"))
        .and_then(|col| col.get("text"))
        .map(|text| ytm_subtitle_texts(text))
        .unwrap_or_default();
    let artist = subtitle_texts.first().cloned().unwrap_or_default();
    let album = subtitle_texts.get(1).cloned().unwrap_or_default();
    let video_id = row
        .get("overlay")
        .and_then(|overlay| overlay.get("musicItemThumbnailOverlayRenderer"))
        .and_then(|overlay| overlay.get("content"))
        .and_then(|content| content.get("musicPlayButtonRenderer"))
        .and_then(ytm_watch_id)
        .or_else(|| row.get("navigationEndpoint").and_then(ytm_watch_id))
        .or_else(|| ytm_watch_id(row))?;
    let duration_secs = row
        .get("fixedColumns")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|c| c.get("musicResponsiveListItemFixedColumnRenderer"))
        .and_then(|c| c.get("text"))
        .map(ytm_text)
        .and_then(|t| parse_yt_length(&t))
        .unwrap_or(0.0) as u64;
    let thumbnail_url = ytm_thumbnail(row);
    let clip_url = ytm_clip_thumbnail(row);
    Some(YtmBrowseItem {
        video_id,
        title: title.trim().to_string(),
        artist: artist.trim().to_string(),
        album: album.trim().to_string(),
        thumbnail_url,
        clip_url,
        clip_video_id: String::new(),
        duration_secs,
        category: default_category.to_string(),
    })
}

fn ytm_browse_item_from_tile(
    tile: &serde_json::Value,
    default_category: &str,
) -> Option<YtmBrowseItem> {
    let title = ytm_text(tile.get("title").unwrap_or(&serde_json::Value::Null));
    if title.trim().is_empty() {
        return None;
    }
    let subtitle_texts =
        ytm_subtitle_texts(tile.get("subtitle").unwrap_or(&serde_json::Value::Null));
    let artist = subtitle_texts.first().cloned().unwrap_or_default();
    let video_id = tile
        .get("navigationEndpoint")
        .and_then(ytm_watch_id)
        .or_else(|| {
            tile.get("overlay")
                .and_then(|overlay| overlay.get("musicItemThumbnailOverlayRenderer"))
                .and_then(|overlay| overlay.get("content"))
                .and_then(|content| content.get("musicPlayButtonRenderer"))
                .and_then(ytm_watch_id)
        })
        .or_else(|| ytm_watch_id(tile))?;
    let thumbnail_url = ytm_thumbnail(tile);
    let clip_url = ytm_clip_thumbnail(tile);
    Some(YtmBrowseItem {
        video_id,
        title: title.trim().to_string(),
        artist: artist.trim().to_string(),
        album: String::new(),
        thumbnail_url,
        clip_url,
        clip_video_id: String::new(),
        duration_secs: 0,
        category: default_category.to_string(),
    })
}

fn ytm_browse_item_from_panel(
    renderer: &serde_json::Value,
    default_category: &str,
) -> Option<YtmBrowseItem> {
    let video_id = renderer.get("videoId").and_then(|v| v.as_str())?;
    if video_id.len() != 11
        || !video_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    let title = renderer.get("title").map(ytm_text).unwrap_or_default();
    if title.trim().is_empty() {
        return None;
    }
    let artist = renderer
        .get("longBylineText")
        .or_else(|| renderer.get("shortBylineText"))
        .map(ytm_text)
        .unwrap_or_default();
    let duration_secs = renderer
        .get("lengthText")
        .map(ytm_text)
        .and_then(|t| parse_yt_length(&t))
        .unwrap_or(0.0) as u64;
    let thumbnail_url = ytm_thumbnail(renderer);
    let clip_url = ytm_clip_thumbnail(renderer);
    Some(YtmBrowseItem {
        video_id: video_id.to_string(),
        title: title.trim().to_string(),
        artist: artist.trim().to_string(),
        album: String::new(),
        thumbnail_url,
        clip_url,
        clip_video_id: String::new(),
        duration_secs,
        category: default_category.to_string(),
    })
}

fn parse_ytm_browse_response(json: &serde_json::Value, limit: usize) -> Vec<YtmBrowseItem> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let sections = json
        .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .or_else(|| json.pointer("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents"))
        .and_then(|c| c.as_array());

    if let Some(sections) = sections {
        for section in sections {
            let (shelf_items, category) =
                if let Some(carousel) = section.get("musicCarouselShelfRenderer") {
                    let cat = carousel
                        .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title")
                        .map(ytm_text)
                        .unwrap_or_else(|| "Recommendations".to_string());
                    (carousel.get("contents").and_then(|c| c.as_array()), cat)
                } else if let Some(shelf) = section.get("musicShelfRenderer") {
                    let cat = shelf
                        .get("title")
                        .map(ytm_text)
                        .unwrap_or_else(|| "Tracks".to_string());
                    (shelf.get("contents").and_then(|c| c.as_array()), cat)
                } else {
                    (None, "Recommendations".to_string())
                };

            if let Some(items) = shelf_items {
                for item in items {
                    if out.len() >= limit {
                        break;
                    }
                    let candidate = if let Some(row) = item.get("musicResponsiveListItemRenderer") {
                        ytm_browse_item_from_row(row, &category)
                    } else if let Some(tile) = item.get("musicTwoRowItemRenderer") {
                        ytm_browse_item_from_tile(tile, &category)
                    } else {
                        None
                    };

                    if let Some(song) = candidate {
                        if seen.insert(song.video_id.clone()) {
                            out.push(song);
                        }
                    }
                }
            }
        }
    }
    out
}

fn parse_ytm_next_response(json: &serde_json::Value, limit: usize) -> Vec<YtmBrowseItem> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let items = json
        .pointer("/contents/singleColumnMusicWatchNextResultsRenderer/tabbedRenderer/watchNextTabbedResultsRenderer/tabs/0/tabRenderer/content/musicQueueRenderer/content/playlistPanelRenderer/contents")
        .and_then(|c| c.as_array());

    if let Some(items) = items {
        for item in items {
            if out.len() >= limit {
                break;
            }
            if let Some(panel) = item.get("playlistPanelVideoRenderer") {
                if let Some(song) = ytm_browse_item_from_panel(panel, "Recommended") {
                    if seen.insert(song.video_id.clone()) {
                        out.push(song);
                    }
                }
            }
        }
    }
    out
}

pub async fn ytm_browse(
    browse_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<YtmBrowseItem>, String> {
    let limit = limit.unwrap_or(30).clamp(1, 100);
    let id = browse_id.unwrap_or_else(|| "FEmusic_home".to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let auth = youtube_auth(&client).await;
    let version = YT_REMIX_VERSION;
    let body = serde_json::json!({
        "context": innertube_context("WEB_REMIX", &version, auth.visitor.as_deref()),
        "browseId": id,
    });
    let Some(json) = ytm_innertube_post(&client, "browse", &body, &auth, &version, None).await
    else {
        return Err(format!("browse {id} failed"));
    };
    let mut items = parse_ytm_browse_response(&json, limit);

    // `FEmusic_charts` does not return tracks in its first page: it returns
    // artist/category tiles and puts the actual chart entries behind a
    // continuation. Without following it, charts looked empty.
    if items.is_empty() {
        let mut tokens: Vec<String> = Vec::new();
        if let Some(token) = ytm_next_continuation(&json) {
            tokens.push(token);
        }
        // Also collect any continuation embedded in a section list.
        if let Some(sections) = json.pointer(
            "/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents",
        ).and_then(|c| c.as_array()) {
            for section in sections {
                if let Some(token) = section
                    .pointer("/musicShelfRenderer/continuations/0/nextContinuationData/continuation")
                    .and_then(|value| value.as_str())
                {
                    if !token.is_empty() && !tokens.contains(&token.to_string()) {
                        tokens.push(token.to_string());
                    }
                }
            }
        }
        for token in tokens.iter().take(3) {
            let body = serde_json::json!({
                "context": innertube_context("WEB_REMIX", &version, auth.visitor.as_deref()),
                "continuation": token,
            });
            let Some(page) =
                ytm_innertube_post(&client, "browse", &body, &auth, &version, Some(token)).await
            else {
                continue;
            };
            items.extend(parse_ytm_browse_continuation(&page, limit));
            if items.len() >= limit {
                break;
            }
        }
        items.truncate(limit);
    }
    eprintln!("[ytm-browse] id=\"{id}\" items={}", items.len());
    Ok(items)
}

/// Rows from a browse continuation page, which sit outside any tab/section
/// wrapper.
///
/// A continuation response can wrap its rows three different ways, so try each
/// container rather than assuming one: a bare `musicShelfContinuation`, a
/// playlist shelf, or a `sectionListContinuation` that itself holds carousels.
fn parse_ytm_browse_continuation(json: &serde_json::Value, limit: usize) -> Vec<YtmBrowseItem> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut category = "Charts".to_string();

    let push_items = |items: &[serde_json::Value],
                      category: &str,
                      out: &mut Vec<YtmBrowseItem>,
                      seen: &mut std::collections::HashSet<String>,
                      limit: usize| {
        for item in items {
            if out.len() >= limit {
                break;
            }
            let candidate = if let Some(row) = item.get("musicResponsiveListItemRenderer") {
                ytm_browse_item_from_row(row, &category)
            } else if let Some(tile) = item.get("musicTwoRowItemRenderer") {
                ytm_browse_item_from_tile(tile, &category)
            } else {
                None
            };
            if let Some(song) = candidate {
                if seen.insert(song.video_id.clone()) {
                    out.push(song);
                }
            }
        }
    };

    let continuation = json.pointer("/continuationContents");
    if let Some(title) = continuation
        .and_then(|c| {
            c.pointer("/musicShelfContinuation/header/musicShelfHeaderBasicRenderer/title")
        })
        .map(ytm_text)
    {
        if !title.trim().is_empty() {
            category = title;
        }
    }
    if let Some(items) = continuation
        .and_then(|c| c.pointer("/musicShelfContinuation/contents"))
        .and_then(|c| c.as_array())
    {
        push_items(items, &category, &mut out, &mut seen, limit);
    }
    if let Some(items) = continuation
        .and_then(|c| c.pointer("/musicPlaylistShelfContinuation/contents"))
        .and_then(|c| c.as_array())
    {
        push_items(items, &category, &mut out, &mut seen, limit);
    }
    // `sectionListContinuation` nests whole carousels, not rows.
    if let Some(sections) = continuation
        .and_then(|c| c.pointer("/sectionListContinuation/contents"))
        .and_then(|c| c.as_array())
    {
        for section in sections {
            let shelf_category = section
                .pointer("/musicCarouselShelfRenderer/header/musicCarouselShelfBasicHeaderRenderer/title")
                .or_else(|| section.pointer("/musicShelfRenderer/title"))
                .map(ytm_text)
                .filter(|t| !t.trim().is_empty());
            if let Some(title) = shelf_category.clone() {
                category = title;
            }
            if let Some(items) = section
                .get("musicCarouselShelfRenderer")
                .and_then(|c| c.get("contents"))
                .or_else(|| {
                    section
                        .get("musicShelfRenderer")
                        .and_then(|c| c.get("contents"))
                })
                .and_then(|c| c.as_array())
            {
                push_items(items, &category, &mut out, &mut seen, limit);
            }
        }
    }
    out
}

pub async fn ytm_recommendations(
    video_id: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<YtmBrowseItem>, String> {
    let limit = limit.unwrap_or(20).clamp(1, 50);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let auth = youtube_auth(&client).await;
    let visitor = auth.visitor.clone();
    let version = YT_REMIX_VERSION;

    // 1. If video_id provided, call /next for algorithmic radio recommendations
    if let Some(id) = video_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if id.len() == 11
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            let body = serde_json::json!({
                "context": innertube_context("WEB_REMIX", &version, visitor.as_deref()),
                "videoId": id,
            });
            let mut req = client
                .post(format!(
                    "https://music.youtube.com/youtubei/v1/next?key={}&prettyPrint=false",
                    auth.api_key
                ))
                .header("User-Agent", YT_UA)
                .header("Content-Type", "application/json")
                .header("Origin", "https://music.youtube.com")
                .header("Referer", "https://music.youtube.com/")
                .header("x-youtube-client-name", "67")
                .header("x-youtube-client-version", YT_REMIX_VERSION)
                .header("sec-ch-ua-full-version", YT_SEC_CH_UA_FULL);
            if let Some(data) = visitor.as_deref() {
                req = req.header("X-Goog-Visitor-Id", data);
            }
            if let Ok(res) = req.json(&body).send().await {
                if res.status().is_success() {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        let items = parse_ytm_next_response(&json, limit);
                        if !items.is_empty() {
                            eprintln!("[ytm-recom] via=next videoId={id} items={}", items.len());
                            return Ok(items);
                        }
                    }
                }
            }
        }
    }

    // 2. If query provided, search YTM and map to browse items
    if let Some(q) = query.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let songs = ytm_search_songs(q.to_string(), Some(limit)).await?;
        if !songs.is_empty() {
            let items: Vec<YtmBrowseItem> = songs
                .into_iter()
                .map(|s| YtmBrowseItem {
                    video_id: s.video_id,
                    title: s.title,
                    artist: s.artist,
                    album: s.album,
                    // Carry the resolved pair through — dropping them here left
                    // every search-based recommendation with a blank cover.
                    thumbnail_url: s.thumbnail_url,
                    clip_url: s.clip_url,
                    clip_video_id: s.clip_video_id,
                    duration_secs: s.duration_secs,
                    category: format!("Based on {}", q),
                })
                .collect();
            eprintln!("[ytm-recom] via=search query=\"{q}\" items={}", items.len());
            return Ok(items);
        }
    }

    // 3. Fallback to home/editorial recommendations
    ytm_browse(Some("FEmusic_home".to_string()), Some(limit)).await
}

/// One Innertube player client to try, in order.
struct PlayerClient {
    /// Innertube clientName.
    name: &'static str,
    /// Pinned clientVersion (dynamic versions get 404/400).
    version: &'static str,
    /// x-youtube-client-name header value.
    header_name: &'static str,
    /// youtubei host for this client.
    host: &'static str,
    /// Origin/Referer base URL.
    origin: &'static str,
}

/// WEB_REMIX first (music entities), then ANDROID: pinned release 20.10.38
/// answers OK with plain (non-ciphered) audio URLs where WEB_REMIX answers
/// UNPLAYABLE or cipher-only (e.g. major-label tracks like Training Season).
/// Older Android releases get HTTP 400; TVHTML5 asks for login — both skipped.
const PLAYER_CLIENTS: &[PlayerClient] = &[
    PlayerClient {
        name: "WEB_REMIX",
        version: YT_REMIX_VERSION,
        header_name: "67",
        host: "https://music.youtube.com",
        origin: "https://music.youtube.com",
    },
    PlayerClient {
        name: "ANDROID",
        version: YT_ANDROID_VERSION,
        header_name: "3",
        host: "https://www.youtube.com",
        origin: "https://www.youtube.com",
    },
];

/// Raw player JSON for one client. Ok(None) = this client has no playable
/// response (non-OK playability); try the next client.
async fn ytm_player_json(
    client: &reqwest::Client,
    api_key: &str,
    visitor: Option<&str>,
    player: &PlayerClient,
    video_id: &str,
) -> Result<Option<serde_json::Value>, String> {
    let body = serde_json::json!({
        "context": innertube_context(player.name, player.version, visitor),
        "videoId": video_id,
        "playbackContext": {
            "contentPlaybackContext": {
                "referer": format!("https://music.youtube.com/watch?v={video_id}"),
            }
        },
    });
    let mut req = client
        .post(format!(
            "{}/youtubei/v1/player?key={}&prettyPrint=false&alt=json",
            player.host, api_key
        ))
        .header("User-Agent", YT_UA)
        .header("Content-Type", "application/json")
        .header("Origin", player.origin)
        .header("Referer", format!("{}/", player.origin))
        .header("x-youtube-client-name", player.header_name)
        .header("x-youtube-client-version", player.version)
        .header("sec-ch-ua-full-version", YT_SEC_CH_UA_FULL);
    if let Some(data) = visitor {
        req = req.header("X-Goog-Visitor-Id", data);
    }
    let res = req.json(&body).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("player {} HTTP {}", player.name, res.status()));
    }
    let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    if json
        .pointer("/playabilityStatus/status")
        .and_then(|status| status.as_str())
        != Some("OK")
    {
        return Ok(None);
    }
    Ok(Some(json))
}

/// True when the candidate `(mime, bitrate)` beats the current pick under the
/// same rule as `pick_best_audio`: decodable container first, then bitrate.
///
/// The container rank is load-bearing, not a preference: Symphonia (behind
/// rodio's decoder) ships no Opus codec, and the native Opus fallback only
/// reads Ogg — never WebM — so a WebM-Opus pick would fail at load time.
/// Shared here so the per-response pick and the cross-client pick in
/// `ytm_audio_stream` rank streams identically.
fn is_better_audio_pick(mime: &str, bitrate: u64, current: &YtmAudio) -> bool {
    let key = (audio_container_rank(mime), std::cmp::Reverse(bitrate));
    key < (
        audio_container_rank(&current.mime),
        std::cmp::Reverse(current.bitrate),
    )
}

/// Best plain-URL audio stream in a player response. Ciphered formats need
/// JS deciphering — skipped natively. Returns the pick plus (total, plain)
/// audio format counts for diagnostics.
fn pick_best_audio(json: &serde_json::Value) -> (Option<YtmAudio>, usize, usize) {
    let duration_secs = json
        .pointer("/videoDetails/lengthSeconds")
        .and_then(|value| value.as_str())
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(0);
    let title = json
        .pointer("/videoDetails/title")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let artist = json
        .pointer("/videoDetails/author")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let mut best: Option<YtmAudio> = None;
    let mut total = 0usize;
    let mut plain = 0usize;
    for key in ["formats", "adaptiveFormats"] {
        let formats = json
            .pointer(&format!("/streamingData/{key}"))
            .and_then(|value| value.as_array());
        let formats = match formats {
            Some(formats) => formats,
            None => continue,
        };
        for format in formats {
            total += 1;
            let mime = format
                .get("mimeType")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if !mime.contains("audio/") {
                continue;
            }
            let url = match format.get("url").and_then(|value| value.as_str()) {
                Some(url) => url,
                None => continue,
            };
            plain += 1;
            let bitrate = format
                .get("bitrate")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let better = best
                .as_ref()
                .map(|current: &YtmAudio| is_better_audio_pick(mime, bitrate, current))
                .unwrap_or(true);
            if better {
                best = Some(YtmAudio {
                    url: url.to_string(),
                    mime: mime.split(';').next().unwrap_or(mime).to_string(),
                    bitrate,
                    duration_secs,
                    title: title.clone(),
                    artist: artist.clone(),
                });
            }
        }
    }
    (best, total, plain)
}

/// Best playable audio stream for a video, trying each player client and
/// keeping the best pick across all of them.
///
/// WEB_REMIX is tried first, but it sometimes answers OK with a playable yet
/// low-bitrate stream while ANDROID would have served a better one — taking
/// the first pick would lock playback to the worse quality. Comparing every
/// client's pick with `is_better_audio_pick` keeps all clients in the running.
async fn ytm_audio_stream(
    client: &reqwest::Client,
    video_id: &str,
) -> Result<Option<YtmAudio>, String> {
    let auth = youtube_auth(client).await;
    let visitor = auth.visitor.clone();
    let mut last_error: Option<String> = None;
    let mut best: Option<YtmAudio> = None;
    for player in PLAYER_CLIENTS {
        match ytm_player_json(client, &auth.api_key, visitor.as_deref(), player, video_id).await {
            Err(error) => {
                eprintln!(
                    "[ytm-player] id={video_id} via={} error={error}",
                    player.name
                );
                last_error = Some(error);
            }
            Ok(None) => {
                eprintln!(
                    "[ytm-player] id={video_id} via={} playability!=OK",
                    player.name
                );
            }
            Ok(Some(json)) => {
                let (pick, total, plain) = pick_best_audio(&json);
                eprintln!(
                    "[ytm-player] id={video_id} via={} formats={total} audio_plain={plain} picked={}",
                    player.name,
                    pick.as_ref().map(|audio| audio.bitrate).unwrap_or(0)
                );
                if let Some(pick) = pick {
                    let better = best
                        .as_ref()
                        .map(|current| is_better_audio_pick(&pick.mime, pick.bitrate, current))
                        .unwrap_or(true);
                    if better {
                        best = Some(pick);
                    }
                }
                // OK playability but cipher-only: a later client may differ.
            }
        }
    }
    if best.is_some() {
        return Ok(best);
    }
    // No client yielded a playable stream: surface the last transport
    // error when every attempt failed that way, else a clean miss.
    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

/// Best *video* stream in a player response, for the "clip" view.
///
/// Prefers a progressive (muxed video+audio) MP4 from `streamingData.formats`:
/// it plays standalone in a `<video>` element and never needs a separate audio
/// track, which is all a muted looping preview wants. Falls back to an adaptive
/// video-only MP4, which Chromium also plays fine when muted.
///
/// Returns url, mime, and whether the pick carries an audio track. Only a
/// muxed pick does — the UI needs to know, because "play the clip's sound"
/// is impossible for a video-only stream.
fn pick_best_video(json: &serde_json::Value) -> Option<(String, String, bool)> {
    let mut best_progressive: Option<(u64, String, String)> = None;
    let mut best_adaptive: Option<(u64, String, String)> = None;
    // `formats` holds muxed streams; `adaptiveFormats` holds video-only ones.
    for (key, progressive) in [("formats", true), ("adaptiveFormats", false)] {
        let formats = json
            .pointer(&format!("/streamingData/{key}"))
            .and_then(|value| value.as_array());
        let Some(formats) = formats else { continue };
        for format in formats {
            let mime = format
                .get("mimeType")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if !mime.contains("video/") || !mime.contains("mp4") {
                continue;
            }
            let Some(url) = format.get("url").and_then(|value| value.as_str()) else {
                continue;
            };
            let bitrate = format
                .get("bitrate")
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            let base = mime.split(';').next().unwrap_or(mime).to_string();
            let slot = if progressive {
                &mut best_progressive
            } else {
                &mut best_adaptive
            };
            if slot.as_ref().is_none_or(|(b, _, _)| bitrate > *b) {
                *slot = Some((bitrate, url.to_string(), base));
            }
        }
    }
    let muxed = best_progressive.is_some();
    let (_, url, mime) = best_progressive.or(best_adaptive)?;
    Some((url, mime, muxed))
}

/// Best playable video stream, trying each player client until one yields a
/// plain-URL stream.
async fn ytm_video_stream(
    client: &reqwest::Client,
    video_id: &str,
) -> Result<Option<(String, String, bool)>, String> {
    let auth = youtube_auth(client).await;
    let visitor = auth.visitor.clone();
    let mut last_error: Option<String> = None;
    for player in PLAYER_CLIENTS {
        match ytm_player_json(client, &auth.api_key, visitor.as_deref(), player, video_id).await {
            Err(error) => {
                eprintln!(
                    "[ytm-video] id={video_id} via={} error={error}",
                    player.name
                );
                last_error = Some(error);
            }
            Ok(None) => {}
            Ok(Some(json)) => {
                if let Some(pick) = pick_best_video(&json) {
                    return Ok(Some(pick));
                }
            }
        }
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtmVideoStream {
    pub url: String,
    pub mime_type: String,
    /// True only for a muxed (progressive) pick. A video-only adaptive pick
    /// has no sound, so the UI must keep the song audio under it instead of
    /// switching to the clip's.
    pub has_audio: bool,
    pub title: String,
    pub artist: String,
}

/// Progressive/adaptive MP4 URL for a videoId, so the UI can play a real video
/// clip instead of only a still frame. None when the video is audio-only.

pub async fn ytm_video_url(video_id: String) -> Result<Option<YtmVideoStream>, String> {
    let id = video_id.trim().to_string();
    if id.len() != 11
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Ok(None);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let Some((url, mime, has_audio)) = ytm_video_stream(&client, &id).await? else {
        return Ok(None);
    };
    Ok(Some(YtmVideoStream {
        url,
        mime_type: mime,
        has_audio,
        title: String::new(),
        artist: String::new(),
    }))
}

pub async fn ytm_stream_url(video_id: String) -> Result<Option<YtmStream>, String> {
    let id = video_id.trim().to_string();
    if id.len() != 11
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Ok(None);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let audio = ytm_audio_stream(&client, &id).await?;
    Ok(audio.map(|audio| YtmStream {
        url: audio.url,
        mime_type: audio.mime,
        bitrate: audio.bitrate,
        duration_secs: audio.duration_secs,
        title: audio.title,
        artist: audio.artist,
    }))
}

/// Container preference for local decode: rodio/symphonia handles MP4/AAC
/// reliably. WebM-Opus is not natively decodable — Symphonia ships no Opus
/// codec, and the opus fallback only reads Ogg, never WebM — so an Opus pick
/// would fail at load time. Rank is checked before bitrate (see
/// `is_better_audio_pick`); do not reorder without a WebM-Opus decode path.
fn audio_container_rank(mime: &str) -> u8 {
    if mime.contains("audio/mp4") {
        0
    } else if mime.contains("audio/mpeg") {
        1
    } else if mime.contains("audio/webm") {
        2
    } else {
        3
    }
}

fn audio_ext_for_mime(mime: &str) -> &'static str {
    if mime.contains("audio/mp4") {
        "m4a"
    } else if mime.contains("audio/webm") {
        "webm"
    } else if mime.contains("audio/mpeg") {
        "mp3"
    } else {
        "m4a"
    }
}

fn sanitize_filename(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    let trimmed = out.trim().trim_matches('.').trim().to_string();
    if trimmed.is_empty() {
        return "track".to_string();
    }
    trimmed.chars().take(80).collect()
}

/// googlevideo audio URLs reject full (non-ranged) downloads with HTTP 403
/// and only serve bounded `Range` requests (206). Download in chunks and
/// concatenate, like yt-dlp does.
const AUDIO_CHUNK_BYTES: u64 = 512 * 1024;
const AUDIO_MAX_CHUNKS: usize = 256;

/// Parse `Content-Range: bytes 0-65535/1234567` → total size.
/// Rejects the `bytes */N` (range-unsatisfied) form.
fn parse_content_range_total(header: &str) -> Option<u64> {
    let (range, total) = header.split_once('/')?;
    let range = range.trim();
    let range = range.strip_prefix("bytes").map(str::trim)?;
    if range == "*" {
        return None;
    }
    let (from, to) = range.split_once('-')?;
    from.trim().parse::<u64>().ok()?;
    to.trim().parse::<u64>().ok()?;
    total.trim().parse::<u64>().ok()
}

const YT_ANDROID_UA: &str = "com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip";
const YT_ANDROID_ORIGIN: &str = "https://www.youtube.com";
const YT_ANDROID_REFERER: &str = "https://www.youtube.com/";

/// How many audio range requests fly at once. googlevideo only serves bounded
/// `Range` replies (206); fetching them in parallel turns a tens-of-seconds
/// sequential download into a few seconds.
const AUDIO_DL_CONCURRENCY: usize = 4;

async fn download_audio_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let end0 = AUDIO_CHUNK_BYTES - 1;
    let res = client
        .get(url)
        .header("User-Agent", YT_ANDROID_UA)
        .header("Origin", YT_ANDROID_ORIGIN)
        .header("Referer", YT_ANDROID_REFERER)
        .header("Range", format!("bytes=0-{end0}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        eprintln!("[ytm-dl] range-not-satisfiable on first chunk");
        return Err("audio HTTP 416".to_string());
    }
    if status != reqwest::StatusCode::PARTIAL_CONTENT {
        if status.is_success() {
            // Server ignored Range: treat the body as the whole file.
            let body = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
            if body.is_empty() {
                return Err("audio download too small".to_string());
            }
            eprintln!("[ytm-dl] bytes={} chunks=1 (full body)", body.len());
            return Ok(body);
        }
        eprintln!("[ytm-dl] audio HTTP {status} (first chunk)");
        return Err(format!("audio HTTP {status}"));
    }
    let total = res
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_content_range_total);
    let first = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
    if first.is_empty() {
        return Err("audio download too small".to_string());
    }
    let done = |len: u64| {
        first.len() < AUDIO_CHUNK_BYTES as usize || total.map(|size| len >= size).unwrap_or(false)
    };
    if done(first.len() as u64) {
        eprintln!("[ytm-dl] bytes={} chunks=1", first.len());
        return Ok(first);
    }
    let Some(total) = total else {
        // No total to plan parallel ranges against: drain sequentially.
        return download_audio_bytes_sequential(client, url, first).await;
    };
    let mut ranges = Vec::new();
    let mut start = first.len() as u64;
    while start < total {
        let end = (start + AUDIO_CHUNK_BYTES - 1).min(total - 1);
        ranges.push((start, end));
        start = end + 1;
    }
    if ranges.len() + 1 > AUDIO_MAX_CHUNKS {
        return Err(format!(
            "audio download took too many chunks ({} bytes so far)",
            first.len()
        ));
    }
    let mut parts: Vec<(u64, Vec<u8>)> = vec![(0, first)];
    for wave in ranges.chunks(AUDIO_DL_CONCURRENCY) {
        let mut handles = Vec::with_capacity(wave.len());
        for &(range_start, range_end) in wave {
            let client = client.clone();
            let url = url.to_string();
            handles.push(tokio::spawn(async move {
                let res = client
                    .get(&url)
                    .header("User-Agent", YT_ANDROID_UA)
                    .header("Origin", YT_ANDROID_ORIGIN)
                    .header("Referer", YT_ANDROID_REFERER)
                    .header("Range", format!("bytes={range_start}-{range_end}"))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;
                if res.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    return Err::<(u64, Vec<u8>), String>(format!(
                        "audio HTTP {} (bytes {range_start}-{range_end})",
                        res.status()
                    ));
                }
                let chunk = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
                Ok((range_start, chunk))
            }));
        }
        for handle in handles {
            parts.push(
                handle
                    .await
                    .map_err(|e| format!("audio download task failed: {e}"))??,
            );
        }
    }
    parts.sort_by_key(|(range_start, _)| *range_start);
    let mut out = Vec::with_capacity(total as usize);
    let mut expect = 0u64;
    for (range_start, chunk) in parts {
        if range_start != expect || chunk.is_empty() {
            return Err(format!("audio download gap at byte {expect}"));
        }
        out.extend_from_slice(&chunk);
        expect += chunk.len() as u64;
    }
    if out.is_empty() {
        return Err("audio download too small".to_string());
    }
    eprintln!("[ytm-dl] bytes={} chunks={}", out.len(), 1 + ranges.len());
    Ok(out)
}

/// Sequential drain used only when the first reply hides the total size, so no
/// parallel plan can be made. Bounded by `AUDIO_MAX_CHUNKS` like before.
async fn download_audio_bytes_sequential(
    client: &reqwest::Client,
    url: &str,
    first: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let mut out = first;
    let mut requests = 1usize;
    loop {
        requests += 1;
        if requests > AUDIO_MAX_CHUNKS {
            return Err(format!(
                "audio download took too many chunks ({} bytes so far)",
                out.len()
            ));
        }
        let start = out.len() as u64;
        let end = start.saturating_add(AUDIO_CHUNK_BYTES - 1);
        let res = client
            .get(url)
            .header("User-Agent", YT_ANDROID_UA)
            .header("Origin", YT_ANDROID_ORIGIN)
            .header("Referer", YT_ANDROID_REFERER)
            .header("Range", format!("bytes={start}-{end}"))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        match res.status() {
            s if s == reqwest::StatusCode::PARTIAL_CONTENT => {
                let chunk = res.bytes().await.map_err(|e| e.to_string())?;
                if chunk.is_empty() || chunk.len() < AUDIO_CHUNK_BYTES as usize {
                    out.extend_from_slice(&chunk);
                    break;
                }
                out.extend_from_slice(&chunk);
            }
            s if s == reqwest::StatusCode::RANGE_NOT_SATISFIABLE => break,
            s if s.is_success() => {
                let body = res.bytes().await.map_err(|e| e.to_string())?;
                out.extend_from_slice(&body);
                break;
            }
            s => {
                eprintln!(
                    "[ytm-dl] audio HTTP {s} (chunk {requests}, {} bytes so far)",
                    out.len()
                );
                return Err(format!("audio HTTP {s}"));
            }
        }
    }
    if out.is_empty() {
        return Err("audio download too small".to_string());
    }
    eprintln!(
        "[ytm-dl] bytes={} chunks={requests} (sequential)",
        out.len()
    );
    Ok(out)
}

fn should_refresh_audio_url(error: &str) -> bool {
    error.contains("HTTP 403")
}

/// A googlevideo signature can be rejected once even when the player response
/// is fresh. Refresh the Innertube player URL once before surfacing a 403.
async fn download_audio_with_fresh_stream(
    client: &reqwest::Client,
    video_id: &str,
    initial_audio: &YtmAudio,
) -> Result<Vec<u8>, String> {
    let mut audio = initial_audio.clone();
    for attempt in 0..2 {
        match download_audio_bytes(client, &audio.url).await {
            Ok(bytes) => return Ok(bytes),
            Err(error) if attempt == 0 && should_refresh_audio_url(&error) => {
                eprintln!("[ytm-dl] refreshing stream URL after {error}");
                audio = match ytm_audio_stream(client, video_id).await? {
                    Some(fresh_audio) => fresh_audio,
                    None => return Err(error),
                };
            }
            Err(error) => return Err(error),
        }
    }
    Err("audio download retry exhausted".to_string())
}

pub async fn ytm_cache_stream(video_id: String) -> Result<Option<YtmCachedTrack>, String> {
    let id = video_id.trim().to_string();
    if id.len() != 11
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Ok(None);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let audio = match ytm_audio_stream(&client, &id).await? {
        Some(audio) => audio,
        None => return Ok(None),
    };
    let dir = std::env::temp_dir().join("playervf-yt");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{id}.{}", audio_ext_for_mime(&audio.mime)));
    let fresh =
        path.exists() && std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0) > 1024;
    if !fresh {
        let bytes = download_audio_with_fresh_stream(&client, &id, &audio).await?;
        if bytes.len() < 1024 {
            return Err("audio download too small".to_string());
        }
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    }
    Ok(Some(YtmCachedTrack {
        path: path.to_string_lossy().to_string(),
        duration_secs: audio.duration_secs,
        format: audio_ext_for_mime(&audio.mime).to_string(),
        title: audio.title,
        artist: audio.artist,
    }))
}

pub async fn ytm_download_track(
    video_id: String,
    title: String,
    artist: String,
    album: Option<String>,
) -> Result<Option<String>, String> {
    let id = video_id.trim().to_string();
    if id.len() != 11
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Ok(None);
    }
    let title = title.trim().to_string();
    let artist = artist.trim().to_string();
    if title.is_empty() {
        return Err("missing title".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let audio = match ytm_audio_stream(&client, &id).await? {
        Some(audio) => audio,
        None => return Ok(None),
    };
    let ext = audio_ext_for_mime(&audio.mime);
    let music_dir = PathBuf::from(get_system_music_folder().ok_or("Music directory unavailable")?);
    let dir = music_dir.join("YouTube Music");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = if artist.is_empty() {
        sanitize_filename(&title)
    } else {
        sanitize_filename(&format!("{artist} - {title}"))
    };
    let _ = album;
    let path = dir.join(format!("{stem}.{ext}"));
    let fresh =
        path.exists() && std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0) > 1024;
    if !fresh {
        let bytes = download_audio_with_fresh_stream(&client, &id, &audio).await?;
        if bytes.len() < 1024 {
            return Err("audio download too small".to_string());
        }
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    }
    Ok(Some(path.to_string_lossy().to_string()))
}

#[allow(dead_code)]
fn extract_yt_video_id(html: &str) -> Option<String> {
    // Pattern 1: "videoId":"XXXXXXXXXXX" (standard JSON)
    if let Some(idx) = html.find("\"videoId\":\"") {
        let rest = &html[idx + 12..];
        if rest.len() >= 11 {
            let id: String = rest.chars().take(11).collect();
            if id.len() == 11
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(id);
            }
        }
    }
    // Pattern 2: "videoId":"\\u..." (escaped unicode)
    if let Some(idx) = html.find(r#""videoId":"\"#) {
        let rest = &html[idx + 13..];
        if rest.len() >= 11 {
            let id: String = rest.chars().take(11).collect();
            if id.len() == 11
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(id);
            }
        }
    }
    // Pattern 3: Regex fallback - videoId key
    if let Ok(re) = regex::Regex::new(r#""videoId"\s*:\s*"([a-zA-Z0-9_-]{11})""#) {
        if let Some(caps) = re.captures(html) {
            if let Some(match_id) = caps.get(1) {
                let id = match_id.as_str();
                if id.len() == 11 {
                    return Some(id.to_string());
                }
            }
        }
    }
    // Pattern 4: Watch URL /watch?v=
    if let Ok(watch_re) = regex::Regex::new(r#"(?:v=|watch\?v=|/shorts/)([a-zA-Z0-9_-]{11})"#) {
        if let Some(caps) = watch_re.captures(html) {
            if let Some(match_id) = caps.get(1) {
                return Some(match_id.as_str().to_string());
            }
        }
    }
    None
}

fn normalize_search_text(s: &str) -> String {
    let mut raw = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_alphanumeric() {
            for l in c.to_lowercase() {
                raw.push(l);
            }
        } else if c.is_whitespace() {
            raw.push(' ');
        }
    }
    let filler = [
        "topic", "official", "video", "audio", "lyric", "lyrics", "clip", "hq", "mv", "mvs",
    ];
    raw.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| !filler.iter().any(|f| *f == w.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Transliterate Cyrillic to Latin (Russian + Ukrainian + Belarusian).
/// Input is expected lowercased already; Latin chars pass through, so the
/// function is safe to apply to either side of a comparison.
fn transliterate_cyrillic_to_latin(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        let latin: Option<&str> = match c {
            'а' => Some("a"),
            'б' => Some("b"),
            'в' => Some("v"),
            'г' => Some("g"),
            'д' => Some("d"),
            'е' => Some("e"),
            'ё' => Some("yo"),
            'ж' => Some("zh"),
            'з' => Some("z"),
            'и' => Some("i"),
            'й' => Some("y"),
            'к' => Some("k"),
            'л' => Some("l"),
            'м' => Some("m"),
            'н' => Some("n"),
            'о' => Some("o"),
            'п' => Some("p"),
            'р' => Some("r"),
            'с' => Some("s"),
            'т' => Some("t"),
            'у' => Some("u"),
            'ф' => Some("f"),
            'х' => Some("kh"),
            'ц' => Some("ts"),
            'ч' => Some("ch"),
            'ш' => Some("sh"),
            'щ' => Some("shch"),
            'ъ' => Some(""),
            'ы' => Some("y"),
            'ь' => Some(""),
            'э' => Some("e"),
            'ю' => Some("yu"),
            'я' => Some("ya"),
            'і' => Some("i"),
            'ї' => Some("yi"),
            'є' => Some("ye"),
            'ґ' => Some("g"),
            'ў' => Some("u"),
            _ => None,
        };
        match latin {
            Some(s) => out.push_str(s),
            None => out.push(c),
        }
    }
    out
}

/// Title/byline match that also bridges Latin ↔ Cyrillic spellings: file
/// tags are often transliterated ("Korol I Shut") while YouTube titles are
/// native script ("Король и Шут"). Both inputs must already be normalized.
fn texts_match(candidate_norm: &str, query_norm: &str) -> bool {
    if query_norm.is_empty() {
        return true;
    }
    if candidate_norm.contains(query_norm) || query_norm.contains(candidate_norm) {
        return true;
    }
    let candidate_t = transliterate_cyrillic_to_latin(candidate_norm);
    let query_t = transliterate_cyrillic_to_latin(query_norm);
    candidate_t.contains(&query_t)
        || query_t.contains(&candidate_t)
        || candidate_norm.contains(&query_t)
        || query_norm.contains(&candidate_t)
        || words_overlap(candidate_norm, query_norm) >= 0.6
        || words_overlap(&candidate_t, &query_t) >= 0.6
}

/// True when two normalized words match: equal, contained, or sharing a stem
/// ("молния" vs "молнией"). Last-resort insurance for reworded titles.
fn words_match(a: &str, b: &str) -> bool {
    if a == b || a.contains(b) || b.contains(a) {
        return true;
    }
    let shared = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    let shortest = a.chars().count().min(b.chars().count());
    shared >= 5 && shared >= shortest.saturating_sub(2)
}

/// Fraction of query words found in the candidate (0.0..=1.0).
fn words_overlap(candidate_norm: &str, query_norm: &str) -> f64 {
    let query_words: Vec<&str> = query_norm.split_whitespace().collect();
    if query_words.is_empty() {
        return 1.0;
    }
    let candidate_words: Vec<&str> = candidate_norm.split_whitespace().collect();
    let hits = query_words
        .iter()
        .filter(|word| candidate_words.iter().any(|cand| words_match(cand, word)))
        .count();
    hits as f64 / query_words.len() as f64
}

fn read_json_string_at(html: &str, from: usize) -> Option<String> {
    let bytes = html.as_bytes();
    if from >= bytes.len() || bytes[from] != b'"' {
        return None;
    }
    let mut i = from + 1;
    let mut out = String::new();
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' {
            return Some(out);
        }
        if c == b'\\' {
            if i + 1 >= bytes.len() {
                return None;
            }
            let next = bytes[i + 1];
            match next {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\u{0008}'),
                b'f' => out.push('\u{000c}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    if i + 6 > bytes.len() {
                        return None;
                    }
                    let hex = std::str::from_utf8(&bytes[i + 2..i + 6]).ok()?;
                    let code = u32::from_str_radix(hex, 16).ok()?;
                    let ch = char::from_u32(code)?;
                    out.push(ch);
                    i += 6;
                    continue;
                }
                _ => return None,
            }
            i += 2;
        } else {
            let rest = &html[i..];
            let ch = rest.chars().next()?;
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    None
}

/// Parse a YouTube `lengthText` ("3:17", "1:02:03") into seconds.
/// Returns None for live/upcoming badges ("LIVE") or unparseable text.
fn parse_yt_length(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() || text.chars().any(|c| c.is_alphabetic()) {
        return None;
    }
    let parts: Vec<&str> = text.split(':').collect();
    let mut nums = Vec::with_capacity(parts.len());
    for part in parts {
        nums.push(part.trim().parse::<f64>().ok()?);
    }
    match nums.len() {
        1 => Some(nums[0]),
        2 => Some(nums[0] * 60.0 + nums[1]),
        3 => Some(nums[0] * 3600.0 + nums[1] * 60.0 + nums[2]),
        _ => None,
    }
}

/// Collect every title/artist-matching `videoRenderer` in page order with its
/// optional `lengthText` duration. Shared by the single and plural resolvers.
/// Slice of `window` starting just past `"key"`.
fn section_after_key<'a>(window: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("\"{key}\"");
    let pos = window.find(&pattern)?;
    Some(&window[pos + pattern.len()..])
}

/// Read a `"..."` string after `"key"`, tolerating whitespace around the
/// colon (`"videoId" : "..."` as well as `"videoId":"..."`).
fn find_key_string(window: &str, key: &str) -> Option<String> {
    let rest = section_after_key(window, key)?.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    read_json_string_at(rest, 0)
}

/// First `"text":"..."` under an already-descended JSON section.
fn first_text(section: &str) -> Option<String> {
    find_key_string(section, "text")
}

fn collect_renderers(html: &str, song: &str, artist: &str) -> Vec<(String, Option<f64>)> {
    let song_norm = normalize_search_text(song);
    let artist_norm = normalize_search_text(artist);
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut scanned = 0usize;
    while let Some(pos) = html[start..].find("\"videoRenderer\"") {
        let abs = start + pos;
        start = abs + 15; // len("\"videoRenderer\"")
        scanned += 1;
        if scanned > 300 {
            break;
        }
        // abs is a char boundary (find + ASCII offset), but abs + 12000 may
        // land inside a multi-byte char (Cyrillic titles) — slicing there
        // panics and takes down the tokio worker. Clamp down to a boundary.
        let window_end = html.floor_char_boundary((abs + 12000).min(html.len()));
        let window = &html[abs..window_end];
        let id = match find_key_string(window, "videoId") {
            Some(id) => id,
            None => continue,
        };
        if id.len() != 11
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        let title = section_after_key(window, "title")
            .and_then(first_text)
            .unwrap_or_default();
        let byline = section_after_key(window, "longBylineText")
            .and_then(first_text)
            .or_else(|| section_after_key(window, "shortBylineText").and_then(first_text))
            .unwrap_or_default();
        let t_norm = normalize_search_text(&title);
        let a_norm = normalize_search_text(&byline);
        if texts_match(&t_norm, &song_norm) && texts_match(&a_norm, &artist_norm) {
            let len_secs = section_after_key(window, "lengthText")
                .and_then(first_text)
                .and_then(|text| parse_yt_length(&text));
            out.push((id, len_secs));
        }
    }
    out
}

/// Rank candidates: duration fits first (page order), then the rest in page
/// order, deduplicated. Powers the "try next video" cycling.
fn rank_ids(
    candidates: &[(String, Option<f64>)],
    expected_secs: Option<f64>,
    limit: usize,
) -> Vec<String> {
    let tolerance = expected_secs
        .filter(|exp| *exp > 0.0)
        .map(|exp| (exp * 0.06).max(8.0));
    let mut seen = std::collections::HashSet::new();
    let mut ordered = Vec::new();
    if let (Some(exp), Some(tol)) = (expected_secs.filter(|e| *e > 0.0), tolerance) {
        for (id, len) in candidates {
            if ordered.len() >= limit {
                break;
            }
            if let Some(duration) = len {
                if (duration - exp).abs() <= tol && seen.insert(id.clone()) {
                    ordered.push(id.clone());
                }
            }
        }
    }
    for (id, _) in candidates {
        if ordered.len() >= limit {
            break;
        }
        if seen.insert(id.clone()) {
            ordered.push(id.clone());
        }
    }
    ordered
}

/// Returns the best `videoRenderer` whose parsed title and byline match the
/// requested song/artist. This keeps us from grabbing an unrelated channel or
/// playlist id that happens to appear earlier in the page.
///
/// When the expected track length is known, an upload whose `lengthText` is
/// within tolerance wins over an earlier title-only match — this rejects
/// wrong-video picks (covers, lyric re-uploads) that otherwise poison the
/// lyrics lookup with a degraded provider set.
#[allow(dead_code)]
fn extract_matching_youtube_video_id(
    html: &str,
    song: &str,
    artist: &str,
    expected_secs: Option<f64>,
) -> Option<String> {
    rank_ids(&collect_renderers(html, song, artist), expected_secs, 1)
        .into_iter()
        .next()
}

async fn search_page_ids(
    client: &reqwest::Client,
    url: &str,
    song: &str,
    artist: &str,
) -> Vec<(String, Option<f64>)> {
    let host = url
        .split("//")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("page");
    let res = match client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Accept-Language", "en-US,en;q=0.9,uk;q=0.8")
        .header("Cookie", "SOCS=CAI; CONSENT=YES+1")
        .send()
        .await
    {
        Ok(res) => res,
        Err(_) => {
            eprintln!("[yt-search] page {host} transport error");
            return Vec::new();
        }
    };
    let status = res.status();
    let html = match res.text().await {
        Ok(html) => html,
        Err(_) => return Vec::new(),
    };
    eprintln!("[yt-search] page {host} HTTP {status} bytes={}", html.len());
    if !status.is_success() {
        return Vec::new();
    }
    collect_renderers(&html, song, artist)
}

/// Concatenate a runs[{text}] array into plain text.
fn runs_text(value: &serde_json::Value) -> String {
    value
        .get("runs")
        .and_then(|runs| runs.as_array())
        .map(|runs| {
            runs.iter()
                .filter_map(|run| run.get("text").and_then(|text| text.as_str()))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn length_text_secs(renderer: &serde_json::Value) -> Option<f64> {
    if let Some(simple) = renderer
        .get("lengthText")
        .and_then(|length| length.get("simpleText"))
        .and_then(|text| text.as_str())
    {
        return parse_yt_length(simple);
    }
    let runs = renderer
        .get("lengthText")
        .map(runs_text)
        .unwrap_or_default();
    if runs.is_empty() {
        None
    } else {
        parse_yt_length(&runs)
    }
}

/// Pure extraction of matching (id, length) pairs from a youtubei search
/// response. Skips non-video items (channels, playlists, ads).
fn parse_youtubei_results(
    json: &serde_json::Value,
    song: &str,
    artist: &str,
) -> Vec<(String, Option<f64>)> {
    let song_norm = normalize_search_text(song);
    let artist_norm = normalize_search_text(artist);
    let mut out = Vec::new();
    let sections = json
        .pointer(
            "/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents",
        )
        .and_then(|contents| contents.as_array());
    let sections = match sections {
        Some(sections) => sections,
        None => return out,
    };
    for section in sections {
        let items = section
            .get("itemSectionRenderer")
            .and_then(|renderer| renderer.get("contents"))
            .and_then(|contents| contents.as_array());
        let items = match items {
            Some(items) => items,
            None => continue,
        };
        for item in items {
            let renderer = match item.get("videoRenderer") {
                Some(renderer) => renderer,
                None => continue,
            };
            let id = renderer
                .get("videoId")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if id.len() != 11
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                continue;
            }
            let title = renderer.get("title").map(runs_text).unwrap_or_default();
            let byline = renderer
                .get("longBylineText")
                .map(runs_text)
                .filter(|text| !text.is_empty())
                .or_else(|| renderer.get("shortBylineText").map(runs_text))
                .unwrap_or_default();
            let title_norm = normalize_search_text(&title);
            let byline_norm = normalize_search_text(&byline);
            if texts_match(&title_norm, &song_norm) && texts_match(&byline_norm, &artist_norm) {
                out.push((id.to_string(), length_text_secs(renderer)));
            }
        }
    }
    out
}

/// Transliterated query variant ("Дурак и молния ..." -> "durak i molniya ...").
/// None for pure-Latin queries (no second attempt needed).
fn transliterated_query_variant(query: &str) -> Option<String> {
    let lowered = query.to_lowercase();
    let translit = transliterate_cyrillic_to_latin(&lowered);
    if translit == lowered {
        None
    } else {
        Some(translit)
    }
}

/// Proven client snapshots (mirrors the reference yt_music_api defaults).
/// Dynamic/future versions get "Requested entity was not found" 404s —
/// Innertube only recognizes shipped client releases.
const YT_REMIX_VERSION: &str = "1.20250310.01.00";
const YT_WEB_VERSION: &str = "2.20250310.01.00";
/// Proven Android client release. Older releases get HTTP 400
/// ("Precondition check failed"); this one returns plain (non-ciphered)
/// audio URLs where WEB_REMIX answers UNPLAYABLE or cipher-only.
const YT_ANDROID_VERSION: &str = "20.10.38";
const YT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";
const YT_SEC_CH_UA_FULL: &str = "134.0.6998.44";

/// Base Innertube context with an optional visitor id.
fn innertube_context(
    client_name: &str,
    client_version: &str,
    visitor: Option<&str>,
) -> serde_json::Value {
    let mut client = serde_json::json!({
        "clientName": client_name,
        "clientVersion": client_version,
        "hl": "en",
        "gl": "US",
    });
    if client_name == "WEB_REMIX" {
        client["originalUrl"] = serde_json::Value::String("https://music.youtube.com".to_string());
    }
    if client_name == "ANDROID" {
        client["androidSdkVersion"] = serde_json::Value::from(30);
    }
    if let Some(data) = visitor {
        client["visitorData"] = serde_json::Value::String(data.to_string());
    }
    serde_json::json!({
        "client": client,
        "user": { "lockedSafetyMode": false },
    })
}

const YT_API_KEY_DEFAULT: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";

#[derive(Debug, Clone)]
struct YtApiAuth {
    visitor: Option<String>,
    api_key: String,
}

static YT_AUTH: std::sync::OnceLock<YtApiAuth> = std::sync::OnceLock::new();

/// Homepage context scraped once per process: visitorData + the page's own
/// INNERTUBE_API_KEY. Hardcoded keys rot (404s) — the live page always knows
/// the current one.
async fn youtube_auth(client: &reqwest::Client) -> YtApiAuth {
    if let Some(cached) = YT_AUTH.get() {
        return cached.clone();
    }
    let (visitor, api_key) = fetch_homepage_context(client).await;
    let from_homepage = api_key.is_some();
    let auth = YtApiAuth {
        visitor: visitor.clone(),
        api_key: api_key.unwrap_or_else(|| YT_API_KEY_DEFAULT.to_string()),
    };
    eprintln!(
        "[yt-search] auth key={} visitor={}",
        if from_homepage { "homepage" } else { "default" },
        if visitor.is_some() { "ok" } else { "missing" }
    );
    let _ = YT_AUTH.set(auth.clone());
    auth
}

async fn fetch_homepage_context(client: &reqwest::Client) -> (Option<String>, Option<String>) {
    let html = match client
        .get("https://music.youtube.com/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Cookie", "SOCS=CAI; CONSENT=YES+1")
        .send()
        .await
    {
        Ok(res) => match res.text().await {
            Ok(html) => html,
            Err(_) => return (None, None),
        },
        Err(_) => return (None, None),
    };
    let marker = "ytcfg.set(";
    let pos = match html.find(marker) {
        Some(pos) => pos,
        None => return (None, None),
    };
    let rest = html[pos + marker.len()..].trim_start();
    if !rest.starts_with('{') {
        return (None, None);
    }
    let abs = pos + marker.len() + (html[pos + marker.len()..].len() - rest.len());
    let raw = match extract_json_bracket(&html, abs) {
        Some(raw) => raw,
        None => return (None, None),
    };
    let json: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(json) => json,
        Err(_) => return (None, None),
    };
    let visitor = json
        .pointer("/INNERTUBE_CONTEXT/client/visitorData")
        .and_then(|value| value.as_str())
        .filter(|data| !data.is_empty())
        .map(str::to_string);
    let api_key = json
        .pointer("/INNERTUBE_API_KEY")
        .and_then(|value| value.as_str())
        .filter(|key| !key.is_empty())
        .map(str::to_string);
    (visitor, api_key)
}

/// Structured search via the Innertube WEB client: ~20 JSON results with
/// reliable title/byline/length. Falls back to empty (HTML strategies cover).
async fn youtubei_search_ids(
    client: &reqwest::Client,
    song: &str,
    artist: &str,
    query: &str,
) -> Vec<(String, Option<f64>)> {
    let found = youtubei_search_query(client, song, artist, query).await;
    if !found.is_empty() {
        return found;
    }
    // A Cyrillic query can miss Latin-titled uploads entirely — retry once
    // fully transliterated before giving up on the structured API.
    match transliterated_query_variant(query) {
        Some(alt) => {
            eprintln!("[yt-search] retrying transliterated query");
            youtubei_search_query(client, song, artist, &alt).await
        }
        None => Vec::new(),
    }
}

async fn youtubei_search_query(
    client: &reqwest::Client,
    song: &str,
    artist: &str,
    query: &str,
) -> Vec<(String, Option<f64>)> {
    let auth = youtube_auth(client).await;
    let visitor = auth.visitor.clone();
    let body = serde_json::json!({
        "context": innertube_context("WEB", YT_WEB_VERSION, visitor.as_deref()),
        "query": query,
    });
    let mut req = client
        .post(format!(
            "https://www.youtube.com/youtubei/v1/search?key={}&prettyPrint=false",
            auth.api_key
        ))
        .header("User-Agent", YT_UA)
        .header("Content-Type", "application/json")
        .header("Origin", "https://www.youtube.com")
        .header("Referer", "https://www.youtube.com/")
        .header("x-youtube-client-name", "1")
        .header("x-youtube-client-version", YT_WEB_VERSION)
        .header("sec-ch-ua-full-version", YT_SEC_CH_UA_FULL);
    if let Some(data) = visitor.as_deref() {
        req = req.header("X-Goog-Visitor-Id", data);
    }
    let res = match req.json(&body).send().await {
        Ok(res) => res,
        Err(_) => {
            eprintln!("[yt-search] youtubei transport error");
            return Vec::new();
        }
    };
    if !res.status().is_success() {
        let status = res.status();
        let snippet: String = res
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect();
        eprintln!(
            "[yt-search] youtubei HTTP {status} key=..{} body={snippet:?}",
            &auth.api_key[auth.api_key.len().saturating_sub(4)..]
        );
        return Vec::new();
    }
    let body = match res.text().await {
        Ok(body) => body,
        Err(_) => return Vec::new(),
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(json) => json,
        Err(_) => {
            eprintln!("[yt-search] youtubei bad json bytes={}", body.len());
            return Vec::new();
        }
    };
    let found = parse_youtubei_results(&json, song, artist);
    if found.is_empty() {
        eprintln!(
            "[yt-search] youtubei empty bytes={} sections={} hasVideoRenderer={}",
            body.len(),
            youtubei_section_kinds(&json),
            body.contains("videoRenderer")
        );
    }
    found
}

/// Comma-joined renderer keys of each top-level section (diagnostics only).
fn youtubei_section_kinds(json: &serde_json::Value) -> String {
    json.pointer(
        "/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents",
    )
    .and_then(|contents| contents.as_array())
    .map(|sections| {
        sections
            .iter()
            .map(|section| {
                section
                    .as_object()
                    .and_then(|obj| obj.keys().next().map(String::as_str))
                    .unwrap_or("?")
            })
            .collect::<Vec<_>>()
            .join(",")
    })
    .unwrap_or_else(|| "no-section-list".to_string())
}

/// Plain text of a {"runs":[...]} or {"simpleText":...} node.
fn ytm_text(value: &serde_json::Value) -> String {
    if let Some(simple) = value.get("simpleText").and_then(|text| text.as_str()) {
        return simple.to_string();
    }
    runs_text(value)
}

fn ytm_watch_id(value: &serde_json::Value) -> Option<String> {
    let watch = value
        .get("watchEndpoint")
        .or_else(|| {
            value
                .get("playNavigationEndpoint")
                .and_then(|e| e.get("watchEndpoint"))
        })
        .or_else(|| {
            value
                .get("navigationEndpoint")
                .and_then(|e| e.get("watchEndpoint"))
        })?;
    let id = watch.get("videoId")?.as_str()?;
    if id.len() == 11
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(id.to_string())
    } else {
        None
    }
}

fn ytm_runs(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value
        .get("runs")
        .and_then(|runs| runs.as_array())
        .cloned()
        .unwrap_or_default()
}

fn ytm_run_text(run: &serde_json::Value) -> &str {
    run.get("text").and_then(|text| text.as_str()).unwrap_or("")
}

/// Meaningful subtitle run texts: separators and type words dropped, since
/// the artist is rarely the first run. YouTube Music localizes the type word,
/// so non-Latin locales ("Пісня", "Відео", "Трек", …) are listed too.
fn ytm_subtitle_texts(subtitle: &serde_json::Value) -> Vec<String> {
    const TYPE_WORDS: &[&str] = &[
        "song",
        "video",
        "artist",
        "album",
        "playlist",
        "single",
        "ep",
        "radio",
        // Cyrillic (uk/ru) localization of the same type words.
        "пісня",
        "пісні",
        "відео",
        "трек",
        "треки",
        "альбом",
        "сингл",
        "радіо",
        "песня",
        "песни",
        "видео",
        "альбом",
        "сингл",
        "радио",
    ];
    ytm_runs(subtitle)
        .iter()
        .map(ytm_run_text)
        .filter(|text| {
            let trimmed = text.trim();
            !trimmed.is_empty()
                && trimmed != "•"
                && trimmed != "·"
                && !TYPE_WORDS.contains(&trimmed.to_lowercase().as_str())
        })
        .map(str::to_string)
        .collect()
}

/// One musicTwoRowItemRenderer tile (Videos shelf: official/lyric videos —
/// prime lyrics-videoId sources when the song entity is missing).
fn parse_ytm_video_tile(
    renderer: &serde_json::Value,
    song_norm: &str,
    artist_norm: &str,
) -> Option<(String, Option<f64>)> {
    let title = ytm_text(renderer.get("title").unwrap_or(&serde_json::Value::Null));
    let subtitle_texts =
        ytm_subtitle_texts(renderer.get("subtitle").unwrap_or(&serde_json::Value::Null));
    if !texts_match(&normalize_search_text(&title), song_norm)
        || !subtitle_texts
            .iter()
            .any(|text| texts_match(&normalize_search_text(text), artist_norm))
    {
        return None;
    }
    let id = ytm_watch_id(
        renderer
            .get("navigationEndpoint")
            .unwrap_or(&serde_json::Value::Null),
    )
    .or_else(|| {
        renderer
            .get("overlay")
            .and_then(|overlay| overlay.get("musicItemThumbnailOverlayRenderer"))
            .and_then(|overlay| overlay.get("content"))
            .and_then(|content| content.get("musicPlayButtonRenderer"))
            .and_then(|button| button.get("playNavigationEndpoint"))
            .and_then(ytm_watch_id)
    });
    // Tiles rarely carry a usable length — duration ranking skips them
    // gracefully (rank_ids treats missing lengths as page-order fallback).
    id.map(|id| (id, None))
}

/// Pure extraction of matching (id, length) pairs from a YouTube Music
/// search response: top-result card + song shelf. True music entities —
/// the ids lyrics backends actually index.
fn parse_ytm_results(
    json: &serde_json::Value,
    song: &str,
    artist: &str,
) -> Vec<(String, Option<f64>)> {
    let song_norm = normalize_search_text(song);
    let artist_norm = normalize_search_text(artist);
    let mut out = Vec::new();
    let sections = json
        .pointer("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .and_then(|contents| contents.as_array());
    let sections = match sections {
        Some(sections) => sections,
        None => return out,
    };
    for section in sections {
        // Top-result card.
        if let Some(card) = section.get("musicCardShelfRenderer") {
            let title = card.get("title").map(ytm_text).unwrap_or_default();
            let subtitle_texts =
                ytm_subtitle_texts(card.get("subtitle").unwrap_or(&serde_json::Value::Null));
            let card_artist_ok = subtitle_texts
                .iter()
                .any(|text| texts_match(&normalize_search_text(text), &artist_norm));
            let id = card
                .get("title")
                .and_then(|title| title.get("runs"))
                .and_then(|runs| runs.as_array())
                .and_then(|runs| runs.iter().filter_map(|run| ytm_watch_id(run)).next())
                .or_else(|| ytm_watch_id(card));
            if let Some(id) = id {
                if texts_match(&normalize_search_text(&title), &song_norm) && card_artist_ok {
                    out.push((id, None));
                }
            }
            // Songs listed under the top-result card.
            if let Some(contents) = card
                .get("contents")
                .and_then(|contents| contents.as_array())
            {
                for item in contents {
                    if let Some(row) = item.get("musicResponsiveListItemRenderer") {
                        let columns = row
                            .get("flexColumns")
                            .and_then(|columns| columns.as_array());
                        let columns = match columns {
                            Some(columns) => columns,
                            None => continue,
                        };
                        let column_text = |index: usize| {
                            columns
                                .get(index)
                                .and_then(|column| {
                                    column.get("musicResponsiveListItemFlexColumnRenderer")
                                })
                                .and_then(|column| column.get("text"))
                                .map(ytm_text)
                                .unwrap_or_default()
                        };
                        let row_title = column_text(0);
                        let row_subtitle_texts = columns
                            .get(1)
                            .and_then(|column| {
                                column.get("musicResponsiveListItemFlexColumnRenderer")
                            })
                            .and_then(|column| column.get("text"))
                            .map(|text| ytm_subtitle_texts(text))
                            .unwrap_or_default();
                        let row_artist_ok = row_subtitle_texts
                            .iter()
                            .any(|text| texts_match(&normalize_search_text(text), &artist_norm));
                        if !texts_match(&normalize_search_text(&row_title), &song_norm)
                            || !row_artist_ok
                        {
                            continue;
                        }
                        let row_id = row
                            .get("overlay")
                            .and_then(|overlay| overlay.get("musicItemThumbnailOverlayRenderer"))
                            .and_then(|overlay| overlay.get("content"))
                            .and_then(|content| content.get("musicPlayButtonRenderer"))
                            .and_then(|button| button.get("playNavigationEndpoint"))
                            .and_then(ytm_watch_id);
                        let row_length = row
                            .get("fixedColumns")
                            .and_then(|columns| columns.as_array())
                            .and_then(|columns| columns.first())
                            .and_then(|column| {
                                column.get("musicResponsiveListItemFixedColumnRenderer")
                            })
                            .and_then(|column| column.get("text"))
                            .map(ytm_text)
                            .and_then(|text| parse_yt_length(&text));
                        if let Some(row_id) = row_id {
                            out.push((row_id, row_length));
                        }
                    }
                }
            }
        }
        // Song shelf rows.
        if let Some(shelf) = section.get("musicShelfRenderer") {
            let items = shelf
                .get("contents")
                .and_then(|contents| contents.as_array());
            let items = match items {
                Some(items) => items,
                None => continue,
            };
            for item in items {
                if let Some(tile) = item.get("musicTwoRowItemRenderer") {
                    if let Some(found) = parse_ytm_video_tile(tile, &song_norm, &artist_norm) {
                        out.push(found);
                    }
                    continue;
                }
                let renderer = match item.get("musicResponsiveListItemRenderer") {
                    Some(renderer) => renderer,
                    None => continue,
                };
                let columns = renderer
                    .get("flexColumns")
                    .and_then(|columns| columns.as_array());
                let columns = match columns {
                    Some(columns) => columns,
                    None => continue,
                };
                let column_text = |index: usize| {
                    columns
                        .get(index)
                        .and_then(|column| column.get("musicResponsiveListItemFlexColumnRenderer"))
                        .and_then(|column| column.get("text"))
                        .map(ytm_text)
                        .unwrap_or_default()
                };
                let title = column_text(0);
                let subtitle_texts = columns
                    .get(1)
                    .and_then(|column| column.get("musicResponsiveListItemFlexColumnRenderer"))
                    .and_then(|column| column.get("text"))
                    .map(|text| ytm_subtitle_texts(text))
                    .unwrap_or_default();
                let row_artist_ok = subtitle_texts
                    .iter()
                    .any(|text| texts_match(&normalize_search_text(text), &artist_norm));
                let id = renderer
                    .get("overlay")
                    .and_then(|overlay| overlay.get("musicItemThumbnailOverlayRenderer"))
                    .and_then(|overlay| overlay.get("content"))
                    .and_then(|content| content.get("musicPlayButtonRenderer"))
                    .and_then(|button| button.get("playNavigationEndpoint"))
                    .and_then(ytm_watch_id);
                let length = renderer
                    .get("fixedColumns")
                    .and_then(|columns| columns.as_array())
                    .and_then(|columns| columns.first())
                    .and_then(|column| column.get("musicResponsiveListItemFixedColumnRenderer"))
                    .and_then(|column| column.get("text"))
                    .map(ytm_text)
                    .and_then(|text| parse_yt_length(&text));
                if let (Some(id), true) = (
                    id,
                    texts_match(&normalize_search_text(&title), &song_norm) && row_artist_ok,
                ) {
                    out.push((id, length));
                }
            }
        }
    }
    out
}

/// YouTube Music search via Innertube WEB_REMIX: true music entities (what
/// the lyrics backends index). Songs filter first, unfiltered fallback.
async fn ytm_search_ids(
    client: &reqwest::Client,
    song: &str,
    artist: &str,
    query: &str,
) -> Vec<(String, Option<f64>)> {
    // Songs-filter chip; falls back to an unfiltered search below.
    const SONGS_FILTER: &str = "EgWKAQIIAWoQEAMQBBAJEAoQEAoMEAkQChAFEAM=";
    let auth = youtube_auth(client).await;
    let visitor = auth.visitor.clone();
    let version = YT_REMIX_VERSION;
    for params in [Some(SONGS_FILTER), None] {
        let mut body = serde_json::json!({
            "context": innertube_context("WEB_REMIX", &version, visitor.as_deref()),
            "query": query,
        });
        if let Some(filter) = params {
            body["params"] = serde_json::Value::String(filter.to_string());
        }
        let mut req = client
            .post(format!(
                "https://music.youtube.com/youtubei/v1/search?key={}&prettyPrint=false",
                auth.api_key
            ))
            .header("User-Agent", YT_UA)
            .header("Content-Type", "application/json")
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com/")
            .header("x-youtube-client-name", "67")
            .header("x-youtube-client-version", YT_REMIX_VERSION)
            .header("sec-ch-ua-full-version", YT_SEC_CH_UA_FULL);
        if let Some(data) = visitor.as_deref() {
            req = req.header("X-Goog-Visitor-Id", data);
        }
        let res = match req.json(&body).send().await {
            Ok(res) => res,
            Err(_) => continue,
        };
        if !res.status().is_success() {
            let status = res.status();
            let snippet: String = res
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            eprintln!(
                "[yt-search] ytm HTTP {status} key=..{} body={snippet:?}",
                &auth.api_key[auth.api_key.len().saturating_sub(4)..]
            );
            continue;
        }
        match res.json::<serde_json::Value>().await {
            Ok(json) => {
                let found = parse_ytm_results(&json, song, artist);
                if !found.is_empty() {
                    return found;
                }
                eprintln!(
                    "[yt-search] ytm empty hasShelf={}",
                    json.to_string().contains("musicShelfRenderer")
                );
            }
            Err(_) => {
                eprintln!("[yt-search] ytm bad json");
                continue;
            }
        }
    }
    Vec::new()
}

/// True when the candidate carries a length within tolerance of expected.
fn duration_fit(
    candidates: &[(String, Option<f64>)],
    id: &str,
    expected_secs: Option<f64>,
) -> bool {
    let exp = match expected_secs.filter(|exp| *exp > 0.0) {
        Some(exp) => exp,
        None => return false,
    };
    let tol = (exp * 0.06).max(8.0);
    candidates
        .iter()
        .find(|(candidate_id, _)| candidate_id == id)
        .and_then(|(_, len)| *len)
        .map(|duration| (duration - exp).abs() <= tol)
        .unwrap_or(false)
}

fn feat_cleaned_query(song_trimmed: &str, artist_trimmed: &str) -> Option<String> {
    let clean_song = song_trimmed
        .split("feat")
        .next()
        .unwrap_or(song_trimmed)
        .split("ft.")
        .next()
        .unwrap_or(song_trimmed)
        .trim();
    if clean_song != song_trimmed {
        Some(format!("{clean_song} {artist_trimmed}").trim().to_string())
    } else {
        None
    }
}

async fn youtube_search_pages(
    client: &reqwest::Client,
    song: &str,
    artist: &str,
    query: &str,
    clean_query: Option<&str>,
) -> Vec<(String, Option<f64>)> {
    let mut out = search_page_ids(
        client,
        &format!(
            "https://www.youtube.com/results?search_query={}",
            urlencoding_fallback(query)
        ),
        song,
        artist,
    )
    .await;
    out.extend(
        search_page_ids(
            client,
            &format!(
                "https://music.youtube.com/search?q={}",
                urlencoding_fallback(query)
            ),
            song,
            artist,
        )
        .await,
    );
    if let Some(clean) = clean_query {
        out.extend(
            search_page_ids(
                client,
                &format!(
                    "https://www.youtube.com/results?search_query={}",
                    urlencoding_fallback(clean)
                ),
                song,
                artist,
            )
            .await,
        );
    }
    out
}

/// Ranked candidate videoIds for a track (duration fits first). Backs the
/// "try next video match" button when the top pick yields weak lyrics.

pub async fn resolve_youtube_video_ids(
    song: String,
    artist: String,
    duration_secs: Option<f64>,
    limit: Option<usize>,
) -> Result<Vec<String>, String> {
    let limit = limit.unwrap_or(5).clamp(1, 10);
    let song_trimmed = song.trim().to_string();
    let artist_trimmed = artist.trim().to_string();
    let q = if artist_trimmed.is_empty() {
        song_trimmed.clone()
    } else {
        format!("{song_trimmed} {artist_trimmed}")
    };
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let clean_q = feat_cleaned_query(&song_trimmed, &artist_trimmed);
    let mut all = ytm_search_ids(&client, &song, &artist, &q).await;
    all.extend(youtubei_search_ids(&client, &song, &artist, &q).await);
    if all.is_empty() {
        all.extend(youtube_search_pages(&client, &song, &artist, &q, clean_q.as_deref()).await);
    }
    let ids = rank_ids(&all, duration_secs, limit);
    eprintln!(
        "[yt-search] many q=\"{q}\" candidates={} returned={}",
        all.len(),
        ids.len()
    );
    Ok(ids)
}

pub async fn resolve_youtube_video_id(
    song: String,
    artist: String,
    duration_secs: Option<f64>,
) -> Result<Option<String>, String> {
    let song_trimmed = song.trim().to_string();
    let artist_trimmed = artist.trim().to_string();
    let q = if artist_trimmed.is_empty() {
        song_trimmed.clone()
    } else {
        format!("{song_trimmed} {artist_trimmed}")
    };
    if q.is_empty() {
        return Ok(None);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| e.to_string())?;
    let clean_q = feat_cleaned_query(&song_trimmed, &artist_trimmed);

    // Strategy 0: YouTube Music entities first (what lyrics backends index),
    // then structured YouTube search — reliable title/byline/length.
    // A duration-verified fit (or any match when no duration is known) wins
    // immediately, so the common case costs small JSON calls, not pages.
    let mut api = ytm_search_ids(&client, &song, &artist, &q).await;
    api.extend(youtubei_search_ids(&client, &song, &artist, &q).await);
    let quick = rank_ids(&api, duration_secs, 1).into_iter().next();
    let quick_ok = quick
        .as_deref()
        .map(|id| duration_fit(&api, id, duration_secs))
        .unwrap_or(false);
    if quick.is_some() && (quick_ok || duration_secs.filter(|exp| *exp > 0.0).is_none()) {
        eprintln!(
            "[yt-search] single q=\"{q}\" via=structured picked={}",
            quick.as_deref().unwrap_or("none")
        );
        return Ok(quick);
    }

    // Strategies 1-3: HTML search pages (also backstop a blocked youtubei).
    // NOTE: no blind first-videoId fallback — an unrelated id poisons the
    // lyrics lookup (and its 30-day cache) worse than an honest NotFound.
    let mut all = api;
    let api_len = all.len();
    all.extend(youtube_search_pages(&client, &song, &artist, &q, clean_q.as_deref()).await);
    let picked = rank_ids(&all, duration_secs, 1).into_iter().next();
    eprintln!(
        "[yt-search] single q=\"{q}\" via=pages api={api_len} total={} picked={}",
        all.len(),
        picked.as_deref().unwrap_or("none")
    );
    Ok(picked)
}

fn urlencoding_fallback(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ── YouTube Music login ───────────────────────────────────────────────────────

/// Official YouTube Music sign-in entry point: the Google account chooser for
/// the YouTube/YouTube Music web flow (Ukrainian locale, music template),
/// whose `continue` chain lands on https://music.youtube.com/ after sign-in.
/// PlayerVF only hands this URL to the system browser: Google owns the
/// account chooser and login flow, so no credentials, cookies or session
/// tokens are ever collected by the app, and opening the page grants the app
/// no access token. NOTE: the ephemeral per-session `dsh` query value is
/// intentionally omitted — Google re-issues it when the page loads.
const YTM_LOGIN_URL: &str = "https://accounts.google.com/v3/signin/accountchooser?continue=https://www.youtube.com/signin?action_handle_signin%3Dtrue%26app%3Ddesktop%26hl%3Duk%26next%3Dhttps%253A%252F%252Fmusic.youtube.com%252F%26feature%3D__FEATURE__&hl=uk&ltmpl=music&passive=true&service=youtube&uilel=3&flowName=GlifWebSignIn&flowEntry=ServiceLogin";

/// Opens the YouTube Music sign-in flow in the user's default browser so the
/// sign-in happens on Google's and YouTube's own pages.

pub fn open_ytm_login() -> Result<(), String> {
    let mut command = if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "android") {
        let mut command = std::process::Command::new("am");
        command.args(["start", "-a", "android.intent.action.VIEW", "-d"]);
        command
    } else {
        std::process::Command::new("xdg-open")
    };
    command
        .arg(YTM_LOGIN_URL)
        .spawn()
        .map(drop)
        .map_err(|error| format!("Could not open the system browser: {error}"))
}

// ── Entry ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "playervf-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("current time")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create test directory");
        path
    }

    fn write_wav(path: &Path) {
        // A tiny PCM WAV is enough for metadata parsing and keeps these tests self-contained.
        let sample_rate = 8_000u32;
        let samples = [0i16; 8];
        let data_size = (samples.len() * std::mem::size_of::<i16>()) as u32;
        let riff_size = 36 + data_size;
        let mut bytes = Vec::with_capacity((44 + data_size) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_size.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        fs::write(path, bytes).expect("write wav");
    }

    fn write_opus(path: &Path) {
        use opus_pure::{Application, OggOpusWriter, OpusEncoder, OpusHead, MAX_PACKET_BYTES};
        use std::io::BufWriter;

        let mut encoder =
            OpusEncoder::new(48_000, 1, Application::Audio).expect("create Opus encoder");
        let head = OpusHead::for_encoder(&encoder, 48_000);
        let file = File::create(path).expect("create opus");
        let mut writer =
            OggOpusWriter::new(BufWriter::new(file), head).expect("create Ogg Opus writer");
        let samples = vec![0.25f32; 960];
        let mut packet = vec![0u8; MAX_PACKET_BYTES];
        let packet_len = encoder
            .encode(&samples, 960, &mut packet)
            .expect("encode Opus packet");

        writer
            .write_packet(&packet[..packet_len])
            .expect("write Opus packet");
        writer.finish().expect("finish Opus file");
    }

    #[test]
    fn decodes_playable_wav_files_before_replacing_the_sink() {
        let root = temp_directory("decode-wav");
        let song = root.join("song.wav");
        write_wav(&song);

        let (_decoder, duration_secs) = decode_track(&song.to_string_lossy()).expect("decode wav");

        assert!(duration_secs > 0.0);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn scans_and_decodes_opus_files_with_unicode_names() {
        let root = temp_directory("decode-opus");
        let song = root
            .join("\u{0430}\u{043d}\u{0433}\u{0435}\u{043b}\u{043e}\u{0447}\u{0435}\u{043a}.opus");
        write_opus(&song);

        let track = read_track(&song).expect("scan Opus");
        let (_source, duration_secs) = decode_track(&song.to_string_lossy()).expect("decode Opus");

        assert_eq!(track.format, "opus");
        assert!(duration_secs > 0.0);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn reports_invalid_media_without_panicking() {
        let root = temp_directory("decode-invalid");
        let song = root.join("broken.mp3");
        fs::write(&song, b"not an audio file").expect("write invalid media");

        let error = match decode_track(&song.to_string_lossy()) {
            Ok(_) => panic!("invalid media should not decode"),
            Err(error) => error,
        };

        assert!(error.contains("Cannot decode broken.mp3"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn metered_source_tracks_loudness_with_attack_and_release() {
        use rodio::Source as _;

        struct Constant {
            remaining: usize,
            level: f32,
        }
        impl Iterator for Constant {
            type Item = f32;
            fn next(&mut self) -> Option<f32> {
                if self.remaining == 0 {
                    return None;
                }
                self.remaining -= 1;
                Some(self.level)
            }
        }
        impl rodio::Source for Constant {
            fn current_span_len(&self) -> Option<usize> {
                None
            }
            fn channels(&self) -> u16 {
                1
            }
            fn sample_rate(&self) -> u32 {
                48_000
            }
            fn total_duration(&self) -> Option<Duration> {
                None
            }
            fn try_seek(&mut self, _pos: Duration) -> Result<(), rodio::source::SeekError> {
                Ok(())
            }
        }

        let meter = Arc::new(Mutex::new(LevelMeter::default()));
        // 0.5 amplitude → RMS 0.5 → target 1.0 after boost+clamp.
        let mut loud = MeteredSource::new(
            Box::new(Constant {
                remaining: 10 * 2048,
                level: 0.5,
            }),
            Arc::clone(&meter),
        );
        let mut count = 0;
        while loud.next().is_some() {
            count += 1;
        }
        assert_eq!(count, 10 * 2048);
        // Passthrough must preserve decoding geometry.
        assert_eq!(loud.channels(), 1);
        assert_eq!(loud.sample_rate(), 48_000);
        let value = meter.lock().expect("meter").value;
        assert!(
            value > 0.95,
            "loud signal should drive the meter near 1, got {value}"
        );

        // Silence releases back down (target 0 each window).
        let mut quiet = MeteredSource::new(
            Box::new(Constant {
                remaining: 4 * 2048,
                level: 0.0,
            }),
            Arc::clone(&meter),
        );
        while quiet.next().is_some() {}
        let cooled = meter.lock().expect("meter").value;
        assert!(
            cooled < value * 0.25,
            "silence should release the meter, got {cooled}"
        );

        // Non-finite samples are zeroed, never poison the meter.
        let mut poison = MeteredSource::new(
            Box::new(Constant {
                remaining: 2048,
                level: f32::NAN,
            }),
            Arc::clone(&meter),
        );
        while poison.next().is_some() {}
        assert!(meter.lock().expect("meter").value.is_finite());
    }

    #[test]
    fn scans_supported_files_when_metadata_is_unreadable() {
        let root = temp_directory("scan-unreadable-metadata");
        let song = root.join("untagged.mp3");
        fs::write(&song, b"not an audio file").expect("write unsupported media");

        let result = scan_sources(vec![source("file", LibrarySourceKind::File, &song)], None);

        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.tracks[0].title, "untagged");
        fs::remove_dir_all(root).expect("remove test directory");
    }

    fn source(id: &str, kind: LibrarySourceKind, path: &Path) -> LibrarySource {
        LibrarySource {
            id: id.into(),
            kind,
            path: path.to_string_lossy().into_owned(),
        }
    }

    #[test]
    fn scans_folder_sources_recursively() {
        let root = temp_directory("folder");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create nested directory");
        write_wav(&nested.join("song.wav"));

        let result = scan_sources(
            vec![source("folder", LibrarySourceKind::Folder, &root)],
            None,
        );

        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.source_statuses[0].state, SourceScanState::Ready);
        assert_eq!(result.source_statuses[0].track_count, 1);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn scans_individual_file_sources() {
        let root = temp_directory("file");
        let song = root.join("song.wav");
        write_wav(&song);

        let result = scan_sources(vec![source("file", LibrarySourceKind::File, &song)], None);

        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.source_statuses[0].state, SourceScanState::Ready);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn unavailable_source_does_not_block_available_sources() {
        let root = temp_directory("missing");
        let song = root.join("song.wav");
        write_wav(&song);
        let missing = root.join("gone");

        let result = scan_sources(
            vec![
                source("missing", LibrarySourceKind::Folder, &missing),
                source("file", LibrarySourceKind::File, &song),
            ],
            None,
        );

        assert_eq!(result.tracks.len(), 1);
        assert_eq!(
            result.source_statuses[0].state,
            SourceScanState::Unavailable
        );
        assert_eq!(result.source_statuses[1].state, SourceScanState::Ready);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn deduplicates_canonical_paths_from_overlapping_sources() {
        let root = temp_directory("dedupe");
        let song = root.join("song.wav");
        write_wav(&song);

        let result = scan_sources(
            vec![
                source("folder", LibrarySourceKind::Folder, &root),
                source("file", LibrarySourceKind::File, &song),
            ],
            None,
        );

        assert_eq!(result.tracks.len(), 1);
        assert_eq!(result.source_statuses[0].track_count, 1);
        assert_eq!(result.source_statuses[1].track_count, 0);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn keeps_system_music_first_when_caller_omits_or_duplicates_it() {
        let root = temp_directory("system-first");
        let folder = source("folder", LibrarySourceKind::Folder, &root);
        let duplicate_system = LibrarySource {
            id: "different-system-id".into(),
            kind: LibrarySourceKind::System,
            path: "ignored".into(),
        };

        let sources = sources_with_system_music(vec![folder, duplicate_system]);

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].id, "system-music");
        assert_eq!(sources[0].kind, LibrarySourceKind::System);
        assert_eq!(sources[1].kind, LibrarySourceKind::Folder);
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn parses_youtube_length_text() {
        assert_eq!(parse_yt_length("3:17"), Some(197.0));
        assert_eq!(parse_yt_length("1:02:03"), Some(3723.0));
        assert_eq!(parse_yt_length("0:45"), Some(45.0));
        assert_eq!(parse_yt_length("LIVE"), None);
        assert_eq!(parse_yt_length(""), None);
        assert_eq!(parse_yt_length("SHORTS"), None);
    }

    fn renderer_html(id: &str, title: &str, artist: &str, length: &str) -> String {
        format!(
            r#""videoRenderer":{{"videoId":"{id}","title":{{"runs":[{{"text":"{title}"}}]}},"longBylineText":{{"runs":[{{"text":"{artist}"}}]}},"lengthText":{{"runs":[{{"text":"{length}"}}]}}}}"#,
        )
    }

    #[test]
    fn collect_renderers_survives_multibyte_split_at_window_edge() {
        // Regression: `&html[abs..abs + 12000]` panicked when byte 12000
        // landed inside a 2-byte Cyrillic char (tokio worker crash in
        // production: "end byte index ... is not a char boundary ... 'я'").
        let prefix = renderer_html("UlwfDIf8e3Q", "Король и Шут", "Король и Шут", "3:17");
        let mut html = prefix;
        while html.len() < 11999 {
            html.push('x');
        }
        assert_eq!(html.len(), 11999);
        // Bytes 11999..12001 are the two bytes of 'я': window_end = 12000
        // would split the char without the floor_char_boundary clamp.
        assert!(!html.is_char_boundary(12000));
        html.push_str("яяя хвост");
        let found = collect_renderers(&html, "Король и Шут", "Король и Шут");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "UlwfDIf8e3Q");
        assert_eq!(found[0].1, Some(197.0));
    }

    #[test]
    fn prefers_duration_matching_upload_over_earlier_mismatch() {
        // Wrong-video pick first (3:03), correct upload (3:17) second.
        let html = format!(
            "{}---{}",
            renderer_html("jyFivH9gxz0", "Bumble Bee", "Bambee", "3:03"),
            renderer_html("UlwfDIf8e3Q", "Bumble Bee", "Bambee", "3:17"),
        );
        assert_eq!(
            extract_matching_youtube_video_id(&html, "Bumble Bee", "Bambee", Some(197.0)),
            Some("UlwfDIf8e3Q".to_string())
        );
        // Without a known duration the first title match wins (legacy).
        assert_eq!(
            extract_matching_youtube_video_id(&html, "Bumble Bee", "Bambee", None),
            Some("jyFivH9gxz0".to_string())
        );
    }

    #[test]
    fn falls_back_to_title_match_when_no_length_fits() {
        let html = renderer_html("jyFivH9gxz0", "Bumble Bee", "Bambee", "3:03");
        assert_eq!(
            extract_matching_youtube_video_id(&html, "Bumble Bee", "Bambee", Some(197.0)),
            Some("jyFivH9gxz0".to_string())
        );
        assert_eq!(
            extract_matching_youtube_video_id(&html, "Other Song", "Bambee", Some(197.0)),
            None
        );
    }

    #[test]
    fn extracts_caption_tracks_and_prefers_manual_english() {
        let html = r#"{"captions":{"playerCaptionsTracklistRenderer":{"captionTracks":[{"baseUrl":"https://example.com/asr?x=1","name":{"simpleText":"English (auto)"},"languageCode":"en","kind":"asr"},{"baseUrl":"https://example.com/en?x=1","name":{"runs":[{"text":"English"}]},"languageCode":"en","kind":""},{"baseUrl":"https://example.com/de?x=1","name":{"simpleText":"German"},"languageCode":"de","kind":""}]}}}"#;
        let tracks = extract_caption_tracks(html).expect("caption tracks");
        assert_eq!(tracks.len(), 3);
        let (url, _name, lang, asr) =
            pick_caption_track(&tracks, Some("en")).expect("picked track");
        assert_eq!(url, "https://example.com/en?x=1");
        assert_eq!(lang, "en");
        assert!(!asr);
        // Unknown language falls back to the first manual track, not ASR.
        let (url, _, _, asr) = pick_caption_track(&tracks, Some("uk")).expect("fallback");
        assert_eq!(url, "https://example.com/en?x=1");
        assert!(!asr);
    }

    #[test]
    fn parses_json3_caption_events() {
        let body = r#"{"events":[{"tStartMs":1500,"dDurationMs":1200,"segs":[{"utf8":"Hello "},{"utf8":"world"}]},{"tStartMs":3000,"segs":[{"utf8":"   "}]},{"noSegs":true}]}"#;
        let lines = parse_json3_captions(body);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start_ms, 1500);
        assert_eq!(lines[0].dur_ms, 1200);
        assert_eq!(lines[0].text, "Hello world");
        assert!(parse_json3_captions("not json").is_empty());
    }

    #[test]
    fn ranks_duration_fits_first_then_page_order_deduped() {
        let html = format!(
            "{}---{}---{}",
            renderer_html("jyFivH9gxz0", "Bumble Bee", "Bambee", "3:03"),
            renderer_html("UlwfDIf8e3Q", "Bumble Bee", "Bambee", "3:17"),
            renderer_html("UlwfDIf8e3Q", "Bumble Bee", "Bambee", "3:17"),
        );
        let cands = collect_renderers(&html, "Bumble Bee", "Bambee");
        assert_eq!(cands.len(), 3);
        // Duration fit first, duplicates removed, limit honoured.
        assert_eq!(
            rank_ids(&cands, Some(197.0), 5),
            vec!["UlwfDIf8e3Q".to_string(), "jyFivH9gxz0".to_string()]
        );
        assert_eq!(
            rank_ids(&cands, Some(197.0), 1),
            vec!["UlwfDIf8e3Q".to_string()]
        );
        // No duration info: pure page order.
        assert_eq!(
            rank_ids(&cands, None, 5),
            vec!["jyFivH9gxz0".to_string(), "UlwfDIf8e3Q".to_string()]
        );
    }

    #[test]
    fn parses_youtubei_results_skipping_non_videos() {
        let json = serde_json::from_str::<serde_json::Value>(
            r#"{"contents":{"twoColumnSearchResultsRenderer":{"primaryContents":{"sectionListRenderer":{"contents":[{"itemSectionRenderer":{"contents":[{"videoRenderer":{"videoId":"UlwfDIf8e3Q","title":{"runs":[{"text":"Bambee - Bumble Bee"}]},"longBylineText":{"runs":[{"text":"Bambee"}]},"lengthText":{"simpleText":"3:17"}}},{"channelRenderer":{"channelId":"UCxxxx"}},{"videoRenderer":{"videoId":"jyFivH9gxz0","title":{"runs":[{"text":"Bumble Bee"}]},"shortBylineText":{"runs":[{"text":"Bambee"}]},"lengthText":{"runs":[{"text":"3:03"}]}}},{"videoRenderer":{"videoId":"short","title":{"runs":[{"text":"Bumble Bee"}]},"longBylineText":{"runs":[{"text":"Bambee"}]}}},{"videoRenderer":{"videoId":"AAAAAAAAAAA","title":{"runs":[{"text":"Unrelated Song"}]},"longBylineText":{"runs":[{"text":"Someone Else"}]},"lengthText":{"simpleText":"3:17"}}}]}}]}}}}}"#,
        )
        .expect("fixture");
        let cands = parse_youtubei_results(&json, "Bumble Bee", "Bambee");
        assert_eq!(
            cands,
            vec![
                ("UlwfDIf8e3Q".to_string(), Some(197.0)),
                ("jyFivH9gxz0".to_string(), Some(183.0)),
            ]
        );
        // Duration-aware ranking prefers the 3:17 upload for a 197s track.
        assert_eq!(
            rank_ids(&cands, Some(197.0), 5),
            vec!["UlwfDIf8e3Q".to_string(), "jyFivH9gxz0".to_string()]
        );
    }

    #[test]
    fn duration_fit_respects_tolerance() {
        let cands = vec![("UlwfDIf8e3Q".to_string(), Some(197.0))];
        assert!(duration_fit(&cands, "UlwfDIf8e3Q", Some(197.0)));
        assert!(!duration_fit(&cands, "UlwfDIf8e3Q", Some(300.0)));
        assert!(!duration_fit(&cands, "UlwfDIf8e3Q", None));
        assert!(!duration_fit(&cands, "missing-id", Some(197.0)));
        assert!(!duration_fit(
            &vec![("x".to_string(), None)],
            "x",
            Some(197.0)
        ));
    }

    #[test]
    fn transliterates_cyrillic_for_cross_script_matching() {
        assert_eq!(
            transliterate_cyrillic_to_latin("король и шут"),
            "korol i shut"
        );
        assert_eq!(
            transliterate_cyrillic_to_latin("дурак и молния"),
            "durak i molniya"
        );
        // Latin passes through untouched.
        assert_eq!(
            transliterate_cyrillic_to_latin("korol i shut"),
            "korol i shut"
        );
    }

    #[test]
    fn matches_transliterated_tags_against_native_titles() {
        let query_artist = normalize_search_text("Korol I Shut");
        let title_artist = normalize_search_text("Король и Шут");
        assert!(texts_match(&title_artist, &query_artist));
        assert!(texts_match(&query_artist, &title_artist));
        let query_song = normalize_search_text("Дурак и молния");
        let title_song = normalize_search_text("дурак и молния (official video)");
        assert!(texts_match(&title_song, &query_song));
        // Genuine mismatches still rejected.
        assert!(!texts_match(
            &normalize_search_text("Король и Шут"),
            &normalize_search_text("Bambee")
        ));
        assert!(!texts_match(
            &normalize_search_text("Лесник"),
            &normalize_search_text("Дурак и молния")
        ));
        // Empty query matches anything (legacy semantics).
        assert!(texts_match(&normalize_search_text("xxx"), ""));
    }

    #[test]
    fn parses_youtubei_cyrillic_results() {
        let json = serde_json::from_str::<serde_json::Value>(
            r#"{"contents":{"twoColumnSearchResultsRenderer":{"primaryContents":{"sectionListRenderer":{"contents":[{"itemSectionRenderer":{"contents":[{"videoRenderer":{"videoId":"AAAAAAAAAAA","title":{"runs":[{"text":"Король и Шут — Дурак и молния"}]},"longBylineText":{"runs":[{"text":"Король и Шут"}]},"lengthText":{"simpleText":"1:55"}}}]}}]}}}}}"#,
        )
        .expect("fixture");
        let cands = parse_youtubei_results(&json, "Дурак и молния", "Korol I Shut");
        assert_eq!(cands, vec![("AAAAAAAAAAA".to_string(), Some(115.0))]);
    }

    #[test]
    fn matches_words_modulo_endings() {
        assert!(words_match("молния", "молнией"));
        assert!(words_match("bumblebee", "bumble"));
        assert!(!words_match("лесник", "молния"));
        assert!(!words_match("ab", "cd"));
    }

    #[test]
    fn overlaps_reworded_titles() {
        assert!(words_overlap("дурак и молния концерт", "дурак и молния") >= 0.6);
        assert!(words_overlap("лесник", "дурак и молния") < 0.6);
        assert_eq!(words_overlap("anything", ""), 1.0);
    }

    #[test]
    fn builds_transliterated_query_variant() {
        assert_eq!(
            transliterated_query_variant("Дурак и молния Korol I Shut"),
            Some("durak i molniya korol i shut".to_string())
        );
        assert_eq!(transliterated_query_variant("Bumble Bee Bambee"), None);
    }

    #[test]
    fn client_snapshots_are_pinned() {
        // Proven snapshots (reference yt_music_api defaults). Dynamic/future
        // versions get "Requested entity was not found" 404s.
        assert_eq!(YT_REMIX_VERSION, "1.20250310.01.00");
        assert_eq!(YT_WEB_VERSION, "2.20250310.01.00");
        // Pinned Android release: older ones get HTTP 400, and this one
        // returns plain audio URLs where WEB_REMIX answers UNPLAYABLE.
        assert_eq!(YT_ANDROID_VERSION, "20.10.38");
        assert!(YT_UA.contains("Chrome/134.0.0.0"));
    }

    #[test]
    fn player_clients_try_remix_before_android() {
        assert_eq!(PLAYER_CLIENTS.len(), 2);
        assert_eq!(PLAYER_CLIENTS[0].name, "WEB_REMIX");
        assert_eq!(PLAYER_CLIENTS[0].host, "https://music.youtube.com");
        assert_eq!(PLAYER_CLIENTS[1].name, "ANDROID");
        assert_eq!(PLAYER_CLIENTS[1].host, "https://www.youtube.com");
        assert_eq!(PLAYER_CLIENTS[1].version, YT_ANDROID_VERSION);
    }

    #[test]
    fn sanitizes_filenames() {
        assert_eq!(sanitize_filename("Joost - Europapa"), "Joost - Europapa");
        assert_eq!(
            sanitize_filename("a/b\\c:d*e?f\"g<h>i|j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(sanitize_filename("   "), "track");
        assert_eq!(sanitize_filename("..."), "track");
        assert_eq!(audio_ext_for_mime("audio/mp4; codecs=\"mp4a.40.2\""), "m4a");
        assert_eq!(audio_ext_for_mime("audio/webm; codecs=opus"), "webm");
        assert_eq!(audio_ext_for_mime("audio/mpeg"), "mp3");
        assert_eq!(audio_ext_for_mime("audio/unknown"), "m4a");
    }

    #[test]
    fn parses_ytm_songs_with_details() {
        let json = serde_json::from_str::<serde_json::Value>(
            r#"{"contents":{"tabbedSearchResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[{"musicShelfRenderer":{"contents":[{"musicResponsiveListItemRenderer":{"flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Europapa"}]}}},{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Joost"},{"text":" • "},{"text":"Europapa"}]}}}],"fixedColumns":[{"musicResponsiveListItemFixedColumnRenderer":{"text":{"runs":[{"text":"2:40"}]}}}],"overlay":{"musicItemThumbnailOverlayRenderer":{"content":{"musicPlayButtonRenderer":{"playNavigationEndpoint":{"watchEndpoint":{"videoId":"4KrgAjLt95A"}}}}}}}},{"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"Europapa"}]},"subtitle":{"runs":[{"text":"Joost"}]},"navigationEndpoint":{"watchEndpoint":{"videoId":"T3EgX7A3ThE"}}}}]}}]}}}}]}}}"#,
        )
        .expect("fixture");
        let songs = parse_ytm_songs(&json, 8);
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].video_id, "4KrgAjLt95A");
        assert_eq!(songs[0].title, "Europapa");
        assert_eq!(songs[0].artist, "Joost");
        assert_eq!(songs[0].album, "Europapa");
        assert_eq!(songs[0].duration_secs, 160);
        assert_eq!(songs[1].video_id, "T3EgX7A3ThE");
        assert_eq!(songs[1].duration_secs, 0);
    }

    #[test]
    fn parses_ytm_card_and_shelf() {
        let json = serde_json::json!({
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [
                                        {
                                            "musicCardShelfRenderer": {
                                                "title": {
                                                    "runs": [{
                                                        "text": "Europapa",
                                                        "navigationEndpoint": {
                                                            "watchEndpoint": { "videoId": "4KrgAjLt95A" }
                                                        }
                                                    }]
                                                },
                                                "subtitle": {
                                                    "runs": [{ "text": "Song" }, { "text": " • " }, { "text": "Joost" }]
                                                }
                                            }
                                        },
                                        {
                                            "musicShelfRenderer": {
                                                "contents": [
                                                    {
                                                        "musicResponsiveListItemRenderer": {
                                                            "flexColumns": [
                                                                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Europapa" }] } } },
                                                                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Joost" }, { "text": " • " }, { "text": "Europapa" }] } } }
                                                            ],
                                                            "fixedColumns": [
                                                                { "musicResponsiveListItemFixedColumnRenderer": { "text": { "runs": [{ "text": "2:40" }] } } }
                                                            ],
                                                            "overlay": {
                                                                "musicItemThumbnailOverlayRenderer": {
                                                                    "content": {
                                                                        "musicPlayButtonRenderer": {
                                                                            "playNavigationEndpoint": {
                                                                                "watchEndpoint": { "videoId": "T3EgX7A3ThE" }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    },
                                                    {
                                                        "musicResponsiveListItemRenderer": {
                                                            "flexColumns": [
                                                                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Europapa (Sped Up)" }] } } },
                                                                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Joost" }] } } }
                                                            ],
                                                            "overlay": {
                                                                "musicItemThumbnailOverlayRenderer": {
                                                                    "content": {
                                                                        "musicPlayButtonRenderer": {
                                                                            "playNavigationEndpoint": {
                                                                                "watchEndpoint": { "videoId": "Sp3dUp00000" }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                ]
                                            }
                                        },
                                        { "badItem": { "nope": true } }
                                    ]
                                }
                            }
                        }
                    }]
                }
            }
        });
        let cands = parse_ytm_results(&json, "Europapa", "Joost");
        // Card (no duration) first, then shelf rows in order; junk skipped.
        assert_eq!(
            cands,
            vec![
                ("4KrgAjLt95A".to_string(), None),
                ("T3EgX7A3ThE".to_string(), Some(160.0)),
                ("Sp3dUp00000".to_string(), None),
            ]
        );
        // Duration-aware ranking prefers the 2:40 upload for a 160s track.
        assert_eq!(
            rank_ids(&cands, Some(160.0), 5),
            vec![
                "T3EgX7A3ThE".to_string(),
                "4KrgAjLt95A".to_string(),
                "Sp3dUp00000".to_string()
            ]
        );
    }

    #[test]
    fn parses_ytm_video_tiles_and_card_contents() {
        let json = serde_json::from_str::<serde_json::Value>(
            r#"{"contents":{"tabbedSearchResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[{"musicCardShelfRenderer":{"title":{"runs":[{"text":"Europapa"}]},"subtitle":{"runs":[{"text":"Song"},{"text":" • "},{"text":"Joost"}]},"contents":[{"musicResponsiveListItemRenderer":{"flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Europapa (Sped Up)"}]}}},{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Joost"}]}}}],"overlay":{"musicItemThumbnailOverlayRenderer":{"content":{"musicPlayButtonRenderer":{"playNavigationEndpoint":{"watchEndpoint":{"videoId":"Sp3dUp00000"}}}}}}}}]}},{"musicShelfRenderer":{"contents":[{"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"Europapa"}]},"subtitle":{"runs":[{"text":"Joost"},{"text":" • "},{"text":"1M views"}]},"navigationEndpoint":{"watchEndpoint":{"videoId":"T3EgX7A3ThE"}}}},{"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"Unrelated"}]},"subtitle":{"runs":[{"text":"Someone"}]},"navigationEndpoint":{"watchEndpoint":{"videoId":"NopeNope000"}}}},{"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"Europapa"}]},"subtitle":{"runs":[{"text":"Joost"}]}}}]}}]}}}}]}}}"#,
        )
        .expect("fixture");
        let cands = parse_ytm_results(&json, "Europapa", "Joost");
        // Card has no usable watch id here; card contents + video tile match.
        // The id-less tile and the unrelated tile are skipped.
        assert_eq!(
            cands,
            vec![
                ("Sp3dUp00000".to_string(), None),
                ("T3EgX7A3ThE".to_string(), None),
            ]
        );
    }

    #[test]
    fn prefers_decodable_audio_containers() {
        assert!(
            audio_container_rank("audio/mp4; codecs=\"mp4a.40.2\"")
                < audio_container_rank("audio/webm; codecs=opus")
        );
        assert!(
            audio_container_rank("audio/mpeg") < audio_container_rank("audio/webm; codecs=opus")
        );
    }

    fn player_json(formats: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "playabilityStatus": { "status": "OK" },
            "videoDetails": {
                "title": "Training Season",
                "author": "Dua Lipa",
                "lengthSeconds": "226",
            },
            "streamingData": formats,
        })
    }

    #[test]
    fn audio_pick_prefers_higher_bitrate_within_container() {
        let current = YtmAudio {
            url: "https://example.com/low".to_string(),
            mime: "audio/mp4".to_string(),
            bitrate: 48000,
            duration_secs: 200,
            title: "T".to_string(),
            artist: "A".to_string(),
        };
        assert!(is_better_audio_pick(
            "audio/mp4; codecs=\"mp4a.40.2\"",
            128000,
            &current
        ));
        assert!(!is_better_audio_pick(
            "audio/mp4; codecs=\"mp4a.40.2\"",
            48000,
            &current
        ));
        // Container rank still dominates: higher-bitrate Opus loses to mp4,
        // because Symphonia cannot decode Opus (see `is_better_audio_pick`).
        assert!(!is_better_audio_pick(
            "audio/webm; codecs=\"opus\"",
            160000,
            &current
        ));
    }

    #[test]
    fn pick_best_audio_prefers_decodable_container() {
        // Android-style response: opus at higher bitrate, mp4a lower.
        // The pick must stay mp4 for decode reliability.
        let json = player_json(serde_json::json!({
            "adaptiveFormats": [
                { "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 143465, "url": "https://example.com/opus" },
                { "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 128000, "url": "https://example.com/mp4" },
                { "mimeType": "video/mp4", "bitrate": 999999, "url": "https://example.com/video" },
            ],
        }));
        let (best, total, plain) = pick_best_audio(&json);
        let best = best.expect("audio pick");
        assert_eq!(best.mime, "audio/mp4");
        assert_eq!(best.bitrate, 128000);
        assert_eq!(best.duration_secs, 226);
        assert_eq!(best.title, "Training Season");
        assert_eq!(best.artist, "Dua Lipa");
        assert_eq!((total, plain), (3, 2));
    }

    #[test]
    fn pick_best_audio_ignores_cipher_only_responses() {
        // WEB_REMIX-style cipher-only response: nothing playable natively,
        // so the client fallback must keep going instead of stopping here.
        let json = player_json(serde_json::json!({
            "adaptiveFormats": [
                { "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 160000, "signatureCipher": "s=abc&url=https://example.com/x" },
            ],
        }));
        let (best, total, plain) = pick_best_audio(&json);
        assert!(best.is_none());
        assert_eq!((total, plain), (1, 0));
    }

    #[test]
    fn pick_best_audio_tolerates_missing_streaming_data() {
        let json = serde_json::json!({ "playabilityStatus": { "status": "OK" } });
        let (best, total, plain) = pick_best_audio(&json);
        assert!(best.is_none());
        assert_eq!((total, plain), (0, 0));
    }

    #[test]
    fn parses_content_range_total() {
        assert_eq!(
            parse_content_range_total("bytes 0-65535/1234567"),
            Some(1234567)
        );
        assert_eq!(parse_content_range_total("bytes 0-100/100"), Some(100));
        assert_eq!(parse_content_range_total("bytes */999"), None);
        assert_eq!(parse_content_range_total("garbage"), None);
    }

    #[test]
    fn only_refreshes_expired_audio_urls() {
        assert!(should_refresh_audio_url("audio HTTP 403 Forbidden"));
        assert!(!should_refresh_audio_url("audio HTTP 416"));
        assert!(!should_refresh_audio_url("audio download too small"));
    }

    /// Fake googlevideo endpoint: 403 without a bounded Range, 206 slices
    /// otherwise. Records every Range header it sees.
    fn spawn_range_server(
        content: Vec<u8>,
        fail_status: Option<u16>,
        max_requests: usize,
    ) -> (
        String,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        std::thread::JoinHandle<()>,
    ) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let url = format!(
            "http://127.0.0.1:{}/audio",
            listener.local_addr().expect("addr").port()
        );
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_clone = std::sync::Arc::clone(&seen);
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(max_requests) {
                let mut stream = match stream {
                    Ok(stream) => stream,
                    Err(_) => break,
                };
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            request.extend_from_slice(&buf[..n]);
                            if request.windows(4).any(|w| w == b"\r\n\r\n") || request.len() > 65536
                            {
                                break;
                            }
                        }
                    }
                }
                let head = String::from_utf8_lossy(&request);
                let range = head
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("Range:")
                            .or_else(|| line.strip_prefix("range:"))
                    })
                    .map(str::trim)
                    .unwrap_or("none")
                    .to_string();
                seen_clone.lock().expect("seen").push(range.clone());
                if let Some(status) = fail_status {
                    let body = format!(
                        "HTTP/1.1 {status} Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(body.as_bytes());
                    continue;
                }
                let (from, to) = range
                    .strip_prefix("bytes=")
                    .and_then(|spec| spec.split_once('-'))
                    .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))
                    .unwrap_or((0, 0));
                let from = from.min(content.len());
                let to = to.min(content.len().saturating_sub(1));
                let slice = if to >= from && !content.is_empty() {
                    &content[from..=to]
                } else {
                    &[][..]
                };
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{to}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    content.len(),
                    slice.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(slice);
            }
        });
        (url, seen, handle)
    }

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .expect("test client")
    }

    #[test]
    fn downloads_audio_in_ranged_chunks() {
        // 1.5MB forces 3 requests at 512KB chunks; the server 403s anything
        // without a bounded Range, like the real googlevideo endpoints.
        // Ranges after the probe fly in one parallel wave, so compare the
        // sorted set — arrival order is not deterministic.
        let content: Vec<u8> = (0..1_500_000u32).map(|i| (i % 251) as u8).collect();
        let (url, seen, handle) = spawn_range_server(content.clone(), None, 3);
        let downloaded = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(download_audio_bytes(&test_client(), &url))
            .expect("chunked download");
        handle.join().expect("server thread");
        assert_eq!(downloaded, content);
        let mut ranges = seen.lock().expect("seen").clone();
        ranges.sort();
        assert_eq!(
            ranges,
            vec![
                "bytes=0-524287".to_string(),
                "bytes=1048576-1499999".to_string(),
                "bytes=524288-1048575".to_string(),
            ]
        );
    }

    #[test]
    fn audio_download_rejects_forbidden() {
        let (url, seen, handle) = spawn_range_server(Vec::new(), Some(403), 1);
        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(download_audio_bytes(&test_client(), &url))
            .expect_err("403 must fail");
        handle.join().expect("server thread");
        assert!(error.contains("403"), "unexpected error: {error}");
        assert_eq!(seen.lock().expect("seen").len(), 1);
    }

    #[test]
    fn parses_browse_response_items() {
        let json = serde_json::json!({
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [
                                        {
                                            "musicCarouselShelfRenderer": {
                                                "header": {
                                                    "musicCarouselShelfBasicHeaderRenderer": {
                                                        "title": { "runs": [{ "text": "Trending Songs" }] }
                                                    }
                                                },
                                                "contents": [
                                                    {
                                                        "musicResponsiveListItemRenderer": {
                                                            "flexColumns": [
                                                                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Song A" }] } } },
                                                                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Artist A" }, { "text": " • " }, { "text": "Album A" }] } } }
                                                            ],
                                                            "fixedColumns": [
                                                                { "musicResponsiveListItemFixedColumnRenderer": { "text": { "runs": [{ "text": "3:30" }] } } }
                                                            ],
                                                            "thumbnail": {
                                                                "musicThumbnailRenderer": {
                                                                    "thumbnail": {
                                                                        "thumbnails": [{ "url": "https://example.com/thumb.jpg" }]
                                                                    }
                                                                }
                                                            },
                                                            "overlay": {
                                                                "musicItemThumbnailOverlayRenderer": {
                                                                    "content": {
                                                                        "musicPlayButtonRenderer": {
                                                                            "playNavigationEndpoint": {
                                                                                "watchEndpoint": { "videoId": "AbCdEfGhIjK" }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                ]
                                            }
                                        }
                                    ]
                                }
                            }
                        }
                    }]
                }
            }
        });
        let items = parse_ytm_browse_response(&json, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].video_id, "AbCdEfGhIjK");
        assert_eq!(items[0].title, "Song A");
        assert_eq!(items[0].artist, "Artist A");
        assert_eq!(items[0].album, "Album A");
        assert_eq!(items[0].duration_secs, 210);
        assert_eq!(items[0].thumbnail_url, "https://example.com/thumb.jpg");
        assert_eq!(items[0].category, "Trending Songs");
    }

    #[test]
    fn parses_next_radio_recommendations() {
        let json = serde_json::json!({
            "contents": {
                "singleColumnMusicWatchNextResultsRenderer": {
                    "tabbedRenderer": {
                        "watchNextTabbedResultsRenderer": {
                            "tabs": [{
                                "tabRenderer": {
                                    "content": {
                                        "musicQueueRenderer": {
                                            "content": {
                                                "playlistPanelRenderer": {
                                                    "contents": [
                                                        {
                                                            "playlistPanelVideoRenderer": {
                                                                "videoId": "XyZ12345678",
                                                                "title": { "runs": [{ "text": "Radio Song 1" }] },
                                                                "longBylineText": { "runs": [{ "text": "Radio Artist 1" }] },
                                                                "lengthText": { "runs": [{ "text": "2:50" }] },
                                                                "thumbnail": {
                                                                    "thumbnails": [{ "url": "https://example.com/radio.jpg" }]
                                                                }
                                                            }
                                                        }
                                                    ]
                                                }
                                            }
                                        }
                                    }
                                }
                            }]
                        }
                    }
                }
            }
        });
        let items = parse_ytm_next_response(&json, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].video_id, "XyZ12345678");
        assert_eq!(items[0].title, "Radio Song 1");
        assert_eq!(items[0].artist, "Radio Artist 1");
        assert_eq!(items[0].duration_secs, 170);
        assert_eq!(items[0].thumbnail_url, "https://example.com/radio.jpg");
    }

    /// A real YT Music row: square album-art crops *and* a landscape video
    /// still in the same `thumbnails` array. The two resolvers must hand back
    /// different bytes — that pairing is the whole point of the feature.
    #[test]
    fn splits_cover_and_clip_from_one_thumbnail_list() {
        let json = serde_json::json!({
            "thumbnail": {
                "musicThumbnailRenderer": {
                    "thumbnail": {
                        "thumbnails": [
                            { "url": "https://lh3.googleusercontent.com/a=w120-h120", "width": 120, "height": 120 },
                            { "url": "https://lh3.googleusercontent.com/a=w544-h544-l90-rj", "width": 544, "height": 544 },
                            { "url": "https://i.ytimg.com/vi/x/hq720.jpg", "width": 480, "height": 360 },
                            { "url": "https://i.ytimg.com/vi/x/maxresdefault.jpg", "width": 1280, "height": 720 }
                        ]
                    }
                }
            }
        });
        let cover = ytm_thumbnail(&json);
        assert_eq!(cover, "https://lh3.googleusercontent.com/a=s0");
        let clip = ytm_clip_thumbnail(&json);
        assert_eq!(clip, "https://i.ytimg.com/vi/x/maxresdefault.jpg");
        assert_ne!(cover, clip);
    }

    /// A pure-audio result carries only square crops: the clip half is empty
    /// so the UI knows to keep offering the cover instead of a black frame.
    #[test]
    fn clip_is_empty_when_only_square_crops_exist() {
        let json = serde_json::json!({
            "thumbnail": {
                "musicThumbnailRenderer": {
                    "thumbnail": {
                        "thumbnails": [
                            { "url": "https://lh3.googleusercontent.com/a=w544-h544-l90-rj", "width": 544, "height": 544 }
                        ]
                    }
                }
            }
        });
        assert_eq!(
            ytm_thumbnail(&json),
            "https://lh3.googleusercontent.com/a=s0"
        );
        assert_eq!(ytm_clip_thumbnail(&json), "");
    }

    /// Stripping a sizing suffix must still *request the original*. Returning
    /// the bare host-relative URL would silently downgrade every cover to
    /// googleusercontent's default size instead.
    #[test]
    fn upgrades_any_sizing_suffix_to_original_bytes() {
        assert_eq!(
            upgrade_cover_url("https://lh3.googleusercontent.com/a=w544-h544-l90-rj"),
            "https://lh3.googleusercontent.com/a=s0"
        );
        assert_eq!(
            upgrade_cover_url("https://lh3.googleusercontent.com/a=s0"),
            "https://lh3.googleusercontent.com/a=s0"
        );
        assert_eq!(
            upgrade_cover_url("https://lh3.googleusercontent.com/a"),
            "https://lh3.googleusercontent.com/a=s0"
        );
        // Non-Google hosts must pass through untouched.
        assert_eq!(
            upgrade_cover_url("https://i.ytimg.com/vi/x/maxresdefault.jpg"),
            "https://i.ytimg.com/vi/x/maxresdefault.jpg"
        );
    }

    /// The Songs and Videos shelves spell the same track differently, so
    /// matching has to fold case, punctuation and the usual video decorations.
    #[test]
    fn normalizes_titles_for_clip_pairing() {
        assert_eq!(ytm_match_key("Що Якщо"), "що якщо");
        assert_eq!(ytm_match_key("Hello, World!"), "hello world");
        // Punctuation is dropped without leaving a gap, so "Don't" and "Dont"
        // land on the same key.
        assert_eq!(ytm_match_key("Don't Stop"), "dont stop");
        assert_eq!(ytm_match_key("  spaced   out  "), "spaced out");
    }

    #[test]
    fn strips_video_decorations_from_titles() {
        assert_eq!(ytm_base_title("Wonderwall (Official Video)"), "wonderwall");
        assert_eq!(
            ytm_base_title("Wonderwall - Official Music Video [HD]"),
            "wonderwall"
        );
        assert_eq!(ytm_base_title("Що якщо (Official Video)"), "що якщо");
        assert_eq!(ytm_base_title("Wonderwall (Remastered)"), "wonderwall");
        // Must not eat a word that merely contains a noise word.
        assert_eq!(ytm_base_title("Videography"), "videography");
        assert_eq!(ytm_base_title("Auditorium"), "auditorium");
        // No decoration at all.
        assert_eq!(ytm_base_title("Wonderwall"), "wonderwall");
    }

    /// The core behaviour: an audio-only song gets the matching official video
    /// from the Videos shelf, and its clip still is re-pointed at that video.
    #[test]
    fn pairs_audio_only_song_with_matching_music_video() {
        let mut songs = vec![
            YtmSong {
                video_id: "Audio000001".to_string(),
                title: "Wonderwall".to_string(),
                artist: "Oasis".to_string(),
                album: "Morning Glory".to_string(),
                thumbnail_url: "cover".to_string(),
                clip_url: String::new(),
                kind: "song".to_string(),
                video_type: "MUSIC_VIDEO_TYPE_ATV".to_string(),
                has_video: false,
                clip_video_id: String::new(),
                duration_secs: 259,
            },
            YtmSong {
                video_id: "Music000001".to_string(),
                title: "Wonderwall (Official Video)".to_string(),
                artist: "Oasis".to_string(),
                album: String::new(),
                thumbnail_url: "v".to_string(),
                clip_url: String::new(),
                kind: "video".to_string(),
                video_type: "MUSIC_VIDEO_TYPE_OMV".to_string(),
                has_video: true,
                clip_video_id: String::new(),
                duration_secs: 0,
            },
        ];
        let paired = pair_song_clips(&mut songs);
        assert_eq!(paired, 1);
        assert_eq!(songs[0].clip_video_id, "Music000001");
        assert!(songs[0].has_video);
        assert_eq!(
            songs[0].clip_url,
            "https://i.ytimg.com/vi/Music000001/maxresdefault.jpg"
        );
        // A song that is already a video is never re-pointed.
        assert_eq!(songs[1].clip_video_id, "");
    }

    fn clip_test_song(id: &str, title: &str, kind: &str, has_video: bool) -> YtmSong {
        YtmSong {
            video_id: id.to_string(),
            title: title.to_string(),
            artist: "Oasis".to_string(),
            album: String::new(),
            thumbnail_url: String::new(),
            clip_url: String::new(),
            kind: kind.to_string(),
            video_type: String::new(),
            has_video,
            clip_video_id: String::new(),
            duration_secs: 0,
        }
    }

    /// The targeted follow-up search accepts the official video and rejects
    /// everything that would be a wrong clip.
    #[test]
    fn fallback_accepts_matching_video_only() {
        let song = clip_test_song("Audio000001", "Wonderwall", "song", false);
        // Exact official video.
        let good = clip_test_song(
            "Music000001",
            "Oasis - Wonderwall (Official Video)",
            "video",
            true,
        );
        assert!(ytm_clip_fallback_accept(&song, &good));
        // Lyric video still shares the bare title, so it is acceptable.
        let lyric = clip_test_song("Music000002", "Wonderwall (Lyrics)", "video", true);
        assert!(ytm_clip_fallback_accept(&song, &lyric));
        // A different song must never be attached.
        let other = clip_test_song(
            "Music000003",
            "Champagne Supernova (Official Video)",
            "video",
            true,
        );
        assert!(!ytm_clip_fallback_accept(&song, &other));
        // The song's own id is never a clip.
        let same_id = clip_test_song("Audio000001", "Wonderwall (Official Video)", "video", true);
        assert!(!ytm_clip_fallback_accept(&song, &same_id));
        // Non-video rows and unplayable rows are rejected.
        let album = clip_test_song("Music000004", "Wonderwall", "album", false);
        assert!(!ytm_clip_fallback_accept(&song, &album));
        let grey = clip_test_song("Music000005", "Wonderwall (Official Video)", "video", false);
        assert!(!ytm_clip_fallback_accept(&song, &grey));
        // Cyrillic titles match the same way.
        let ua_song = clip_test_song("Audio000002", "Що якщо", "song", false);
        let ua_video = clip_test_song(
            "Music000006",
            "Dо Sliz - Що якщо (Official Lyric Video)",
            "video",
            true,
        );
        assert!(ytm_clip_fallback_accept(&ua_song, &ua_video));
    }

    /// One-letter titles would match everything, so they never match.
    #[test]
    fn fallback_rejects_too_short_titles() {
        let song = clip_test_song("Audio000003", "Я", "song", false);
        let video = clip_test_song("Music000007", "Я (Official Video)", "video", true);
        assert!(!ytm_clip_fallback_accept(&song, &video));
    }

    /// Nothing to pair against means no clip, so the UI can hide the control.
    #[test]
    fn leaves_song_without_clip_when_no_video_matches() {
        let mut songs = vec![YtmSong {
            video_id: "Audio000002".to_string(),
            title: "Totally Different Song".to_string(),
            artist: "Nobody".to_string(),
            album: String::new(),
            thumbnail_url: "cover".to_string(),
            clip_url: String::new(),
            kind: "song".to_string(),
            video_type: "MUSIC_VIDEO_TYPE_ATV".to_string(),
            has_video: false,
            clip_video_id: String::new(),
            duration_secs: 100,
        }];
        let paired = pair_song_clips(&mut songs);
        assert_eq!(paired, 0);
        assert_eq!(songs[0].clip_video_id, "");
        assert!(!songs[0].has_video);
    }

    /// A song must never be paired with itself, even when the same id shows up
    /// as a video row.
    #[test]
    fn never_pairs_a_song_with_itself() {
        let mut songs = vec![YtmSong {
            video_id: "Same0000001".to_string(),
            title: "Loop".to_string(),
            artist: "A".to_string(),
            album: String::new(),
            thumbnail_url: String::new(),
            clip_url: String::new(),
            kind: "song".to_string(),
            video_type: String::new(),
            // Pretend YouTube called it a video, but the only same-title
            // candidate is this very row.
            has_video: true,
            clip_video_id: String::new(),
            duration_secs: 0,
        }];
        let paired = pair_song_clips(&mut songs);
        assert_eq!(paired, 0);
        assert_eq!(songs[0].clip_video_id, "");
    }

    /// The Songs shelf omits `videoType` entirely. Treating a missing value as
    /// "has a video" made every song claim a clip and stopped pairing from ever
    /// running — so a song with no `videoType` must be treated as audio.
    #[test]
    fn song_rows_without_videotype_are_not_videos() {
        // A song row: no videoType, and it must not be treated as a video.
        assert!(!ytm_row_is_video("song", ""));
        assert!(!ytm_row_is_video("song", "MUSIC_VIDEO_TYPE_ATV"));
        assert!(!ytm_row_is_video("album", ""));
        // A Videos-shelf row is a video.
        assert!(ytm_row_is_video("video", ""));
        assert!(ytm_row_is_video("video", "MUSIC_VIDEO_TYPE_OMV"));
        // ...unless InnerTube explicitly says it is audio-only.
        assert!(!ytm_row_is_video("video", "MUSIC_VIDEO_TYPE_ATV"));
    }

    /// A song row from the real Songs shelf (no `videoType` on the play button)
    /// must come out of the parser as audio, not as a video with a clip.
    #[test]
    fn parses_audio_song_row_as_having_no_video() {
        // test_search_row has no videoType, exactly like a real Songs row.
        let json = serde_json::json!({
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [{
                                        "musicShelfRenderer": {
                                            "title": { "runs": [{ "text": "Пісні" }] },
                                            "contents": [test_search_row("SongOnly001", "Wonderwall")]
                                        }
                                    }]
                                }
                            }
                        }
                    }]
                }
            }
        });
        let items = parse_ytm_songs(&json, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "song");
        assert!(!items[0].has_video, "song row must not claim a video");
        assert_eq!(items[0].video_type, "");
    }

    /// A browse continuation can nest whole carousels under
    /// `sectionListContinuation` rather than rows directly. Missing that shape
    /// silently dropped every continuation page.
    #[test]
    fn parses_rows_from_section_list_continuation() {
        let json = serde_json::json!({
            "continuationContents": {
                "sectionListContinuation": {
                    "contents": [
                        {
                            "musicCarouselShelfRenderer": {
                                "header": {
                                    "musicCarouselShelfBasicHeaderRenderer": {
                                        "title": { "runs": [{ "text": "Відкриті зараз" }] }
                                    }
                                },
                                "contents": [{
                                    "musicTwoRowItemRenderer": {
                                        "title": { "runs": [{ "text": "Пісня" }] },
                                        "subtitle": { "runs": [{ "text": "Виконавець" }] },
                                        "navigationEndpoint": {
                                            "watchEndpoint": { "videoId": "Cont0000001" }
                                        }
                                    }
                                }]
                            }
                        }
                    ]
                }
            }
        });
        let items = parse_ytm_browse_continuation(&json, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].video_id, "Cont0000001");
        assert_eq!(items[0].title, "Пісня");
        assert_eq!(items[0].category, "Відкриті зараз");
    }

    /// `videoType` is what separates a real music video from an audio-only
    /// upload, which is exactly the "does a clip exist" question.
    #[test]
    fn distinguishes_real_video_from_audio_only() {
        assert!(ytm_is_real_video("MUSIC_VIDEO_TYPE_OMV"));
        assert!(ytm_is_real_video("MUSIC_VIDEO_TYPE_UGC_BROWSED"));
        assert!(ytm_is_real_video(""));
        assert!(!ytm_is_real_video("MUSIC_VIDEO_TYPE_ATV"));
        assert!(!ytm_is_real_video("MUSIC_VIDEO_TYPE_PODCAST_EPISODE"));
    }

    /// browseId prefixes are stable across languages, so a row is still typed
    /// correctly when the shelf heading is in a language we don't list.
    #[test]
    fn detects_kind_from_browse_id_prefix() {
        assert_eq!(ytm_kind_from_browse_id("MPREb_9nqEki4ZDpp"), Some("album"));
        assert_eq!(
            ytm_kind_from_browse_id("VLPLK1PkWQlWtnNfovRdG"),
            Some("playlist")
        );
        assert_eq!(
            ytm_kind_from_browse_id("UCmMUZbaYdNH0bEd1PAlAqsA"),
            Some("artist")
        );
        assert_eq!(ytm_kind_from_browse_id("MPLA1234"), Some("artist"));
        assert_eq!(ytm_kind_from_browse_id(""), None);
    }

    /// A row InnerTube greys out cannot be played here, so it must not be
    /// offered as a result.
    #[test]
    fn drops_greyed_out_unplayable_rows() {
        let mut row = test_search_row("Grey0000001", "Unavailable");
        row["musicResponsiveListItemRenderer"]["musicItemRendererDisplayPolicy"] =
            serde_json::Value::String("MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT".to_string());
        let json = serde_json::json!({
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [{
                                        "musicShelfRenderer": {
                                            "title": { "runs": [{ "text": "Пісні" }] },
                                            "contents": [row]
                                        }
                                    }]
                                }
                            }
                        }
                    }]
                }
            }
        });
        assert!(parse_ytm_songs(&json, 10).is_empty());
    }

    /// The videoType read off a real play button decides `has_video`, which is
    /// what gates the clip control in the UI.
    #[test]
    fn marks_audio_only_songs_as_having_no_video() {
        let mut row = test_search_row("Audio000001", "Audio Only");
        row["musicResponsiveListItemRenderer"]["overlay"]["musicItemThumbnailOverlayRenderer"]
            ["content"]["musicPlayButtonRenderer"]["playNavigationEndpoint"]["watchEndpoint"]
            ["videoType"] = serde_json::Value::String("MUSIC_VIDEO_TYPE_ATV".to_string());
        let json = serde_json::json!({
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [{
                                        "musicShelfRenderer": {
                                            "title": { "runs": [{ "text": "Пісні" }] },
                                            "contents": [row]
                                        }
                                    }]
                                }
                            }
                        }
                    }]
                }
            }
        });
        let items = parse_ytm_songs(&json, 10);
        assert_eq!(items.len(), 1);
        assert!(!items[0].has_video);
        assert_eq!(items[0].video_type, "MUSIC_VIDEO_TYPE_ATV");
    }

    /// `music/get_search_suggestions` takes `input` and returns only query
    /// strings, with the typed prefix flagged so the UI can bold it.
    #[test]
    fn parses_search_suggestions_with_bold_prefix() {
        let json = serde_json::json!({
            "contents": [{
                "searchSuggestionsSectionRenderer": {
                    "contents": [
                        {
                            "searchSuggestionRenderer": {
                                "navigationEndpoint": {
                                    "searchEndpoint": { "query": "faded alan walker" }
                                },
                                "suggestion": {
                                    "runs": [
                                        { "text": "fade", "bold": true },
                                        { "text": "d alan walker" }
                                    ]
                                }
                            }
                        },
                        {
                            "historySuggestionRenderer": {
                                "navigationEndpoint": {
                                    "searchEndpoint": { "query": "faded" }
                                },
                                "suggestion": { "runs": [{ "text": "faded" }] },
                                "serviceEndpoint": {
                                    "feedbackEndpoint": { "feedbackToken": "TOKEN" }
                                }
                            }
                        }
                    ]
                }
            }]
        });
        let items = parse_ytm_suggestions(&json, 10);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "faded alan walker");
        assert!(!items[0].from_history);
        assert_eq!(items[0].runs.len(), 2);
        assert!(items[0].runs[0].bold);
        assert!(!items[0].runs[1].bold);
        assert!(items[1].from_history);
    }

    #[test]
    fn suggestions_tolerate_empty_and_malformed_responses() {
        assert!(parse_ytm_suggestions(&serde_json::json!({}), 10).is_empty());
        assert!(parse_ytm_suggestions(&serde_json::json!({ "contents": [{}] }), 10).is_empty());
        // An entry with no query text is skipped rather than rendered blank.
        let blank = serde_json::json!({
            "contents": [{
                "searchSuggestionsSectionRenderer": {
                    "contents": [{ "searchSuggestionRenderer": {} }]
                }
            }]
        });
        assert!(parse_ytm_suggestions(&blank, 10).is_empty());
    }

    #[test]
    fn classifies_localized_shelf_titles() {
        assert_eq!(ytm_shelf_kind("Songs"), "song");
        assert_eq!(ytm_shelf_kind("Пісні"), "song");
        assert_eq!(ytm_shelf_kind("Відео"), "video");
        assert_eq!(ytm_shelf_kind("Videos"), "video");
        assert_eq!(ytm_shelf_kind("Альбоми"), "album");
        assert_eq!(ytm_shelf_kind("Плейлисти"), "playlist");
        assert_eq!(ytm_shelf_kind("Виконавці"), "artist");
        assert_eq!(ytm_shelf_kind(""), "other");
    }

    /// A minimal `musicResponsiveListItemRenderer` for search tests.
    fn test_search_row(video_id: &str, title: &str) -> serde_json::Value {
        serde_json::json!({
            "musicResponsiveListItemRenderer": {
                "flexColumns": [
                    {
                        "musicResponsiveListItemFlexColumnRenderer": {
                            "text": { "runs": [{ "text": title }] }
                        }
                    },
                    {
                        "musicResponsiveListItemFlexColumnRenderer": {
                            "text": { "runs": [{ "text": "Artist" }] }
                        }
                    }
                ],
                "thumbnail": {
                    "musicThumbnailRenderer": {
                        "thumbnail": {
                            "thumbnails": [
                                {
                                    "url": "https://lh3.googleusercontent.com/a=w544-h544-l90-rj",
                                    "width": 544,
                                    "height": 544
                                }
                            ]
                        }
                    }
                },
                "overlay": {
                    "musicItemThumbnailOverlayRenderer": {
                        "content": {
                            "musicPlayButtonRenderer": {
                                "playNavigationEndpoint": {
                                    "watchEndpoint": { "videoId": video_id }
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    /// Search must walk every tab, not just index 0, and each row must be
    /// labelled with the kind of shelf it came from.
    #[test]
    fn search_labels_rows_per_shelf_across_tabs() {
        let song_row = test_search_row("Song0000001", "Song One");
        let clip_row = test_search_row("Clip0000001", "Clip One");
        let json = serde_json::json!({
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [
                                            {
                                                "musicShelfRenderer": {
                                                    "title": { "runs": [{ "text": "Пісні" }] },
                                                    "contents": [song_row]
                                                }
                                            }
                                        ]
                                    }
                                }
                            }
                        },
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [
                                            {
                                                "musicShelfRenderer": {
                                                    "title": { "runs": [{ "text": "Відео" }] },
                                                    "contents": [clip_row]
                                                }
                                            }
                                        ]
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        });
        let items = parse_ytm_songs(&json, 10);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Song One");
        assert_eq!(items[0].kind, "song");
        assert_eq!(items[1].title, "Clip One");
        assert_eq!(items[1].kind, "video");
    }

    /// Continuation pages arrive outside any tab/section wrapper.
    #[test]
    fn search_parses_continuation_pages() {
        let row = test_search_row("Next0000001", "Page Two");
        let json = serde_json::json!({
            "continuationContents": {
                "musicShelfContinuation": {
                    "header": {
                        "musicShelfHeaderBasicRenderer": {
                            "title": { "runs": [{ "text": "Пісні" }] }
                        }
                    },
                    "contents": [row],
                    "nextContinuationData": { "continuation": "TOKEN_2" }
                }
            }
        });
        let items = parse_ytm_songs(&json, 10);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Page Two");
        assert_eq!(items[0].kind, "song");
        assert_eq!(ytm_next_continuation(&json).as_deref(), Some("TOKEN_2"));
    }

    #[test]
    fn prefers_progressive_mp4_for_the_clip_preview() {
        let json = serde_json::json!({
            "streamingData": {
                "formats": [
                    { "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 128000, "url": "https://ex/aud" },
                    { "mimeType": "video/mp4; codecs=\"avc1\"", "bitrate": 500000, "url": "https://ex/small" },
                    { "mimeType": "video/mp4; codecs=\"avc1\"", "bitrate": 900000, "url": "https://ex/big" }
                ],
                "adaptiveFormats": [
                    { "mimeType": "video/mp4; codecs=\"avc1\"", "bitrate": 4000000, "url": "https://ex/adaptive" }
                ]
            }
        });
        let (url, mime, has_audio) = pick_best_video(&json).expect("video stream");
        assert_eq!(url, "https://ex/big");
        assert_eq!(mime, "video/mp4");
        // Progressive (muxed) pick: carries sound, so clip-audio mode is possible.
        assert!(has_audio);
    }

    /// With no muxed format, a video-only MP4 still plays muted in a `<video>`.
    #[test]
    fn falls_back_to_adaptive_video_only_stream() {
        let json = serde_json::json!({
            "streamingData": {
                "adaptiveFormats": [
                    { "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 128000, "url": "https://ex/aud" },
                    { "mimeType": "video/webm; codecs=\"vp9\"", "bitrate": 3000000, "url": "https://ex/webm" },
                    { "mimeType": "video/mp4; codecs=\"avc1\"", "bitrate": 2500000, "url": "https://ex/adaptive" }
                ]
            }
        });
        let (url, _, has_audio) = pick_best_video(&json).expect("video stream");
        assert_eq!(url, "https://ex/adaptive");
        // Adaptive video-only pick: no sound, so the song audio must stay.
        assert!(!has_audio);
    }

    #[test]
    fn no_video_stream_for_audio_only_player_response() {
        let json = serde_json::json!({
            "streamingData": {
                "adaptiveFormats": [
                    { "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "bitrate": 128000, "url": "https://ex/aud" }
                ]
            }
        });
        assert!(pick_best_video(&json).is_none());
    }

    // ── Live InnerTube probes ────────────────────────────────────────────
    // These hit the real API, so they are `#[ignore]`d: the normal suite stays
    // hermetic and fast. Run them explicitly to check real-world behaviour:
    //
    //     cargo test --lib -- --ignored --nocapture
    //
    // They assert only loose invariants and print the real payload, because
    // YouTube's shapes rotate and a strict assertion here would be noise.

    /// Does search actually find a music video for a song, and does the clip
    /// pairing wire it up? This is the check that matters for the clip feature.
    #[tokio::test]
    #[ignore = "live network test"]
    async fn live_search_finds_and_pairs_clips() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .expect("client");
        for query in ["oasis wonderwall", "daft punk around the world", "Що якщо"] {
            let pages = ytm_search_pages_json(&client, query).await;
            println!("\n=== {query:?}: {} pages ===", pages.len());
            let mut all: Vec<YtmSong> = Vec::new();
            for page in &pages {
                all.extend(parse_ytm_songs(page, 15));
            }
            let mut seen = std::collections::HashSet::new();
            all.retain(|song| seen.insert(song.video_id.clone()));
            println!("gathered {} unique rows", all.len());
            for song in &all {
                println!(
                    "  kind={:<8} video={:<6} has_video={:<5} clip={:<12} title={:?}",
                    song.kind,
                    song.video_id,
                    song.has_video,
                    if song.clip_video_id.is_empty() {
                        "-"
                    } else {
                        &song.clip_video_id
                    },
                    song.title
                );
            }
            let mut all = all;
            let paired = pair_song_clips(&mut all);
            println!("paired_clips={paired}");
            for song in &all {
                if !song.clip_video_id.is_empty() {
                    println!(
                        "  PAIRED {:?} -> {} ({})",
                        song.title, song.clip_video_id, song.kind
                    );
                }
            }
            // A well-known track must at least return results.
            assert!(!all.is_empty(), "no results for {query:?}");
        }
    }

    /// Sanity-check the typeahead endpoint end to end.
    #[tokio::test]
    #[ignore = "live network test"]
    async fn live_search_suggestions() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .expect("client");
        for input in ["fade", "oasis won", "щастя"] {
            let auth = youtube_auth(&client).await;
            let body = serde_json::json!({
                "context": innertube_context("WEB_REMIX", YT_REMIX_VERSION, auth.visitor.as_deref()),
                "input": input,
            });
            let json = ytm_innertube_post(
                &client,
                "music/get_search_suggestions",
                &body,
                &auth,
                YT_REMIX_VERSION,
                None,
            )
            .await;
            match json {
                Some(json) => {
                    let items = parse_ytm_suggestions(&json, 10);
                    println!("{input:?} -> {} suggestions", items.len());
                    for item in &items {
                        println!("    {:?} (history={})", item.text, item.from_history);
                    }
                }
                None => println!("{input:?} -> NO RESPONSE"),
            }
        }
    }

    /// `browse` currently reports zero items for home/charts in the dev log.
    /// Dump the real response shape so the parser can be pointed at it.
    #[tokio::test]
    #[ignore = "live network test"]
    async fn live_browse_shape() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .expect("client");
        let auth = youtube_auth(&client).await;
        for id in ["FEmusic_home", "FEmusic_charts"] {
            let body = serde_json::json!({
                "context": innertube_context("WEB_REMIX", YT_REMIX_VERSION, auth.visitor.as_deref()),
                "browseId": id,
            });
            let json =
                ytm_innertube_post(&client, "browse", &body, &auth, YT_REMIX_VERSION, None).await;
            let Some(json) = json else {
                println!("{id}: NO RESPONSE");
                continue;
            };
            println!("\n=== {id} ===");
            println!(
                "top-level keys: {:?}",
                json.as_object()
                    .map(|map| map.keys().cloned().collect::<Vec<_>>())
            );
            if let Some(contents) = json.get("contents").and_then(|c| c.as_object()) {
                println!("contents keys: {:?}", contents.keys().collect::<Vec<_>>());
                for (key, value) in contents {
                    if let Some(map) = value.as_object() {
                        println!(
                            "  contents.{key} keys: {:?}",
                            map.keys().collect::<Vec<_>>()
                        );
                    }
                }
            }
            if let Some(error) = json.pointer("/error/message") {
                println!("error: {error}");
            }
            // Precisely which renderer holds each section, and how many items
            // each candidate path yields.
            let sections = json
                .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
                .and_then(|c| c.as_array());
            match sections {
                None => println!("sections: NONE"),
                Some(sections) => {
                    println!("sections: {}", sections.len());
                    for (n, section) in sections.iter().enumerate() {
                        let keys: Vec<&str> = section
                            .as_object()
                            .map(|map| map.keys().map(String::as_str).collect())
                            .unwrap_or_default();
                        println!("  section[{n}] keys={keys:?}");
                        if let Some(carousel) = section.get("musicCarouselShelfRenderer") {
                            let items = carousel.get("contents").and_then(|c| c.as_array());
                            println!("    carousel items={}", items.map(|i| i.len()).unwrap_or(0));
                            for item in items.unwrap_or(&Vec::new()).iter().take(2) {
                                let item_keys: Vec<&str> = item
                                    .as_object()
                                    .map(|map| map.keys().map(String::as_str).collect())
                                    .unwrap_or_default();
                                println!("      item keys={item_keys:?}");
                                if let Some(tile) = item.get("musicTwoRowItemRenderer") {
                                    let title = ytm_text(
                                        tile.get("title").unwrap_or(&serde_json::Value::Null),
                                    );
                                    let id = ytm_watch_id(
                                        tile.get("navigationEndpoint")
                                            .unwrap_or(&serde_json::Value::Null),
                                    );
                                    println!(
                                        "        tile title={title:?} id={id:?} => {:?}",
                                        ytm_browse_item_from_tile(tile, "x").map(|i| i.title)
                                    );
                                }
                            }
                        }
                        if let Some(shelf) = section.get("musicShelfRenderer") {
                            println!(
                                "    shelf items={}",
                                shelf
                                    .get("contents")
                                    .and_then(|c| c.as_array())
                                    .map(|i| i.len())
                                    .unwrap_or(0)
                            );
                        }
                        // Inspect the real rows in a carousel: these are the
                        // chart entries currently being dropped.
                        if let Some(items) = section
                            .get("musicCarouselShelfRenderer")
                            .and_then(|c| c.get("contents"))
                            .and_then(|c| c.as_array())
                        {
                            let mut parsed = 0usize;
                            let mut by_reason: std::collections::BTreeMap<&str, usize> =
                                Default::default();
                            for item in items {
                                let Some(row) = item.get("musicResponsiveListItemRenderer") else {
                                    *by_reason.entry("not-a-row").or_default() += 1;
                                    continue;
                                };
                                if ytm_browse_item_from_row(row, "x").is_some() {
                                    parsed += 1;
                                } else if row.get("flexColumns").is_none() {
                                    *by_reason.entry("no-flexColumns").or_default() += 1;
                                } else if ytm_watch_id(row).is_some() {
                                    *by_reason.entry("no-id-from-row").or_default() += 1;
                                } else if row
                                    .get("navigationEndpoint")
                                    .and_then(|e| e.get("browseEndpoint"))
                                    .is_some()
                                {
                                    *by_reason.entry("browse-only(artist/album)").or_default() += 1;
                                } else {
                                    *by_reason.entry("other").or_default() += 1;
                                }
                            }
                            println!(
                                "    carousel rows={} parsed={} reasons={by_reason:?}",
                                items.len(),
                                parsed
                            );
                        }
                    }
                }
            }
            let items = parse_ytm_browse_response(&json, 30);
            println!("parsed items={}", items.len());
            for item in items.iter().take(5) {
                println!("  {:?} - {:?}", item.artist, item.title);
            }
        }
    }

    /// The real `ytm_browse` command, including the continuation fallback that
    /// `FEmusic_charts` needs.
    #[tokio::test]
    #[ignore = "live network test"]
    async fn live_browse_command_yields_items() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .expect("client");
        let auth = youtube_auth(&client).await;
        for id in ["FEmusic_home", "FEmusic_charts"] {
            let body = serde_json::json!({
                "context": innertube_context("WEB_REMIX", YT_REMIX_VERSION, auth.visitor.as_deref()),
                "browseId": id,
            });
            let json =
                ytm_innertube_post(&client, "browse", &body, &auth, YT_REMIX_VERSION, None).await;
            let Some(json) = json else {
                println!("{id}: NO RESPONSE");
                continue;
            };
            // Where do the tokens live, and what comes back?
            let tokens: Vec<String> = json
                .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/continuations")
                .and_then(|c| c.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|c| {
                            c.pointer("/nextContinuationData/continuation")
                                .and_then(|v| v.as_str())
                                .map(str::to_string)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let extra: Vec<String> = json
                .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
                .and_then(|c| c.as_array())
                .map(|sections| {
                    sections
                        .iter()
                        .filter_map(|s| {
                            s.pointer("/musicShelfRenderer/continuations/0/nextContinuationData/continuation")
                                .and_then(|v| v.as_str())
                                .map(str::to_string)
                        })
                        .collect()
                })
                .unwrap_or_default();
            println!("{id}: tokens={} shelf_tokens={}", tokens.len(), extra.len());
            for token in tokens.iter().chain(extra.iter()).take(2) {
                let cbody = serde_json::json!({
                    "context": innertube_context("WEB_REMIX", YT_REMIX_VERSION, auth.visitor.as_deref()),
                    "continuation": token,
                });
                let page = ytm_innertube_post(
                    &client,
                    "browse",
                    &cbody,
                    &auth,
                    YT_REMIX_VERSION,
                    Some(token),
                )
                .await;
                let Some(page) = page else {
                    println!("  continuation: NO RESPONSE");
                    continue;
                };
                let keys: Vec<&str> = page
                    .as_object()
                    .map(|m| m.keys().map(String::as_str).collect())
                    .unwrap_or_default();
                println!("  continuation top keys={keys:?}");
                if let Some(cc) = page.get("continuationContents").and_then(|c| c.as_object()) {
                    let names: Vec<&str> = cc.keys().map(String::as_str).collect();
                    println!("    continuationContents={names:?}");
                    for (name, value) in cc {
                        let n = value
                            .get("contents")
                            .and_then(|c| c.as_array())
                            .map(|c| c.len())
                            .unwrap_or(0);
                        let first: Vec<&str> = value
                            .get("contents")
                            .and_then(|c| c.as_array())
                            .and_then(|c| c.first())
                            .and_then(|c| c.as_object())
                            .map(|m| m.keys().map(String::as_str).collect())
                            .unwrap_or_default();
                        println!("      {name}: contents={n} first_keys={first:?}");
                    }
                }
                let parsed = parse_ytm_browse_continuation(&page, 30);
                println!("    parsed={} ", parsed.len());
            }
            match ytm_browse(Some(id.to_string()), Some(30)).await {
                Ok(items) => println!("{id}: command returned {} items", items.len()),
                Err(error) => println!("{id}: error {error}"),
            }
        }
    }

    /// Does the clip preview have a real video stream to play?
    #[tokio::test]
    #[ignore = "live network test"]
    async fn live_video_stream_for_clip() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .expect("client");
        // Wonderwall's official video vs. an audio-only upload.
        for id in ["Xq3soeUcMBc", "vU05Eksc_iM"] {
            match ytm_video_stream(&client, id).await {
                Ok(Some((url, mime, has_audio))) => println!(
                    "{id}: {mime} audio={has_audio} {}",
                    &url[..url.len().min(80)]
                ),
                Ok(None) => println!("{id}: no video stream"),
                Err(error) => println!("{id}: error {error}"),
            }
        }
    }

    /// The per-track command the player calls when the now-playing song has no
    /// clip: one targeted search, first acceptable hit.
    #[tokio::test]
    #[ignore = "live network test"]
    async fn live_find_clip_for_single_track() {
        for (title, artist) in [
            ("Wonderwall", "Oasis"),
            ("Що якщо", "Do Sliz"),
            ("Around the World", "Daft Punk"),
            ("", "Nobody"),
        ] {
            match ytm_find_clip(title.to_string(), artist.to_string()).await {
                Ok(Some(clip)) => println!(
                    "{title:?} / {artist:?} -> {} {}",
                    clip.video_id, clip.still_url
                ),
                Ok(None) => println!("{title:?} / {artist:?} -> none"),
                Err(error) => println!("{title:?} / {artist:?} -> error {error}"),
            }
        }
    }
}

/// Start the native audio service without a window or command bridge.
pub fn start_audio() -> CoreState {
    let (tx, rx) = std::sync::mpsc::channel();
    let status = Arc::new(Mutex::new(SharedStatus::default()));
    let level = Arc::new(Mutex::new(LevelMeter::default()));
    let (s, l) = (status.clone(), level.clone());
    std::thread::spawn(move || audio_thread(rx, s, l));
    Mutex::new(AudioHandle { tx, status, level })
}
