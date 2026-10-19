#[cfg(not(target_os = "android"))]
pub use rfd::FileDialog;
#[cfg(target_os = "android")]
pub struct FileDialog;
#[cfg(target_os = "android")]
impl FileDialog {
    pub fn new() -> Self {
        Self
    }
    pub fn add_filter(self, _: &str, _: &[&str]) -> Self {
        self
    }
    pub fn set_file_name(self, _: &str) -> Self {
        self
    }
    pub fn pick_file(self) -> Option<std::path::PathBuf> {
        None
    }
    pub fn pick_files(self) -> Option<Vec<std::path::PathBuf>> {
        None
    }
    pub fn pick_folder(self) -> Option<std::path::PathBuf> {
        None
    }
    pub fn save_file(self) -> Option<std::path::PathBuf> {
        None
    }
}
