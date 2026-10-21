//! Example for explicitly caller-provided session credentials.
//! Run only when you are authorized to use that account/session.

use yt_rust::{
    models::MusicSearchFilter,
    provider::YouTubeMusicProvider as YouTubeMusicApi,
    providers::youtube_music::{MusicSession, YouTubeMusicConfig, YouTubeMusicProvider},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("YOUTUBE_MUSIC_WEB_API_KEY")?;
    let cookie_header = std::env::var("YOUTUBE_MUSIC_COOKIE")?;
    let session = MusicSession::new(cookie_header)?;
    let config = YouTubeMusicConfig::with_locale(api_key, "US", "en")?.with_session(session);
    let provider = YouTubeMusicProvider::new(config)?;
    let page =
        YouTubeMusicApi::music_search(&provider, "song", MusicSearchFilter::Songs, None).await?;
    println!("Found {} songs", page.songs.len());
    Ok(())
}
