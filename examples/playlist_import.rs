//! Opt-in public playlist probe; prints counts only, never account data/tokens.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let link = std::env::args()
        .nth(1)
        .ok_or("Usage: playlist_import YOUTUBE_MUSIC_PLAYLIST_URL")?;
    let runtime = tokio::runtime::Runtime::new()?;
    let playlist = runtime.block_on(playervf::native::ytm_import_playlist(link))?;
    println!(
        "Imported {} tracks; unique references: {}",
        playlist.tracks.len(),
        playlist
            .tracks
            .iter()
            .map(|t| &t.path)
            .collect::<std::collections::HashSet<_>>()
            .len()
    );
    Ok(())
}
