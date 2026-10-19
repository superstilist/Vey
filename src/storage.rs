use crate::services::Store;
use std::path::PathBuf;
pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("PLAYERVF_DATA_DIR") {
        return p.into();
    }
    if let Some(p) = std::env::var_os("APPDATA") {
        return PathBuf::from(p).join("PlayerVF-Rust");
    }
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
    root.join("playervf-rust")
}
pub fn load() -> Result<Store, String> {
    let path = data_dir().join("state.json");
    if !path.exists() {
        return Ok(Store::default());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    match serde_json::from_slice::<Store>(&bytes) {
        Ok(store) if store.version == 1 => Ok(store),
        result => {
            let backup = path.with_file_name(format!(
                "state-unreadable-{}.json",
                crate::services::now_ms()
            ));
            std::fs::copy(&path, &backup)
                .map_err(|e| format!("Cannot preserve unreadable state: {e}"))?;
            Err(format!(
                "{}; backup: {}",
                result
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Unsupported state version".into()),
                backup.display()
            ))
        }
    }
}
pub fn save(store: &Store) -> Result<(), String> {
    save_at(store, &data_dir())
}
fn save_at(store: &Store, dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("state.pending.json");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, dir.join("state.json")).map_err(|e| e.to_string())
}

#[derive(Default)]
struct SaveQueue {
    pending: Option<Store>,
    closing: bool,
}
/// A single disk writer, with at most one pending snapshot. Drop flushes it.
pub struct SaveWorker {
    queue: std::sync::Arc<(std::sync::Mutex<SaveQueue>, std::sync::Condvar)>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl SaveWorker {
    pub fn new(on_error: impl Fn(String) + Send + 'static) -> Self {
        Self::at_dir(data_dir(), on_error)
    }
    fn at_dir(dir: PathBuf, on_error: impl Fn(String) + Send + 'static) -> Self {
        let queue = std::sync::Arc::new((
            std::sync::Mutex::new(SaveQueue::default()),
            std::sync::Condvar::new(),
        ));
        let shared = queue.clone();
        let thread = std::thread::spawn(move || loop {
            let (lock, wake) = &*shared;
            let mut state = lock.lock().unwrap();
            while state.pending.is_none() && !state.closing {
                state = wake.wait(state).unwrap();
            }
            let Some(store) = state.pending.take() else {
                break;
            };
            drop(state);
            if let Err(e) = save_at(&store, &dir) {
                on_error(format!("Save failed: {e}"));
            }
        });
        Self {
            queue,
            thread: Some(thread),
        }
    }
    pub fn request(&self, store: Store) {
        let (lock, wake) = &*self.queue;
        lock.lock().unwrap().pending = Some(store);
        wake.notify_one();
    }
}
impl Drop for SaveWorker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        lock.lock().unwrap().closing = true;
        wake.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "vey-save-{}-{}",
            std::process::id(),
            crate::services::now_ms()
        ))
    }
    #[test]
    fn background_writer_flushes_latest_snapshot_on_shutdown() {
        let dir = temp_dir();
        let (tx, rx) = std::sync::mpsc::channel();
        let writer = SaveWorker::at_dir(dir.clone(), move |e| {
            let _ = tx.send(e);
        });
        for i in 0..100 {
            let mut store = Store::default();
            store.settings.volume = i as f32 / 100.;
            writer.request(store);
        }
        drop(writer);
        assert!(rx.try_iter().next().is_none());
        let saved: Store =
            serde_json::from_slice(&std::fs::read(dir.join("state.json")).unwrap()).unwrap();
        assert_eq!(saved.settings.volume, 0.99);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn background_writer_reports_disk_failure() {
        let dir = temp_dir().with_extension("file");
        std::fs::write(&dir, b"not a directory").unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let writer = SaveWorker::at_dir(dir.clone(), move |e| {
            let _ = tx.send(e);
        });
        writer.request(Store::default());
        drop(writer);
        assert!(rx.try_recv().unwrap().contains("Save failed"));
        std::fs::remove_file(dir).unwrap();
    }
}
