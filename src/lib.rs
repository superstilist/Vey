#[cfg(all(not(target_arch = "wasm32"), not(target_os = "windows")))]
mod ambient_native;
#[cfg(not(target_arch = "wasm32"))]
mod app;
mod artwork;
pub mod audio;
pub mod braccato;
#[cfg(not(target_arch = "wasm32"))]
mod challenge;
pub mod i18n;
#[cfg(not(target_arch = "wasm32"))]
pub mod local_sync;
pub mod lyrics;
#[cfg(not(target_arch = "wasm32"))]
pub mod lyrics_service;
pub mod lyrics_translation;
mod lyrics_widget;
mod material;
#[cfg(not(target_arch = "wasm32"))]
mod media_library;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
mod preferences;
mod renderer;
pub mod services;
#[cfg(not(target_arch = "wasm32"))]
mod snapshots;
#[cfg(not(target_arch = "wasm32"))]
pub mod storage;
pub mod themes;
pub mod video;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows_webview;
slint::include_modules!();
#[cfg(not(target_arch = "wasm32"))]
pub fn run() -> Result<(), slint::PlatformError> {
    app::run()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_challenge() -> Result<(), String> {
    challenge::run_helper()
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod server;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn serve_web() -> Result<(), String> {
    server::run()
}

#[cfg(target_os = "android")]
mod android;
#[cfg(not(target_arch = "wasm32"))]
mod dialogs;
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: slint::android::AndroidApp) {
    if let Some(path) = app.internal_data_path() {
        std::env::set_var("PLAYERVF_DATA_DIR", path.join("playervf"));
    }
    android::set_app(Some(app.clone()));
    slint::android::init(app).expect("Android Slint backend");
    let _ = run();
    android::set_app(None);
}
