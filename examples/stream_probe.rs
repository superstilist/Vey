//! Opt-in media diagnostic using Vey's actual resolver and cache.
//! cargo run --locked --example stream_probe -- --cache VIDEO_ID [VIDEO_ID ...]
//! Never prints signed media URLs or credentials.
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), String> {
    use rodio::Source;
    let mut ids: Vec<String> = std::env::args().skip(1).collect();
    let cache = ids.iter().any(|arg| arg == "--cache");
    ids.retain(|arg| arg != "--cache");
    if ids.is_empty() {
        ids.push("kOn-HdEg6AQ".into());
    }
    for id in ids {
        if cache {
            let track = playervf::native::ytm_cache_stream(id.clone())
                .await?
                .ok_or("No audio source")?;
            let file = std::fs::File::open(&track.path).map_err(|e| e.to_string())?;
            let bytes = file.metadata().map_err(|e| e.to_string())?.len();
            let decoder = rodio::Decoder::try_from(file).map_err(|e| e.to_string())?;
            let channels = decoder.channels();
            let sample_rate = decoder.sample_rate();
            let samples = decoder.count();
            let seconds = samples as f64 / channels as f64 / sample_rate as f64;
            println!("{id}: cached={bytes} bytes, decoded={seconds:.2}s, expected={}s, channels={channels}, rate={sample_rate}", track.duration_secs);
        } else {
            let stream = playervf::native::ytm_stream_url(id.clone())
                .await?
                .ok_or("No audio source")?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| e.to_string())?;
            for range in [
                "bytes=0-524287",
                "bytes=524288-1048575",
                "bytes=1048576-1572863",
            ] {
                let response = client
                    .get(&stream.url)
                    .header("Range", range)
                    .send()
                    .await
                    .map_err(|e| e.without_url().to_string())?;
                let status = response.status();
                let content_range = response
                    .headers()
                    .get("content-range")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("none")
                    .to_owned();
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| e.without_url().to_string())?
                    .len();
                println!(
                    "{id} {range}: HTTP {status}, content-range={content_range}, bytes={bytes}"
                );
            }
        }
    }
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn main() {}
