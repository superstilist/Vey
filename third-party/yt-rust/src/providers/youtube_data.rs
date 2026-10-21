//! Adapter for documented YouTube Data API v3 endpoints only.
//!
//! `search.list` is quota-expensive. This provider does not access private
//! YouTube Music/InnerTube endpoints and does not resolve audio streams.

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use url::Url;

use crate::{
    error::{Error, Result},
    models::{Artist, ChartPage, SearchPage, Thumbnail, Track},
    provider::DiscoveryProvider,
};

const API_BASE: &str = "https://www.googleapis.com/youtube/v3/";
const SOURCE: &str = "youtube-data-api-v3";

/// Official YouTube Data API client.
#[derive(Clone)]
pub struct YouTubeDataProvider {
    client: Client,
    api_key: String,
}

impl YouTubeDataProvider {
    /// Construct a client using a caller-provided API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_client(
            api_key,
            Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()?,
        )
    }

    /// Construct a client with a configured reusable HTTP client.
    pub fn with_client(api_key: impl Into<String>, client: Client) -> Result<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(Error::MissingConfiguration("YouTube Data API key"));
        }
        Ok(Self { client, api_key })
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        Url::parse(API_BASE)
            .and_then(|base| base.join(path))
            .map_err(|error| Error::InvalidInput(format!("invalid API endpoint: {error}")))
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T> {
        let mut url = self.endpoint(path)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("key", &self.api_key);
            for (key, value) in params {
                query.append_pair(key, value);
            }
        }
        let response = self.client.get(url).send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            let message = serde_json::from_slice::<ApiErrorEnvelope>(&bytes)
                .ok()
                .and_then(|body| body.error.map(|error| error.message))
                .unwrap_or_else(|| "request failed".to_owned());
            if status == StatusCode::FORBIDDEN && message.to_ascii_lowercase().contains("quota") {
                return Err(Error::QuotaExceeded(message));
            }
            return Err(Error::Provider {
                status: status.as_u16(),
                message,
            });
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn video_items(&self, ids: &[String]) -> Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let response: VideoListResponse = self
            .get(
                "videos",
                &[
                    ("part", "snippet,contentDetails".to_owned()),
                    ("id", ids.join(",")),
                ],
            )
            .await?;
        Ok(response.items.into_iter().map(track_from_video).collect())
    }
}

