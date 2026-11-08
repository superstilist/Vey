//! Whole-song shaping runs on one bounded worker. Only owned data crosses threads.
use super::*;
use std::sync::mpsc::{self, Receiver, SyncSender};

struct Job {
    generation: u64,
    lyrics: Vec<Lyric>,
    backgrounds: Vec<(bool, String)>,
    width: f32,
    size: f32,
    gap: f32,
    stage: bool,
    timed: bool,
    family: String,
    spacing: f32,
    weight: i32,
}
struct Ready {
    generation: u64,
    rows: Vec<Row>,
    offsets: Vec<f32>,
}
#[derive(Default)]
pub(super) struct Worker {
    generation: u64,
    channels: Option<(SyncSender<Job>, Receiver<Ready>)>,
    queued: Option<Job>,
    running: bool,
}
impl Worker {
    pub(super) fn request(
        &mut self,
        source: &ModelRc<LyricRow>,
        lyrics: Vec<Lyric>,
        width: f32,
        size: f32,
        gap: f32,
        stage: bool,
        timed: bool,
        style: &crate::LyricStyle,
        weight: i32,
    ) {
        self.generation = self.generation.wrapping_add(1);
        self.queued = Some(Job {
            generation: self.generation,
            lyrics,
            backgrounds: (0..source.row_count())
                .map(|i| {
                    let row = source.row_data(i).unwrap();
                    (row.words.row_count() > 0, row.background_text.to_string())
                })
                .collect(),
            width,
            size,
            gap,
            stage,
            timed,
            family: style.family.to_string(),
            spacing: style.line_spacing,
            weight,
        });
    }
    pub(super) fn poll(&mut self) -> Option<(Vec<BraccatoRow>, Vec<f32>)> {
        if self.channels.is_none() {
            let (send, jobs) = mpsc::sync_channel::<Job>(1);
            let (done, receive) = mpsc::sync_channel(1);
            std::thread::Builder::new()
                .name("vey-lyric-layout".into())
                .spawn(move || {
                    while let Ok(job) = jobs.recv() {
                        let source = model(
                            job.backgrounds
                                .into_iter()
                                .map(|(words, background)| LyricRow {
                                    background_text: background.into(),
                                    words: model(if words {
                                        vec![crate::WordRow::default()]
                                    } else {
                                        vec![]
                                    }),
                                    ..Default::default()
                                })
                                .collect(),
                        );
                        let style = crate::LyricStyle {
                            family: job.family.into(),
                            line_spacing: job.spacing,
                            ..Default::default()
                        };
                        let (rows, offsets) = styled_layout(
                            &source,
                            &job.lyrics,
                            job.width,
                            job.size,
                            job.gap,
                            job.stage,
                            &style,
                            job.weight,
                            job.timed,
                        );
                        if done
                            .send(Ready {
                                generation: job.generation,
                                rows: rows.into_iter().map(Row::capture).collect(),
                                offsets,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .expect("native lyric layout worker");
            self.channels = Some((send, receive));
        }
        let (send, receive) = self.channels.as_ref().unwrap();
        let ready = receive.try_recv().ok();
        if ready.is_some() {
            self.running = false;
        }
        if !self.running {
            if let Some(job) = self.queued.take() {
                self.running = send.try_send(job).is_ok();
            }
        }
        ready
            .filter(|r| r.generation == self.generation)
            .map(|r| (r.rows.into_iter().map(Row::restore).collect(), r.offsets))
    }
}

#[derive(Debug, PartialEq)]
struct Word {
    text: String,
    font_family: String,
    active: bool,
    background: bool,
    line_synced: bool,
    start_ms: f32,
    duration_ms: f32,
    cue_offset: f32,
    cue_span: f32,
    motion_start_ms: f32,
    motion_duration_ms: f32,
    motion_origin_em: f32,
    source_part: i32,
}
impl Word {
    fn capture(row: BraccatoWord) -> Self {
        Self {
            text: row.text.to_string(),
            font_family: row.font_family.to_string(),
            active: row.active,
            background: row.background,
            line_synced: row.line_synced,
            start_ms: row.start_ms,
            duration_ms: row.duration_ms,
            cue_offset: row.cue_offset,
            cue_span: row.cue_span,
            motion_start_ms: row.motion_start_ms,
            motion_duration_ms: row.motion_duration_ms,
            motion_origin_em: row.motion_origin_em,
            source_part: row.source_part,
        }
    }
    fn restore(self) -> BraccatoWord {
        BraccatoWord {
            text: self.text.into(),
            font_family: self.font_family.into(),
            active: self.active,
            background: self.background,
            line_synced: self.line_synced,
            start_ms: self.start_ms,
            duration_ms: self.duration_ms,
            cue_offset: self.cue_offset,
            cue_span: self.cue_span,
            motion_start_ms: self.motion_start_ms,
            motion_duration_ms: self.motion_duration_ms,
            motion_origin_em: self.motion_origin_em,
            source_part: self.source_part,

            ..Default::default()
        }
    }
}
#[derive(Debug, PartialEq)]
struct Row {
    text: String,
    agent: String,
    font_family: String,
    stage_role: String,
    secondary: bool,
    translation: bool,
    romanization: bool,
    instrumental: bool,
    source_index: i32,
    start_ms: f32,
    end_ms: f32,
    parts: Vec<Word>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latest_layout_preserves_cues_and_matches_synchronous_shaping() {
        let source = model(vec![LyricRow {
            text: "Слова не повинні зависати".into(),
            start_ms: 1000,
            end_ms: 4000,
            words: model(vec![
                crate::WordRow {
                    text: "Слова ".into(),
                    start_ms: 1000,
                    end_ms: 2000,
                    explicit: true,
                    ..Default::default()
                },
                crate::WordRow {
                    text: "не повинні зависати".into(),
                    start_ms: 2000,
                    end_ms: 4000,
                    explicit: true,
                    ..Default::default()
                },
            ]),
            background_text: "Backing".into(),
            ..Default::default()
        }]);
        let lyrics = document(&source, false);
        let style = crate::themes::NativeTheme::default().style();
        let mut worker = Worker::default();
        worker.request(
            &source,
            lyrics.clone(),
            180.,
            48.,
            0.65,
            false,
            true,
            &style,
            700,
        );
        assert!(worker.poll().is_none());
        worker.request(
            &source,
            lyrics.clone(),
            240.,
            48.,
            0.65,
            false,
            true,
            &style,
            700,
        );
        worker.request(
            &source,
            lyrics.clone(),
            680.,
            48.,
            0.65,
            false,
            true,
            &style,
            700,
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let (actual, offsets) = loop {
            if let Some(result) = worker.poll() {
                break result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "layout worker timeout"
            );
            std::thread::sleep(Duration::from_millis(2));
        };
        let (expected, expected_offsets) =
            styled_layout(&source, &lyrics, 680., 48., 0.65, false, &style, 700, true);
        assert_eq!(
            actual.into_iter().map(Row::capture).collect::<Vec<_>>(),
            expected.into_iter().map(Row::capture).collect::<Vec<_>>()
        );
        assert_eq!(offsets, expected_offsets);
        assert!(
            worker.poll().is_none(),
            "obsolete layouts cannot be applied later"
        );
    }
}
impl Row {
    fn capture(row: BraccatoRow) -> Self {
        Self {
            text: row.text.to_string(),
            agent: row.agent.to_string(),
            font_family: row.font_family.to_string(),
            stage_role: row.stage_role.to_string(),
            secondary: row.secondary,
            translation: row.translation,
            romanization: row.romanization,
            instrumental: row.instrumental,
            source_index: row.source_index,
            start_ms: row.start_ms,
            end_ms: row.end_ms,
            parts: (0..row.parts.row_count())
                .filter_map(|i| row.parts.row_data(i))
                .map(Word::capture)
                .collect(),
        }
    }
    fn restore(self) -> BraccatoRow {
        BraccatoRow {
            text: self.text.into(),
            agent: self.agent.into(),
            font_family: self.font_family.into(),
            stage_role: self.stage_role.into(),
            secondary: self.secondary,
            translation: self.translation,
            romanization: self.romanization,
            instrumental: self.instrumental,
            source_index: self.source_index,
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            parts: model(self.parts.into_iter().map(Word::restore).collect()),
            ..Default::default()
        }
    }
}
