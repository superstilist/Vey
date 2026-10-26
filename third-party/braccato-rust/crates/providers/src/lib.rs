//! Native asynchronous provider chain. Network access is performed with Rust HTTP clients.

use braccato_types::Lyric;
use serde::Deserialize;
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, time::Duration};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Default)]
pub struct ProviderContext {
    pub song: String,
    pub artist: String,
    pub duration_s: f64,
    pub album: Option<String>,
    pub video_id: Option<String>,
    pub cancellation: CancellationToken,
}
#[derive(Clone, Debug)]
pub struct LyricSourceResult {
    pub lyrics: Vec<Lyric>,
    pub language: Option<String>,
    pub songwriters: Vec<String>,
    pub source: String,
    pub source_href: String,
    pub cache_allowed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyncQuality {
    Syllable,
    Word,
    Line,
    Unsynced,
}
pub type ProviderFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<LyricSourceResult>, ProviderError>> + Send + 'a>>;
pub type ProviderFn = Arc<dyn for<'a> Fn(&'a ProviderContext) -> ProviderFuture<'a> + Send + Sync>;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("provider response could not be parsed: {0}")]
    Parse(#[from] serde_json::Error),
}
#[derive(Default)]
pub struct ProviderChain {
    providers: HashMap<String, (SyncQuality, ProviderFn)>,
    priority: Vec<String>,
    has_custom_priority: bool,
    cache: std::sync::Mutex<HashMap<String, Option<LyricSourceResult>>>,
}
impl ProviderChain {
    pub fn register(&mut self, key: impl Into<String>, quality: SyncQuality, provider: ProviderFn) {
        let key = key.into();
        self.providers.insert(key.clone(), (quality, provider));
        if !self.priority.contains(&key) {
            self.priority.push(key);
        }
    }
    pub fn set_priority(&mut self, keys: Vec<String>) {
        self.priority = keys;
        self.has_custom_priority = true;
    }
    pub fn unregister(&mut self, key: &str) -> bool {
        self.priority.retain(|candidate| candidate != key);
        self.providers.remove(key).is_some()
    }
    pub fn registered(&self) -> Vec<(String, SyncQuality)> {
        self.providers
            .iter()
            .map(|(key, (quality, _))| (key.clone(), *quality))
            .collect()
    }
    pub fn clear_cache(&self) {
        self.cache.lock().expect("provider cache poisoned").clear();
    }
    pub fn fetch<'a>(
        &'a self,
        context: &'a ProviderContext,
        disabled: &'a [String],
        reference: Option<(&'a str, f64)>,
    ) -> ProviderFuture<'a> {
        Box::pin(async move {
            if context.cancellation.is_cancelled() {
                return Ok(None);
            }
            let order = if self.has_custom_priority {
                self.priority.clone()
            } else {
                let mut ranked = self
                    .providers
                    .iter()
                    .map(|(k, (q, _))| (k.clone(), *q))
                    .collect::<Vec<_>>();
                ranked.sort_by_key(|(_, quality)| *quality);
                ranked.into_iter().map(|(key, _)| key).collect()
            };
            for key in order {
                if disabled.contains(&key) {
                    continue;
                }
                let Some((_, provider)) = self.providers.get(&key) else {
                    continue;
                };
                let cached = self
                    .cache
                    .lock()
                    .expect("provider cache poisoned")
                    .get(&key)
                    .cloned();
                let result = if let Some(cached) = cached {
                    Ok(cached)
                } else {
                    let result = tokio::select! {
                        biased;
                        _ = context.cancellation.cancelled() => return Ok(None),
                        result = provider(context) => result,
                    };
                    let cached = result.as_ref().ok().cloned().flatten();
                    self.cache
                        .lock()
                        .expect("provider cache poisoned")
                        .insert(key.clone(), cached);
                    result
                };
                match result {
                    Ok(Some(result)) if !result.lyrics.is_empty() => {
                        if let Some((reference, threshold)) = reference
                            && similarity(
                                reference,
                                &result
                                    .lyrics
                                    .iter()
                                    .map(|l| l.words.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ) < threshold
                        {
                            continue;
                        }
                        return Ok(Some(result));
                    }
                    Ok(_) => {}
                    Err(_) => continue,
                }
            }
            Ok(None)
        })
    }
}

fn http_client(timeout: Duration) -> Result<reqwest::Client, ProviderError> {
    Ok(reqwest::Client::builder().timeout(timeout).build()?)
}
#[derive(Deserialize)]
struct LrcLibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