impl DiscoveryProvider for YouTubeDataProvider {
    fn provider_name(&self) -> &'static str {
        SOURCE
    }

    fn search(
        &self,
        query: &str,
        page_token: Option<&str>,
    ) -> impl std::future::Future<Output = Result<SearchPage>> + Send {
        let query = query.to_owned();
        let page_token = page_token.map(str::to_owned);
        async move {
            if query.trim().is_empty() {
                return Err(Error::InvalidInput(
                    "search query cannot be empty".to_owned(),
                ));
            }
            let mut params = vec![
                ("part", "snippet".to_owned()),
                ("type", "video".to_owned()),
                ("q", query.trim().to_owned()),
                ("maxResults", "25".to_owned()),
            ];
            if let Some(token) = page_token {
                params.push(("pageToken", token));
            }
            let response: SearchResponse = self.get("search", &params).await?;
            let ids: Vec<String> = response
                .items
                .iter()
                .filter_map(|item| item.id.video_id.clone())
                .collect();
            let mut tracks = self.video_items(&ids).await?;
            if tracks.is_empty() {
                tracks = response
                    .items
                    .into_iter()
                    .filter_map(track_from_search)
                    .collect();
            }
            Ok(SearchPage {
                items: tracks,
                next_page_token: response.next_page_token,
                source: SOURCE.to_owned(),
            })
        }
    }

    fn trending(
        &self,
        region: Option<&str>,
    ) -> impl std::future::Future<Output = Result<ChartPage>> + Send {
        let region = region.map(str::to_owned);
        async move {
            let mut params = vec![
                ("part", "snippet,contentDetails".to_owned()),
                ("chart", "mostPopular".to_owned()),
                ("maxResults", "25".to_owned()),
            ];
            if let Some(region) = region.as_deref() {
                if region.len() != 2 || !region.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                    return Err(Error::InvalidInput(
                        "region must be a 2-letter country code".to_owned(),
                    ));
                }
                params.push(("regionCode", region.to_ascii_uppercase()));
            }
            let response: VideoListResponse = self.get("videos", &params).await?;
            Ok(ChartPage {
                items: response.items.into_iter().map(track_from_video).collect(),
                region: region.map(|value| value.to_ascii_uppercase()),
                is_music_chart: false,
                source: SOURCE.to_owned(),
            })
        }
    }

    fn video(
        &self,
        video_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Track>>> + Send {
        let video_id = video_id.to_owned();
        async move {
            if video_id.trim().is_empty() || video_id.contains(',') {
                return Err(Error::InvalidInput(
                    "video ID must be nonempty and contain no commas".to_owned(),
                ));
            }
            let response: VideoListResponse = self
                .get(
                    "videos",
                    &[
                        ("part", "snippet,contentDetails".to_owned()),
                        ("id", video_id.to_owned()),
                    ],
                )
                .await?;
            Ok(response.items.into_iter().next().map(track_from_video))
        }
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<SearchItem>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct SearchItem {
    #[serde(default)]
    id: SearchId,
    snippet: Snippet,
}

#[derive(Default, Deserialize)]
struct SearchId {
    #[serde(rename = "videoId")]
    video_id: Option<String>,
}

#[derive(Deserialize)]
struct VideoListResponse {
    #[serde(default)]
    items: Vec<VideoItem>,
}

#[derive(Deserialize)]
struct VideoItem {
    id: String,
    snippet: Snippet,
    #[serde(default)]
    #[serde(rename = "contentDetails")]
    content_details: Option<ContentDetails>,
}

#[derive(Deserialize)]
struct Snippet {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
    #[serde(rename = "channelTitle")]
    channel_title: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    #[serde(default)]
    thumbnails: ThumbnailSet,
}

#[derive(Default, Deserialize)]
struct ThumbnailSet {
    #[serde(default)]
    default: Option<ThumbnailEntry>,
    #[serde(default)]
    medium: Option<ThumbnailEntry>,
    #[serde(default)]
    high: Option<ThumbnailEntry>,
    #[serde(default)]
    standard: Option<ThumbnailEntry>,
    #[serde(default)]
    maxres: Option<ThumbnailEntry>,
}

#[derive(Deserialize)]
struct ThumbnailEntry {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Deserialize)]
struct ContentDetails {
    duration: Option<String>,
}

#[derive(Deserialize)]
struct ApiErrorEnvelope {
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

fn thumbnails(set: ThumbnailSet) -> Vec<Thumbnail> {
    [set.maxres, set.standard, set.high, set.medium, set.default]
        .into_iter()
        .flatten()
        .map(|item| Thumbnail {
            url: item.url,
            width: item.width,
            height: item.height,
        })
        .collect()
}

fn artist(snippet: &Snippet) -> Vec<Artist> {
    snippet
        .channel_title
        .iter()
        .map(|name| Artist {
            name: name.clone(),
            channel_id: snippet.channel_id.clone(),
        })
        .collect()
}

fn track_from_video(item: VideoItem) -> Track {
    let artists = artist(&item.snippet);
    Track {
        video_id: item.id,
        title: item.snippet.title,
        artists,
        album: None,
        description: item.snippet.description,
        duration: item.content_details.and_then(|details| details.duration),
        thumbnails: thumbnails(item.snippet.thumbnails),
        channel_id: item.snippet.channel_id,
        published_at: item.snippet.published_at,
        source: SOURCE.to_owned(),
    }
}

fn track_from_search(item: SearchItem) -> Option<Track> {
    let artists = artist(&item.snippet);
    Some(Track {
        video_id: item.id.video_id?,
        title: item.snippet.title,
        artists,
        album: None,
        description: item.snippet.description,
        duration: None,
        thumbnails: thumbnails(item.snippet.thumbnails),
        channel_id: item.snippet.channel_id,
        published_at: item.snippet.published_at,
        source: SOURCE.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_video_metadata_with_optional_fields() {
        let item: VideoItem = serde_json::from_str(
            r#"{
                "id":"abc123",
                "snippet":{
                    "title":"Example song",
                    "channelId":"channel1",
                    "channelTitle":"Example Artist",
                    "thumbnails":{"high":{"url":"https://i.ytimg.com/thumb.jpg","width":480,"height":360}}
                },
                "contentDetails":{"duration":"PT3M10S"}
            }"#,
        ).unwrap();
        let track = track_from_video(item);
        assert_eq!(track.video_id, "abc123");
        assert_eq!(track.title, "Example song");
        assert_eq!(track.artists[0].name, "Example Artist");
        assert_eq!(track.duration.as_deref(), Some("PT3M10S"));
        assert_eq!(track.thumbnails[0].width, Some(480));
    }

    #[test]
    fn maps_search_page_token_and_drops_non_video_results() {
        let response: SearchResponse = serde_json::from_str(
            r#"{
                "nextPageToken":"next",
                "items":[
                    {"id":{"videoId":"v1"},"snippet":{"title":"Track","channelTitle":"Artist"}},
                    {"id":{"channelId":"c1"},"snippet":{"title":"Channel"}}
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(response.next_page_token.as_deref(), Some("next"));
        assert_eq!(
            response
                .items
                .into_iter()
                .filter_map(track_from_search)
                .count(),
            1
        );
    }

    #[test]
    fn thumbnail_variants_are_ordered_largest_first() {
        let set: ThumbnailSet = serde_json::from_str(
            r#"{"default":{"url":"d"},"high":{"url":"h"},"maxres":{"url":"m"}}"#,
        )
        .unwrap();
        assert_eq!(
            thumbnails(set)
                .iter()
                .map(|item| item.url.as_str())
                .collect::<Vec<_>>(),
            ["m", "h", "d"]
        );
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;

    #[tokio::test]
    async fn rejects_empty_api_key_and_query_before_network_access() {
        assert!(matches!(
            YouTubeDataProvider::new("  "),
            Err(Error::MissingConfiguration(_))
        ));
        let provider = YouTubeDataProvider::new("test-key").unwrap();
        assert!(matches!(
            provider.search("  ", None).await,
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            provider.video("bad,id").await,
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(
            provider.trending(Some("USA")).await,
            Err(Error::InvalidInput(_))
        ));
    }
}
