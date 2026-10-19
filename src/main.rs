#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), slint::PlatformError> {
    if std::env::args().any(|a| a == "--lyrics-challenge") {
        return playervf::run_challenge().map_err(Into::into);
    }
    if std::env::args().any(|a| a == "--web-server") {
        return playervf::serve_web().map_err(Into::into);
    }
    if std::env::args().any(|a| a == "--lyrics-auth-check") {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        runtime.block_on(playervf::lyrics_service::prewarm_authentication())?;
        println!("Lyrics JWT cached and valid");
        return Ok(());
    }
    playervf::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