pub fn lrclib_synced(api_url: impl Into<String>) -> ProviderFn {
    let url = api_url.into();
    Arc::new(move |ctx| {
        let url = url.clone();
        Box::pin(async move {
            let client = http_client(Duration::from_secs(10))?;
            let mut req = client
                .get(&url)
                .query(&[("track_name", &ctx.song), ("artist_name", &ctx.artist)])
                .query(&[("duration", ctx.duration_s)]);
            if let Some(album) = &ctx.album {
                req = req.query(&[("album_name", album)]);
            }
            let response = req
                .header(
                    "Lrclib-Client",
                    "Braccato (https://github.com/better-lyrics/braccato)",
                )
                .send()
                .await?;
            if !response.status().is_success() {
                return Ok(None);
            }
            let data: LrcLibResponse = response.json().await?;
            let Some(text) = data.synced_lyrics else {
                return Ok(None);
            };
            let lyrics = braccato_parsers::parse_lrc(&text, ctx.duration_s * 1000.0);
            if lyrics.is_empty() {
                return Ok(None);
            }
            Ok(Some(LyricSourceResult {
                lyrics,
                language: None,
                songwriters: vec![],
                source: "LRCLib".into(),
                source_href: "https://lrclib.net/".into(),
                cache_allowed: true,
            }))
        })
    })
}

pub fn lrclib_plain(api_url: impl Into<String>) -> ProviderFn {
    let url = api_url.into();
    Arc::new(move |ctx| {
        let url = url.clone();
        Box::pin(async move {
            let client = http_client(Duration::from_secs(10))?;
            let mut req = client
                .get(&url)
                .query(&[("track_name", &ctx.song), ("artist_name", &ctx.artist)])
                .query(&[("duration", ctx.duration_s)]);
            if let Some(album) = &ctx.album {
                req = req.query(&[("album_name", album)]);
            }
            let response = req.send().await?;
            if !response.status().is_success() {
                return Ok(None);
            }
            let data: LrcLibResponse = response.json().await?;
            let Some(text) = data.plain_lyrics else {
                return Ok(None);
            };
            let lyrics = braccato_parsers::parse(&text, 0.0).unwrap_or_default();
            if lyrics.is_empty() {
                return Ok(None);
            }
            Ok(Some(LyricSourceResult {
                lyrics,
                language: None,
                songwriters: braccato_parsers::lrc_songwriters(&text),
                source: "LRCLib".into(),
                source_href: "https://lrclib.net/".into(),
                cache_allowed: false,
            }))
        })
    })
}

/// Better Lyrics API backed by the provider's TTML endpoint.
pub fn blyrics(api_url: impl Into<String>) -> ProviderFn {
    let url = api_url.into();
    Arc::new(move |ctx| {
        let url = url.clone();
        Box::pin(async move {
            let response = with_track_query(
                http_client(Duration::from_secs(10))?.get(url),
                ctx,
                "s",
                "a",
                "d",
                "al",
            )
            .send()
            .await?;
            if !response.status().is_success() {
                return Ok(None);
            }
            let payload: serde_json::Value = response.json().await?;
            let Some(ttml) = payload.get("ttml").and_then(|v| v.as_str()) else {
                return Ok(None);
            };
            let lyrics = braccato_parsers::parse(ttml, ctx.duration_s * 1000.0).unwrap_or_default();
            if lyrics.is_empty() {
                return Ok(None);
            }
            Ok(Some(LyricSourceResult {
                lyrics,
                language: payload
                    .get("lang")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                songwriters: braccato_parsers::ttml_songwriters(ttml),
                source: "boidu.dev".into(),
                source_href: "https://boidu.dev/".into(),
                cache_allowed: true,
            }))
        })
    })
}

/// Better Lyrics' Kugou gateway, returning enhanced LRC.
pub fn legato(api_url: impl Into<String>) -> ProviderFn {
    let url = api_url.into();
    Arc::new(move |ctx| {
        let url = url.clone();
        Box::pin(async move {
            let response = with_track_query(
                http_client(Duration::from_secs(10))?.get(url),
                ctx,
                "s",
                "a",
                "d",
                "al",
            )
            .send()
            .await?;
            if !response.status().is_success() {
                return Ok(None);
            }
            let payload: serde_json::Value = response.json().await?;
            let Some(text) = payload.get("lyrics").and_then(|v| v.as_str()) else {
                return Ok(None);
            };
            let lyrics = braccato_parsers::parse_lrc(text, ctx.duration_s * 1000.0);
            if lyrics.is_empty() {
                return Ok(None);
            }
            Ok(Some(LyricSourceResult {
                lyrics,
                language: None,
                songwriters: braccato_parsers::lrc_songwriters(text),
                source: "Better Lyrics Legato".into(),
                source_href: "https://boidu.dev/".into(),
                cache_allowed: true,
            }))
        })
    })
}

