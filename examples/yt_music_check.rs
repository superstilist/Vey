//! Opt-in public catalog and optional media check, using Vey's actual service path.
//! cargo run --locked --example yt_music_check -- "oasis wonderwall" [--audio]
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let audio = args.iter().any(|arg| arg == "--audio");
    let query = args
        .into_iter()
        .filter(|arg| arg != "--audio")
        .collect::<Vec<_>>()
        .join(" ");
    let query = if query.is_empty() {
        "oasis wonderwall".into()
    } else {
        query
    };
    let rows = playervf::native::ytm_search_songs(query, Some(8)).await?;
    let playable: Vec<_> = rows
        .iter()
        .filter(|row| ["song", "video", "top"].contains(&row.kind.as_str()))
        .collect();
    if playable.is_empty() {
        return Err("No playable catalog rows returned".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "parser": "vendored yt-rust + Vey renderer adapters",
            "tracks": playable.iter().map(|row| serde_json::json!({
                "videoId": row.video_id,
                "title": row.title,
                "artist": row.artist,
                "album": row.album,
                "durationSecs": row.duration_secs,
                "hasArtwork": !row.thumbnail_url.is_empty(),
                "hasPairedClip": !row.clip_video_id.is_empty()
            })).collect::<Vec<_>>()
        }))
        .map_err(|error| error.to_string())?
    );
    if audio {
        let track = playervf::native::ytm_cache_stream(playable[0].video_id.clone())
            .await?
            .ok_or("The selected track has no available audio stream")?;
        let bytes = std::fs::metadata(&track.path)
            .map_err(|error| error.to_string())?
            .len();
        if bytes == 0 {
            return Err("Downloaded audio is empty".into());
        }
        println!("Audio cached: {bytes} bytes; format={}", track.format);
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
