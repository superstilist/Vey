# yt-rust

An async Rust library for YouTube video discovery and opt-in YouTube Music catalog metadata. It normalizes supported search results, suggestions, chart tracks where returned, artwork, and player handoff. Some declared Music detail capabilities remain explicitly unsupported. It is a library only; there is no HTTP server.

## Important boundaries

- The default `youtube-data` feature uses documented YouTube Data API v3 endpoints. Its `mostPopular` chart is a general video chart, not a YouTube Music chart.
- The optional `youtube-music` feature uses the YouTube Music web interface, which is undocumented and unstable. Enable and use it only with express authorization appropriate to your use. This adapter does not bypass access controls, evade bot controls, or decipher media protections.
- YouTube Music web requests can be public or use a session cookie supplied explicitly by the caller. Credentials are not read from browsers, persisted, refreshed, or logged. Handle cookies/API keys as secrets. The client disables redirects by default and uses only the fixed Music origin.
- Playback is a YouTube watch/embed handoff. The crate does not resolve, extract, download, or expose direct audio stream URLs.
- A Music `video_id` suitable for player handoff is distinct from an album/artist `MusicId` unless explicitly populated as such.
- DuckDuckGo enrichment is not included; it is a separate future provider integration and must not be presented as authoritative YouTube Music artwork.

## Features

- `youtube-data` (default): official video search, metadata, and general popular-video chart.
- `youtube-music` (opt-in): YouTube Music web catalog requests, locale configuration, renderer-based search normalization, suggestions, and best-effort Music chart parsing. These endpoints may change without notice; entity detail and related-track methods currently return `Unsupported`.
- `full`: enables both provider features.

## YouTube Music setup

Search uses filter-specific encoded params, but filter/result coverage is best effort against an undocumented API. The parser recognizes known shelves and responsive list-item renderers and skips unknown layouts. Search continuation is forwarded opaquely. Suggestion order follows the service response. Chart results are best effort; chart continuation and standalone track/album/artist detail and related-track lookups remain unsupported.


A web API key is required by the current web interface; this is not a YouTube Data API key. Set up config without storing secrets in source code:

```rust,no_run
use yt_rust::{
    provider::YouTubeMusicProvider as MusicProvider,
    models::MusicSearchFilter,
    providers::youtube_music::{YouTubeMusicConfig, YouTubeMusicProvider},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("YOUTUBE_MUSIC_WEB_API_KEY")?;
    let client = YouTubeMusicProvider::new(YouTubeMusicConfig::with_locale(key, "US", "en")?)?;
    let results = MusicProvider::music_search(&client, "artist song", MusicSearchFilter::Songs, None).await?;
    for song in results.songs {
        println!("{} — {}", song.title, song.artists.first().map_or("unknown", |artist| artist.name.as_str()));
    }
    Ok(())
}
```

Provide an explicitly caller-owned session only when required and authorized: construct `MusicSession::new(cookie_header)?`, then pass it with `YouTubeMusicConfig::with_session`. Avoid printing config or credentials; debug output redacts secrets. Session endpoints/account features may have separate access and subscription limitations.

`music_suggestions` returns query suggestions in service order with exact duplicates removed. `music_charts(Some("US"), None)` queries the Music chart surface with `US` in the request client context; region is a two-letter country code. A response may omit content depending on locale/account. Search filters are sent as web-interface filter parameters, but their encodings and response layouts are undocumented and best effort. Chart continuation and `music_album`, `music_artist`, `music_track`, and `related_tracks` are currently `Unsupported`; the adapter does not guess at detail mappings or request player/streaming data.

## YouTube Data API setup

Use `YouTubeDataProvider::new(api_key)` and `DiscoveryProvider::search` / `trending`. Search uses the quota-expensive `search.list`; pages carry a `next_page_token`. `trending(Some("US"))` retrieves a general popular-video chart.

## Recommendations and playback

`recommendations::recommend` ranks caller-provided candidates against caller-provided listening history, with no implicit persistence. The provider `related_tracks` operation (where available) is distinct from this local heuristic and is not necessarily personalized. `Track::watch_url`, `Track::embed_url`, and `PlaybackHandoff` hand playback to YouTube's player, which applies its own availability rules.

## Documentation and tests

See [docs/YOUTUBE_MUSIC.md](docs/YOUTUBE_MUSIC.md) for the capability matrix and safety/configuration details. Examples live under `examples/`.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

The standard test suite is credential-free and makes no live API calls. Live checks should be opt-in and considered quota- and account-sensitive.

## Sources and thanks

YouTube Music web-interface request shapes were researched using the public [ytmusicapi reference](https://ytmusicapi.readthedocs.io/en/stable/reference/search.html). This crate is independent and not affiliated with YouTube or ytmusicapi.

YouTube API policy: https://developers.google.com/youtube/terms/developer-policies
**Do not use the optional undocumented adapter without express authorization.**