/// Better Lyrics' QQ Music gateway, returning QRC word timing.
pub fn portato(api_url: impl Into<String>) -> ProviderFn {
    let url = api_url.into();
    Arc::new(move |ctx| {
        let url = url.clone();
        Box::pin(async move {
            let response = with_track_query(
                http_client(Duration::from_secs(10))?.get(url),
                ctx,
                "s",
                "a",
                "d",
                "al",
            )
            .send()
            .await?;
            if !response.status().is_success() {
                return Ok(None);
            }
            let payload: serde_json::Value = response.json().await?;
            if payload
                .get("error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Ok(None);
            }
            let Some(text) = payload.get("lyrics").and_then(|v| v.as_str()) else {
                return Ok(None);
            };
            let lyrics = braccato_parsers::parse(text, ctx.duration_s * 1000.0).unwrap_or_default();
            if lyrics.is_empty() {
                return Ok(None);
            }
            Ok(Some(LyricSourceResult {
                lyrics,
                language: None,
                songwriters: braccato_parsers::qrc_songwriters(text),
                source: "Better Lyrics Portato".into(),
                source_href: "https://boidu.dev/".into(),
                cache_allowed: true,
            }))
        })
    })
}

/// Unison is keyed primarily by video ID when the caller has one.
pub fn unison(
    api_url: impl Into<String>,
    source_href: impl Into<String>,
    key_id: Option<String>,
) -> ProviderFn {
    let url = api_url.into();
    let href = source_href.into();
    Arc::new(move |ctx| {
        let url = url.clone();
        let href = href.clone();
        let key_id = key_id.clone();
        Box::pin(async move {
            let client = http_client(Duration::from_secs(10))?;
            let mut request = client
                .get(url)
                .query(&[("song", &ctx.song), ("artist", &ctx.artist)])
                .query(&[("duration", ctx.duration_s.round())]);
            if let Some(video) = &ctx.video_id {
                request = request.query(&[("v", video)]);
            }
            if let Some(album) = &ctx.album {
                request = request.query(&[("album", album)]);
            }
            if let Some(key) = key_id {
                request = request.header("x-key-id", key);
            }
            let response = request.send().await?;
            if !response.status().is_success() {
                return Ok(None);
            }
            let payload: serde_json::Value = response.json().await?;
            let Some(data) = payload.get("data") else {
                return Ok(None);
            };
            let Some(format) = data.get("format").and_then(|v| v.as_str()) else {
                return Ok(None);
            };
            let Some(text) = data.get("lyrics").and_then(|v| v.as_str()) else {
                return Ok(None);
            };
            let lyrics = match format {
                "ttml" | "lrc" | "plain" => {
                    braccato_parsers::parse(text, ctx.duration_s * 1000.0).unwrap_or_default()
                }
                _ => Vec::new(),
            };
            if lyrics.is_empty() {
                return Ok(None);
            }
            Ok(Some(LyricSourceResult {
                lyrics,
                language: None,
                songwriters: match format {
                    "ttml" => braccato_parsers::ttml_songwriters(text),
                    "lrc" => braccato_parsers::lrc_songwriters(text),
                    _ => vec![],
                },
                source: "Unison".into(),
                source_href: href,
                cache_allowed: false,
            }))
        })
    })
}

/// BiniLyrics search followed by its native TTML URL fetch.
pub fn binimum(api_url: impl Into<String>) -> ProviderFn {
    let url = api_url.into();
    Arc::new(move |ctx| {
        let url = url.clone();
        Box::pin(async move {
            let client = http_client(Duration::from_secs(10))?;
            let search =
                with_track_query(client.get(url), ctx, "track", "artist", "duration", "album")
                    .send()
                    .await?;
            if !search.status().is_success() {
                return Ok(None);
            }
            let payload: serde_json::Value = search.json().await?;
            let Some(lyrics_url) = payload
                .get("results")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.get("lyricsUrl"))
                .and_then(|v| v.as_str())
            else {
                return Ok(None);
            };
            let response = http_client(Duration::from_secs(10))?
                .get(lyrics_url)
                .send()
                .await?;
            if !response.status().is_success() {
                return Ok(None);
            }
            let ttml = response.text().await?;
            let lyrics =
                braccato_parsers::parse(&ttml, ctx.duration_s * 1000.0).unwrap_or_default();
            if lyrics.is_empty() {
                return Ok(None);
            }
            Ok(Some(LyricSourceResult {
                lyrics,
                language: None,
                songwriters: braccato_parsers::ttml_songwriters(&ttml),
                source: "BiniLyrics".into(),
                source_href: "https://lyrics-api.binimum.org/".into(),
                cache_allowed: true,
            }))
        })
    })
}

fn with_track_query(
    mut request: reqwest::RequestBuilder,
    ctx: &ProviderContext,
    song: &str,
    artist: &str,
    duration: &str,
    album: &str,
) -> reqwest::RequestBuilder {
    request = request
        .query(&[(song, &ctx.song), (artist, &ctx.artist)])
        .query(&[(duration, ctx.duration_s)]);
    if let Some(value) = &ctx.album {
        request = request.query(&[(album, value)]);
    }
    request
}

