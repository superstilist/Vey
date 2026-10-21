//! Run with: cargo run --features youtube-music --example youtube_music_search

use yt_rust::{
    models::MusicSearchFilter,
    provider::YouTubeMusicProvider as YouTubeMusicApi,
    providers::youtube_music::{YouTubeMusicConfig, YouTubeMusicProvider},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("YOUTUBE_MUSIC_WEB_API_KEY")?;
    let provider =
        YouTubeMusicProvider::new(YouTubeMusicConfig::with_locale(api_key, "US", "en")?)?;
    let page =
        YouTubeMusicApi::music_search(&provider, "artist song", MusicSearchFilter::Songs, None)
            .await?;

    for song in page.songs {
        let artist = song
            .artists
            .first()
            .map_or("unknown", |item| item.name.as_str());
        let video_id = song.video_id.as_deref().unwrap_or("no video id");
        println!("{artist} — {} [{video_id}]", song.title);
        if let Some(cover) = song.thumbnails.first() {
            println!("  artwork: {}", cover.url);
        }
    }
    if let Some(next) = page.continuation {
        println!("Continuation is available ({})", next.len());
    }
    Ok(())
}
