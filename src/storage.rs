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
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("state.pending.json");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, dir.join("state.json")).map_err(|e| e.to_string())
}