pub fn similarity(a: &str, b: &str) -> f64 {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let n = 2;
    if a.chars().count() < n || b.chars().count() < n {
        return 0.0;
    }
    let grams = |s: &str| {
        s.as_bytes()
            .windows(n)
            .map(|w| String::from_utf8_lossy(w).to_string())
            .collect::<Vec<_>>()
    };
    let mut left = grams(&a);
    let mut matched = 0;
    for gram in grams(&b) {
        if let Some(i) = left.iter().position(|g| *g == gram) {
            left.remove(i);
            matched += 1;
        }
    }
    (2 * matched) as f64 / (a.len() + b.len() - 2) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use braccato_types::Lyric;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    #[test]
    fn similarity_uses_bigrams() {
        assert!(similarity("hello there", "hello there") > 0.99);
        assert!(similarity("hello", "xxxxx") < 0.3);
    }
    fn result() -> LyricSourceResult {
        LyricSourceResult {
            lyrics: vec![Lyric {
                words: "right lyrics".into(),
                ..Default::default()
            }],
            language: None,
            songwriters: vec![],
            source: "mock".into(),
            source_href: "http://mock".into(),
            cache_allowed: true,
        }
    }

    #[tokio::test]
    async fn chain_falls_back_after_provider_error_and_validation_rejection() {
        let mut chain = ProviderChain::default();
        let broken: ProviderFn = Arc::new(|_| {
            Box::pin(async {
                Err(ProviderError::Parse(
                    serde_json::from_str::<u8>("invalid").unwrap_err(),
                ))
            })
        });
        let wrong = LyricSourceResult {
            lyrics: vec![Lyric {
                words: "unrelated text".into(),
                ..Default::default()
            }],
            ..result()
        };
        let wrong_provider: ProviderFn = Arc::new(move |_| {
            let value = wrong.clone();
            Box::pin(async move { Ok(Some(value)) })
        });
        let good = result();
        let good_provider: ProviderFn = Arc::new(move |_| {
            let value = good.clone();
            Box::pin(async move { Ok(Some(value)) })
        });
        chain.register("broken", SyncQuality::Syllable, broken);
        chain.register("wrong", SyncQuality::Word, wrong_provider);
        chain.register("good", SyncQuality::Line, good_provider);
        let found = chain
            .fetch(
                &ProviderContext::default(),
                &[],
                Some(("right lyrics", 0.9)),
            )
            .await
            .unwrap();
        assert_eq!(found.unwrap().source, "mock");
    }

    #[tokio::test]
    async fn cancellation_stops_before_the_next_provider() {
        let mut chain = ProviderChain::default();
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let marker = called.clone();
        let provider: ProviderFn = Arc::new(move |_| {
            let marker = marker.clone();
            Box::pin(async move {
                marker.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(Some(result()))
            })
        });
        chain.register("mock", SyncQuality::Line, provider);
        let context = ProviderContext::default();
        context.cancellation.cancel();
        assert!(chain.fetch(&context, &[], None).await.unwrap().is_none());
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn chain_caches_results_and_can_clear_or_unregister_providers() {
        let mut chain = ProviderChain::default();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let marker = calls.clone();
        let provider: ProviderFn = Arc::new(move |_| {
            let marker = marker.clone();
            Box::pin(async move {
                marker.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(Some(result()))
            })
        });
        chain.register("cached", SyncQuality::Line, provider);
        let context = ProviderContext::default();
        assert!(chain.fetch(&context, &[], None).await.unwrap().is_some());
        assert!(chain.fetch(&context, &[], None).await.unwrap().is_some());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        chain.clear_cache();
        assert!(chain.fetch(&context, &[], None).await.unwrap().is_some());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert!(chain.unregister("cached"));
        assert!(chain.fetch(&context, &[], None).await.unwrap().is_none());
        assert_eq!(chain.registered().len(), 0);
    }

    #[tokio::test]
    async fn lrclib_provider_uses_native_http_and_parses_response() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let n = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..n]).to_lowercase();
            assert!(request.contains("track_name=song"));
            assert!(request.contains("lrclib-client:"));
            let body = r#"{"syncedLyrics":"[00:01.00]Hello"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let provider = lrclib_synced(format!("http://{address}/api/get"));
        let context = ProviderContext {
            song: "Song".into(),
            artist: "Artist".into(),
            duration_s: 10.0,
            ..ProviderContext::default()
        };
        let response = provider(&context).await.unwrap().unwrap();
        assert_eq!(response.source, "LRCLib");
        assert_eq!(response.lyrics[0].words, "Hello");
        server.await.unwrap();
    }
}
