//! CPU blur work stays off the Slint event loop. Only raw RGBA crosses threads.
use std::sync::mpsc::{self, Receiver, SyncSender};
pub struct Job {
    pub index: usize,
    pub key: u16,
    pub generation: u64,
    pub text: String,
    pub family: String,
    pub size: f32,
    pub weight: u16,
    pub radius: f32,
}
pub struct Ready {
    pub index: usize,
    pub key: u16,
    pub generation: u64,
    pub size: f32,
    pub padding: f32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
pub struct MaskWorker {
    jobs: SyncSender<Job>,
    ready: Receiver<Ready>,
}
impl MaskWorker {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<Job>(1);
        let (done, ready) = mpsc::channel();
        std::thread::Builder::new()
            .name("vey-lyric-focus".into())
            .spawn(move || {
                let mut cache = crate::native_lyric_fonts::cache();
                let mut font_revision = 0;
                while let Ok(job) = rx.recv() {
                    crate::native_lyric_fonts::refresh_cache(&mut cache, &mut font_revision);
                    let mask =
                        cache.blurred_run(&job.text, &job.family, job.weight, job.radius, job.size);
                    let image = mask.wide.to_rgba8();
                    let (width, height, pixels) = image.map_or((0, 0, vec![]), |p| {
                        (p.width(), p.height(), p.as_bytes().to_vec())
                    });
                    if done
                        .send(Ready {
                            index: job.index,
                            key: job.key,
                            generation: job.generation,
                            size: job.size,
                            padding: mask.padding,
                            width,
                            height,
                            pixels,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("native lyric mask worker");
        Self { jobs: tx, ready }
    }
    pub fn submit(&self, job: Job) -> bool {
        self.jobs.try_send(job).is_ok()
    }
    pub fn take(&self) -> Option<Ready> {
        self.ready.try_recv().ok()
    }
}
