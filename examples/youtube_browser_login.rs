//! Opt-in visible Google sign-in launch check. Use an isolated PLAYERVF_DATA_DIR.
//! The user alone enters authentication data; this example never prints identity.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("PLAYERVF_DATA_DIR").is_none() {
        return Err("Set PLAYERVF_DATA_DIR to an isolated test directory".into());
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let cancel = std::sync::atomic::AtomicBool::new(false);
    match runtime.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(90),
            playervf::youtube_browser::connect(&cancel),
        )
        .await
    }) {
        Ok(Ok(_)) => println!("Browser account verified; no identity or session data printed."),
        Ok(Err(error)) => return Err(error.into()),
        Err(_) => println!("Browser login remains pending. No account marked connected."),
    }
    Ok(())
}
