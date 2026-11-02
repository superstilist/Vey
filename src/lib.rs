mod account;
pub mod library_import;
pub mod home;
#[cfg(not(target_arch = "wasm32"))]
mod music_accounts;
#[cfg(not(target_arch = "wasm32"))]
pub mod youtube_browser;
pub mod search_index;
#[cfg(any(test, not(target_arch = "wasm32")))]
mod ambient_motion;
#[cfg(not(target_arch = "wasm32"))]
pub mod ambient_native;
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
pub mod lyric_appearance;
pub mod lyrics;
#[cfg(not(target_arch = "wasm32"))]
pub mod lyrics_service;
pub mod lyrics_translation;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod media_session;
#[cfg(any(test, target_arch = "wasm32"))]
mod lyrics_widget;
mod material;
#[cfg(not(target_arch = "wasm32"))]
mod media_library;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
mod native_lyric_camera;
#[cfg(not(target_arch = "wasm32"))]
mod native_lyric_focus;
#[cfg(not(target_arch = "wasm32"))]
mod native_lyric_follow;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_lyric_fonts;
#[cfg(not(target_arch = "wasm32"))]
mod native_lyric_glyphs;
#[cfg(not(target_arch = "wasm32"))]
mod native_lyric_masks;
#[cfg(not(target_arch = "wasm32"))]
mod native_lyric_paint;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_lyrics;
pub mod navigation_fx;
pub mod player_themes;
mod preferences;
#[cfg(target_arch = "wasm32")]
mod renderer;
pub mod services;
#[cfg(not(target_arch = "wasm32"))]
mod snapshots;
#[cfg(not(target_arch = "wasm32"))]
pub mod storage;
mod theme_colors;
pub mod theme_presets;
pub mod themes;
pub mod track_model;
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
