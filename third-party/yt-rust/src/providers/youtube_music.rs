//! Opt-in adapter for YouTube Music's undocumented web interface.
//!
//! Use only where the caller has express authorization. The interface is
//! unstable and may change without notice. This adapter performs catalog
//! requests only; it does not resolve or download playable media streams.

use std::time::Duration;

use reqwest::{header, Client, StatusCode};
use serde_json::{json, Value};
use url::Url;

use crate::{
    error::{Error, Result},
    models::{
        Artist, MusicAlbum, MusicAlbumDetails, MusicAlbumRef, MusicArtist, MusicArtistDetails,
        MusicChartPage, MusicId, MusicPlaylist, MusicSearchFilter, MusicSearchPage, MusicTrack,
        Suggestion, Thumbnail,
    },
    provider::YouTubeMusicProvider as YouTubeMusicProviderTrait,
};

const API_BASE: &str = "https://music.youtube.com/youtubei/v1/";
const SOURCE: &str = "youtube-music-web";
const CLIENT_NAME: &str = "WEB_REMIX";
const CLIENT_VERSION: &str = "1.20261001.01.00";

/// Caller-owned optional session credentials. The value should be a cookie
/// header string from an authorized session and is never stored by this crate.
#[derive(Clone)]
pub struct MusicSession {
    cookie_header: String,
}

impl MusicSession {
    /// Construct from a caller-provided Cookie header value.
    pub fn new(cookie_header: impl Into<String>) -> Result<Self> {
        let cookie_header = cookie_header.into();
        if cookie_header.trim().is_empty()
            || cookie_header.contains(['\r', '\n'])
            || !cookie_header.is_ascii()
        {
            return Err(Error::InvalidInput(
                "session cookie must be a nonempty single-line ASCII header".to_owned(),
            ));
        }
        Ok(Self { cookie_header })
    }
}

impl std::fmt::Debug for MusicSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MusicSession")
            .field("cookie_header", &"[REDACTED]")
            .finish()
    }
}

/// Configuration for public or explicitly caller-authenticated requests.
#[derive(Clone)]
pub struct YouTubeMusicConfig {
    pub api_key: String,
    pub country: String,
    pub language: String,
    pub session: Option<MusicSession>,
}

impl std::fmt::Debug for YouTubeMusicConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("YouTubeMusicConfig")
            .field("api_key", &"[REDACTED]")
            .field("country", &self.country)
            .field("language", &self.language)
            .field("session", &self.session)
            .finish()
    }
}

impl YouTubeMusicConfig {
    /// Create public (unauthenticated) configuration. `api_key` is required by
    /// the current web interface and is not a YouTube Data API key.
    pub fn public(api_key: impl Into<String>) -> Result<Self> {
        Self::with_locale(api_key, "US", "en")
    }

    /// Create public configuration with an explicit country and language.
    pub fn with_locale(
        api_key: impl Into<String>,
        country: impl Into<String>,
        language: impl Into<String>,
    ) -> Result<Self> {
        let mut config = Self {
            api_key: api_key.into(),
            country: country.into(),
            language: language.into(),
            session: None,
        };
        config.validate()?;
        config.country.make_ascii_uppercase();
        config.language.make_ascii_lowercase();
        Ok(config)
    }

    /// Opt in to using a caller-owned authorized session.
    pub fn with_session(mut self, session: MusicSession) -> Self {
        self.session = Some(session);
        self
    }

    fn validate(&self) -> Result<()> {
        if self.api_key.trim().is_empty() {
            return Err(Error::MissingConfiguration("YouTube Music web API key"));
        }
        if self.country.len() != 2 || !self.country.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(Error::InvalidInput(
                "country must be a 2-letter country code".to_owned(),
            ));
        }
        if self.language.is_empty()
            || self.language.len() > 35
            || !self
                .language
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(Error::InvalidInput(
                "language must be a BCP-47-like tag".to_owned(),
            ));
        }
        Ok(())
    }
}

/// YouTube Music web catalog provider. Credentials are only attached to the
/// fixed `music.youtube.com` origin; requests never follow redirects.
#[derive(Clone)]
pub struct YouTubeMusicProvider {
    client: Client,
    config: YouTubeMusicConfig,
    base_url: Url,
}

impl YouTubeMusicProvider {
    pub fn new(config: YouTubeMusicConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Self::with_client(config, client)
    }

    /// Inject a reusable client for testing or caller-managed transport.
    /// The client is caller-controlled; configure it to disable redirects and
    /// ensure it does not forward credentials to other origins.
    pub fn with_client(config: YouTubeMusicConfig, client: Client) -> Result<Self> {
        config.validate()?;
        let base_url = Url::parse(API_BASE).expect("constant API base URL is valid");
        Ok(Self {
            client,
            config,
            base_url,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        let url = self
            .base_url
            .join(path)
            .map_err(|error| Error::InvalidInput(format!("invalid Music endpoint: {error}")))?;
        if url.origin() != self.base_url.origin() || !url.path().starts_with("/youtubei/v1/") {
            return Err(Error::InvalidInput(
                "Music request path escaped the fixed endpoint".to_owned(),
            ));
        }
        Ok(url)
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.post_with_country(path, body, &self.config.country)
            .await
    }

    async fn post_with_country(&self, path: &str, body: Value, country: &str) -> Result<Value> {
        let mut url = self.endpoint(path)?;
        url.query_pairs_mut()
            .append_pair("key", &self.config.api_key);
        let context = json!({
            "client": {
                "clientName": CLIENT_NAME,
                "clientVersion": CLIENT_VERSION,
                "hl": self.config.language,
                "gl": country.to_ascii_uppercase(),
                "platform": "DESKTOP"
            }
        });
        let mut request = self.client.post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, "https://music.youtube.com")
            .header(header::REFERER, "https://music.youtube.com/")
            .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36")
            .json(&with_context(body, context));
        if let Some(session) = &self.config.session {
            request = request.header(header::COOKIE, &session.cookie_header);
        }
        let response = request.send().await?;
        let status = response.status();
        if status.is_redirection() {
            return Err(Error::Provider {
                status: status.as_u16(),
                message: "unexpected redirect; credentials were not forwarded".to_owned(),
            });
        }
        let bytes = response.bytes().await?;
        if !status.is_success() {
            let message = provider_message(&bytes);
            if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE
            {
                return Err(Error::Provider {
                    status: status.as_u16(),
                    message: "Music service is rate limiting or temporarily unavailable".to_owned(),
                });
            }
            return Err(Error::Provider {
                status: status.as_u16(),
                message,
            });
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

impl YouTubeMusicProvider {
    async fn search_page(
        &self,
        query: &str,
        filter: MusicSearchFilter,
        continuation: Option<&str>,
    ) -> Result<MusicSearchPage> {
        if query.trim().is_empty() {
            return Err(Error::InvalidInput(
                "search query cannot be empty".to_owned(),
            ));
        }
        let mut body = if let Some(token) = continuation {
            json!({ "continuation": token })
        } else {
            json!({ "query": query.trim() })
        };
        if let Some(params) = search_filter_params(filter) {
            body["params"] = json!(params);
        }
        let response = self.post("search", body).await?;
        parse_search(&response)
    }
}

impl YouTubeMusicProviderTrait for YouTubeMusicProvider {
    fn music_search(
        &self,
        query: &str,
        filter: MusicSearchFilter,
        continuation: Option<&str>,
    ) -> impl std::future::Future<Output = Result<MusicSearchPage>> + Send {
        let query = query.to_owned();
        let continuation = continuation.map(str::to_owned);
        async move {
            self.search_page(&query, filter, continuation.as_deref())
                .await
        }
    }

    fn music_suggestions(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Suggestion>>> + Send {
        let query = query.to_owned();
        async move {
            if query.trim().is_empty() {
                return Err(Error::InvalidInput(
                    "suggestion query cannot be empty".to_owned(),
                ));
            }
            let response = self
                .post(
                    "music/get_search_suggestions",
                    json!({ "input": query.trim() }),
                )
                .await?;
            Ok(parse_suggestions(&response))
        }
    }

    fn music_charts(
        &self,
        region: Option<&str>,
        continuation: Option<&str>,
    ) -> impl std::future::Future<Output = Result<MusicChartPage>> + Send {
        let region = region
            .map(str::to_owned)
            .unwrap_or_else(|| self.config.country.clone());
        let continuation = continuation.map(str::to_owned);
        async move {
            if region.len() != 2 || !region.bytes().all(|b| b.is_ascii_alphabetic()) {
                return Err(Error::InvalidInput(
                    "region must be a 2-letter country code".to_owned(),
                ));
            }
            if continuation.is_some() {
                return Err(Error::Unsupported(
                    "chart continuation until a chart response supplies a continuation token",
                ));
            }
            let response = self
                .post_with_country("browse", json!({ "browseId": "FEmusic_charts" }), &region)
                .await?;
            parse_chart(&response, &region)
        }
    }

    fn music_track(
        &self,
        _id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Option<MusicTrack>>> + Send {
        std::future::ready(Err(Error::Unsupported(
            "Music track metadata lookup; no verified metadata-only browse response mapping",
        )))
    }

    fn music_album(
        &self,
        _id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Option<MusicAlbumDetails>>> + Send {
        std::future::ready(Err(Error::Unsupported(
            "Music album detail lookup; no verified browse response mapping",
        )))
    }

    fn music_artist(
        &self,
        _id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Option<MusicArtistDetails>>> + Send {
        std::future::ready(Err(Error::Unsupported(
            "Music artist detail lookup; no verified browse response mapping",
        )))
    }

    fn related_tracks(
        &self,
        _id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Vec<MusicTrack>>> + Send {
        std::future::ready(Err(Error::Unsupported(
            "related-track feed; no verified response mapping",
        )))
    }
}

fn with_context(mut body: Value, context: Value) -> Value {
    if let Some(object) = body.as_object_mut() {
        object.insert("context".to_owned(), context);
    }
    body
}

fn provider_message(bytes: &[u8]) -> String {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => return "Music service request failed".to_owned(),
    };
    value
        .get("message")
        .and_then(text)
        .unwrap_or_else(|| "Music service request failed".to_owned())
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            value
                .get("simpleText")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| {
            value
                .get("runs")
                .and_then(Value::as_array)
                .map(|runs| {
                    runs.iter()
                        .filter_map(|run| run.get("text").and_then(Value::as_str))
                        .collect::<String>()
                })
                .filter(|s| !s.is_empty())
        })
}

fn thumbnails(value: &Value) -> Vec<Thumbnail> {
    value
        .get("thumbnails")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|thumbnail| {
            Some(Thumbnail {
                url: thumbnail.get("url")?.as_str()?.to_owned(),
                width: thumbnail
                    .get("width")
                    .and_then(Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok()),
                height: thumbnail
                    .get("height")
                    .and_then(Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok()),
            })
        })
        .collect()
}

fn search_filter_params(filter: MusicSearchFilter) -> Option<&'static str> {
    Some(match filter {
        MusicSearchFilter::Songs => "EgWKAQIIAWoMEA4QChADEAQQCRAF",
        MusicSearchFilter::Videos => "EgWKAQIQAWoMEA4QChADEAQQCRAF",
        MusicSearchFilter::Albums => "EgWKAQIYAWoMEA4QChADEAQQCRAF",
        MusicSearchFilter::Artists => "EgWKAQIgAWoMEA4QChADEAQQCRAF",
        MusicSearchFilter::Playlists => "Eg-KAQwIABAAGAAgACgBMABqChAEEAMQCRAFEAo%3D",
        MusicSearchFilter::CommunityPlaylists => "EgeKAQQoAEABagwQDhAKEAMQBBAJEAU%3D",
    })
}

fn search_sections(response: &Value) -> Option<&Vec<Value>> {
    response.pointer("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .and_then(Value::as_array)
        .or_else(|| response.pointer("/contents/sectionListRenderer/contents").and_then(Value::as_array))
}

/// Decode catalog responses supplied by a host application using its own transport.
pub fn parse_search(response: &Value) -> Result<MusicSearchPage> {
    // Continuations have no tab/section wrapper. Reuse the same row decoder
    // rather than dropping a valid subsequent page as an unknown response.
    for path in [
        "/continuationContents/musicShelfContinuation",
        "/continuationContents/musicPlaylistShelfContinuation",
    ] {
        if let Some(shelf) = response.pointer(path) {
            let wrapped = json!({"contents":{"sectionListRenderer":{"contents":[
                {"musicShelfRenderer": shelf}
            ]}}});
            return parse_search(&wrapped);
        }
    }
    let sections = search_sections(response).ok_or_else(|| Error::Provider {
        status: 200,
        message: "unrecognized Music search response layout".to_owned(),
    })?;
    let mut page = MusicSearchPage {
        source: SOURCE.to_owned(),
        ..MusicSearchPage::default()
    };
    let mut seen = std::collections::HashSet::new();
    for section in sections {
        let shelf = section
            .get("musicShelfRenderer")
            .or_else(|| section.get("musicCardShelfRenderer"));
        let Some(shelf) = shelf else {
            continue;
        };
        let category = shelf
            .get("title")
            .and_then(text)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let kind = category_kind(&category);
        if section.get("musicCardShelfRenderer").is_some() {
            let category = shelf
                .get("subtitle")
                .and_then(text)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if let Some((key, parsed)) = parse_card_shelf(shelf, category_kind(&category)) {
                if seen.insert(key) {
                    push_search_item(&mut page, parsed);
                }
            }
        }
        if let Some(items) = shelf.get("contents").and_then(Value::as_array) {
            for item in items {
                let Some(renderer) = item.get("musicResponsiveListItemRenderer") else {
                    continue;
                };
                if let Some((key, parsed)) = parse_search_renderer(renderer, kind) {
                    if seen.insert(key) {
                        push_search_item(&mut page, parsed);
                    }
                }
            }
        }
        if let Some(token) = shelf
            .get("continuations")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find_map(|item| {
                item.pointer("/nextContinuationData/continuation")
                    .and_then(Value::as_str)
            })
        {
            page.continuation = Some(token.to_owned());
        }
    }
    if page.continuation.is_none() {
        page.continuation = response.pointer("/continuationContents/musicShelfContinuation/continuations/0/nextContinuationData/continuation")
            .and_then(Value::as_str).map(str::to_owned);
    }
    for item_section in sections
        .iter()
        .filter_map(|section| section.get("itemSectionRenderer"))
    {
        for item in item_section
            .get("contents")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(card) = item.get("musicCardShelfRenderer") {
                let category = card
                    .get("subtitle")
                    .and_then(text)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let kind = category_kind(&category);
                if let Some((key, parsed)) = parse_card_shelf(card, kind) {
                    if seen.insert(key) {
                        push_search_item(&mut page, parsed);
                    }
                }
            }
        }
    }
    Ok(page)
}

#[derive(Clone, Copy, Debug)]
enum SearchKind {
    Song,
    Video,
    Album,
    Artist,
    Playlist,
    Unknown,
}

fn category_kind(category: &str) -> SearchKind {
    if category.contains("song") {
        SearchKind::Song
    } else if category.contains("video") {
        SearchKind::Video
    } else if category.contains("album") || category.contains("single") || category.contains("ep") {
        SearchKind::Album
    } else if category.contains("artist") {
        SearchKind::Artist
    } else if category.contains("playlist") {
        SearchKind::Playlist
    } else {
        SearchKind::Unknown
    }
}

fn parse_card_shelf(card: &Value, kind: SearchKind) -> Option<(String, SearchItem)> {
    let title_run = card.pointer("/title/runs/0")?;
    let title = title_run.get("text")?.as_str()?.to_owned();
    let browse_id = title_run
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str);
    let video_id = card
        .pointer("/onTap/watchEndpoint/videoId")
        .or_else(|| title_run.pointer("/navigationEndpoint/watchEndpoint/videoId"))
        .and_then(Value::as_str);
    let kind = match kind {
        SearchKind::Unknown if browse_id.is_some_and(|id| id.starts_with("UC")) => {
            SearchKind::Artist
        }
        SearchKind::Unknown if browse_id.is_some_and(|id| id.starts_with("MPRE")) => {
            SearchKind::Album
        }
        SearchKind::Unknown if video_id.is_some() => SearchKind::Song,
        kind => kind,
    };
    let id = match kind {
        SearchKind::Artist | SearchKind::Album | SearchKind::Playlist => browse_id?,
        SearchKind::Song | SearchKind::Video => video_id?,
        SearchKind::Unknown => return None,
    };
    let thumbs = card
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail")
        .or_else(|| card.get("thumbnail"))
        .map(thumbnails)
        .unwrap_or_default();
    let columns = vec![
        json!({}),
        json!({"musicResponsiveListItemFlexColumnRenderer":{"text":card.get("subtitle").cloned().unwrap_or(Value::Null)}}),
    ];
    let (artists, album, duration) = track_metadata(card, &columns);
    let key = format!("{kind:?}:{id}");
    let item = match kind {
        SearchKind::Artist => SearchItem::Artist(MusicArtist {
            id: MusicId(id.to_owned()),
            name: title,
            thumbnails: thumbs,
            source: SOURCE.to_owned(),
        }),
        SearchKind::Album => SearchItem::Album(MusicAlbum {
            id: MusicId(id.to_owned()),
            title,
            artists: Vec::new(),
            year: None,
            thumbnails: thumbs,
            source: SOURCE.to_owned(),
        }),
        SearchKind::Playlist => SearchItem::Playlist(MusicPlaylist {
            id: MusicId(id.strip_prefix("VL").unwrap_or(id).to_owned()),
            title,
            author: None,
            thumbnails: thumbs,
            source: SOURCE.to_owned(),
        }),
        SearchKind::Song | SearchKind::Video => SearchItem::Track(MusicTrack {
            id: MusicId(id.to_owned()),
            video_id: Some(id.to_owned()),
            title,
            artists,
            album,
            duration,
            thumbnails: thumbs,
            is_explicit: None,
            source: SOURCE.to_owned(),
        }),
        SearchKind::Unknown => return None,
    };
    Some((key, item))
}

// Catalog columns vary between filtered Songs and mixed/top-result shelves.
// Use navigation identities rather than treating album names, clocks or view
// counts in the subtitle as additional artists.
fn track_metadata(
    renderer: &Value,
    columns: &[Value],
) -> (Vec<Artist>, Option<MusicAlbumRef>, Option<String>) {
    let runs: Vec<&Value> = columns
        .iter()
        .skip(1)
        .filter_map(|column| {
            column
                .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                .and_then(Value::as_array)
        })
        .flatten()
        .collect();
    let mut artists = Vec::new();
    let mut album = None;
    for run in &runs {
        let Some(name) = run
            .get("text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let id = run
            .pointer("/navigationEndpoint/browseEndpoint/browseId")
            .and_then(Value::as_str);
        if let Some(id) = id.filter(|id| id.starts_with("UC") || id.starts_with("MPLA")) {
            if !artists.iter().any(|artist: &Artist| artist.name == name) {
                artists.push(Artist {
                    name: name.to_owned(),
                    channel_id: Some(id.to_owned()),
                });
            }
        } else if let Some(id) = id.filter(|id| id.starts_with("MPRE")) {
            album = Some(MusicAlbumRef {
                id: Some(MusicId(id.to_owned())),
                title: name.to_owned(),
            });
        }
    }
    let duration = renderer
        .pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text")
        .and_then(text)
        .filter(|value| is_clock(value))
        .or_else(|| {
            runs.iter()
                .rev()
                .filter_map(|run| run.get("text").and_then(Value::as_str))
                .find(|value| is_clock(value))
                .map(|value| value.trim().to_owned())
        });
    if artists.is_empty() {
        if let Some(subtitle) = columns
            .get(1)
            .and_then(|column| column.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
            .and_then(text)
        {
            if let Some(name) = subtitle.split(['•', '·']).map(str::trim).find(|name| {
                !name.is_empty()
                    && !is_clock(name)
                    && !matches!(
                        name.to_ascii_lowercase().as_str(),
                        "song" | "video" | "single" | "album"
                    )
            }) {
                artists.push(Artist {
                    name: name.to_owned(),
                    channel_id: None,
                });
            }
        }
    }
    if album.is_none() {
        album = columns
            .get(2)
            .and_then(|column| column.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
            .and_then(text)
            .filter(|title| !title.trim().is_empty() && !is_clock(title) && !is_statistic(title))
            .map(|title| MusicAlbumRef { id: None, title });
    }
    (artists, album, duration)
}

fn is_statistic(value: &str) -> bool {
    let lower = value.trim().to_lowercase();
    lower.chars().next().is_some_and(char::is_numeric)
        && [
            "view",
            "play",
            "listener",
            "subscriber",
            "перегляд",
            "просмотр",
        ]
        .iter()
        .any(|unit| lower.contains(unit))
}

fn is_clock(value: &str) -> bool {
    let parts: Vec<_> = value.trim().split(':').collect();
    (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn parse_search_renderer(renderer: &Value, shelf_kind: SearchKind) -> Option<(String, SearchItem)> {
    if renderer
        .get("musicItemRendererDisplayPolicy")
        .and_then(Value::as_str)
        == Some("MUSIC_ITEM_RENDERER_DISPLAY_POLICY_GREY_OUT")
    {
        return None;
    }
    let columns = renderer.get("flexColumns")?.as_array()?;
    let first = columns
        .first()?
        .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs/0")?;
    let title = first.get("text").and_then(Value::as_str)?.to_owned();
    let browse_id = first
        .pointer("/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str);
    let video_id = renderer.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId")
        .or_else(|| first.pointer("/navigationEndpoint/watchEndpoint/videoId"))
        .and_then(Value::as_str);
    let video_type = renderer.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        .or_else(|| renderer.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoType"))
        .and_then(Value::as_str).unwrap_or("");
    let kind = match shelf_kind {
        SearchKind::Unknown if browse_id.is_some_and(|id| id.starts_with("UC")) => {
            SearchKind::Artist
        }
        SearchKind::Unknown if browse_id.is_some_and(|id| id.starts_with("MPRE")) => {
            SearchKind::Album
        }
        SearchKind::Unknown
            if browse_id.is_some_and(|id| id.starts_with("VL") || id.starts_with("PL")) =>
        {
            SearchKind::Playlist
        }
        SearchKind::Unknown if video_id.is_some() => {
            if matches!(video_type, "MUSIC_VIDEO_TYPE_OMV" | "MUSIC_VIDEO_TYPE_UGC") {
                SearchKind::Video
            } else {
                SearchKind::Song
            }
        }
        kind => kind,
    };
    let entity_id = if matches!(kind, SearchKind::Song | SearchKind::Video) {
        video_id?
    } else {
        browse_id?
    };
    let thumb = renderer
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail")
        .map(thumbnails)
        .unwrap_or_default();
    let key = format!("{kind:?}:{entity_id}");
    let item = match kind {
        SearchKind::Song | SearchKind::Video => {
            let (artists, album, duration) = track_metadata(renderer, columns);
            let track = MusicTrack {
                id: MusicId(entity_id.to_owned()),
                video_id: Some(entity_id.to_owned()),
                title,
                artists,
                album,
                duration,
                thumbnails: thumb,
                is_explicit: Some(renderer.get("badges").is_some()),
                source: SOURCE.to_owned(),
            };
            if matches!(kind, SearchKind::Video) {
                SearchItem::Video(track)
            } else {
                SearchItem::Track(track)
            }
        }
        SearchKind::Artist => SearchItem::Artist(MusicArtist {
            id: MusicId(entity_id.to_owned()),
            name: title,
            thumbnails: thumb,
            source: SOURCE.to_owned(),
        }),
        SearchKind::Album => SearchItem::Album(MusicAlbum {
            id: MusicId(entity_id.to_owned()),
            title,
            artists: Vec::new(),
            year: None,
            thumbnails: thumb,
            source: SOURCE.to_owned(),
        }),
        SearchKind::Playlist => SearchItem::Playlist(MusicPlaylist {
            id: MusicId(entity_id.strip_prefix("VL").unwrap_or(entity_id).to_owned()),
            title,
            author: None,
            thumbnails: thumb,
            source: SOURCE.to_owned(),
        }),
        SearchKind::Unknown => return None,
    };
    Some((key, item))
}

#[derive(Debug)]
enum SearchItem {
    Track(MusicTrack),
    Video(MusicTrack),
    Album(MusicAlbum),
    Artist(MusicArtist),
    Playlist(MusicPlaylist),
}

fn push_search_item(page: &mut MusicSearchPage, item: SearchItem) {
    match item {
        SearchItem::Track(track) => page.songs.push(track),
        SearchItem::Video(video) => page.videos.push(video),
        SearchItem::Album(album) => page.albums.push(album),
        SearchItem::Artist(artist) => page.artists.push(artist),
        SearchItem::Playlist(playlist) => page.playlists.push(playlist),
    }
}

/// Decode suggestions without requiring a provider client or session credentials.
pub fn parse_suggestions(response: &Value) -> Vec<Suggestion> {
    let Some(items) = response
        .pointer("/contents/0/searchSuggestionsSectionRenderer/contents")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .filter_map(|item| {
            let renderer = item
                .get("searchSuggestionRenderer")
                .or_else(|| item.get("historySuggestionRenderer"))?;
            let suggestion = renderer
                .pointer("/navigationEndpoint/searchEndpoint/query")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    renderer
                        .pointer("/suggestion/runs")
                        .and_then(Value::as_array)
                        .map(|runs| {
                            runs.iter()
                                .filter_map(|r| r.get("text").and_then(Value::as_str))
                                .collect::<String>()
                        })
                })?;
            if seen.insert(suggestion.clone()) {
                Some(Suggestion {
                    text: suggestion,
                    source: SOURCE.to_owned(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn parse_chart(response: &Value, region: &str) -> Result<MusicChartPage> {
    let sections = response.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .or_else(|| response.pointer("/contents/sectionListRenderer/contents"))
        .and_then(Value::as_array).ok_or_else(|| Error::Provider { status: 200, message: "unrecognized Music chart response layout".to_owned() })?;
    let mut tracks = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut title = None;
    for section in sections {
        if let Some(shelf) = section.get("musicShelfRenderer") {
            title = title.or_else(|| shelf.get("title").and_then(text));
            for item in shelf
                .get("contents")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(renderer) = item.get("musicResponsiveListItemRenderer") else {
                    continue;
                };
                if let Some((key, SearchItem::Track(track))) =
                    parse_search_renderer(renderer, SearchKind::Song)
                {
                    if seen.insert(key) {
                        tracks.push(track);
                    }
                }
            }
        }
    }
    Ok(MusicChartPage {
        tracks,
        region: region.to_ascii_uppercase(),
        title,
        continuation: None,
        source: SOURCE.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_track_and_search_results() {
        let response = json!({"contents":{"tabbedSearchResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[
            {"musicShelfRenderer":{"title":{"runs":[{"text":"Songs"}]},"contents":[
                {"musicResponsiveListItemRenderer":{"flexColumns":[
                    {"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Tune","navigationEndpoint":{"watchEndpoint":{"videoId":"v1"}}}]}}},
                    {"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Singer","navigationEndpoint":{"browseEndpoint":{"browseId":"UC1"}}},{"text":" • "},{"text":"3:20"}]}}},
                    {"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"LP","navigationEndpoint":{"browseEndpoint":{"browseId":"MPRE1"}}}]}}}
                ]},"thumbnail":{"musicThumbnailRenderer":{"thumbnail":{"thumbnails":[{"url":"https://img.example/art","width":512}]}}},"overlay":{"musicItemThumbnailOverlayRenderer":{"content":{"musicPlayButtonRenderer":{"playNavigationEndpoint":{"watchEndpoint":{"videoId":"v1"},"watchEndpointMusicSupportedConfigs":{"watchEndpointMusicConfig":{"musicVideoType":"MUSIC_VIDEO_TYPE_ATV"}}}}}}}},
                {"musicResponsiveListItemRenderer":{"flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Tune","navigationEndpoint":{"watchEndpoint":{"videoId":"v1"}}}]}}}]}}
            ],"continuations":[{"nextContinuationData":{"continuation":"next-token"}}]}},
            {"musicShelfRenderer":{"title":{"runs":[{"text":"Artists"}]},"contents":[{"musicResponsiveListItemRenderer":{"flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Band","navigationEndpoint":{"browseEndpoint":{"browseId":"UC2"}}}]}}}],"thumbnail":{"musicThumbnailRenderer":{"thumbnail":{"thumbnails":[]}}}}}]}},
            {"musicShelfRenderer":{"title":{"runs":[{"text":"Albums"}]},"contents":[{"musicResponsiveListItemRenderer":{"flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Record","navigationEndpoint":{"browseEndpoint":{"browseId":"MPRE2"}}}]}}}]}}]}}
        ]}}}}]}}});
        let page = parse_search(&response).unwrap();
        assert_eq!(page.songs.len(), 1);
        assert_eq!(page.songs[0].id.0, "v1");
        assert_eq!(page.songs[0].album.as_ref().unwrap().title, "LP");
        assert_eq!(page.songs[0].artists[0].channel_id.as_deref(), Some("UC1"));
        assert_eq!(page.artists[0].name, "Band");
        assert_eq!(page.albums[0].title, "Record");
        assert_eq!(page.continuation.as_deref(), Some("next-token"));
    }

    #[test]
    fn cookie_debug_is_redacted_and_headers_reject_injection() {
        let cookie = MusicSession::new("SAPISID=secret").unwrap();
        assert!(!format!("{cookie:?}").contains("secret"));
        assert!(MusicSession::new("a=b\r\nInjected: yes").is_err());
        let config = YouTubeMusicConfig::with_locale("api-secret", "us", "EN-US")
            .unwrap()
            .with_session(cookie);
        assert!(!format!("{config:?}").contains("api-secret"));
        assert!(!format!("{config:?}").contains("SAPISID"));
    }

    #[tokio::test]
    async fn sends_request_context_and_auth_only_to_music_origin() {
        use wiremock::{
            matchers::{body_json, header, method, path, query_param},
            Mock, MockServer, ResponseTemplate,
        };

        let server = MockServer::start().await;
        let config = YouTubeMusicConfig::with_locale("test-key", "gb", "EN-GB")
            .unwrap()
            .with_session(MusicSession::new("SAPISID=secret").unwrap());
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let mut provider = YouTubeMusicProvider::with_client(config, client).unwrap();
        provider.base_url = Url::parse(&format!("{}/youtubei/v1/", server.uri())).unwrap();

        Mock::given(method("POST"))
            .and(path("/youtubei/v1/search"))
            .and(query_param("key", "test-key"))
            .and(header("cookie", "SAPISID=secret"))
            .and(body_json(json!({
                "query": "night music",
                "params": search_filter_params(MusicSearchFilter::Songs).unwrap(),
                "context": { "client": {
                    "clientName": CLIENT_NAME,
                    "clientVersion": CLIENT_VERSION,
                    "hl": "en-gb",
                    "gl": "GB",
                    "platform": "DESKTOP"
                }}
            })))

            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"contents":{"tabbedSearchResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[]}}}}]}}})))
            .mount(&server)
            .await;

        let page = provider
            .search_page("night music", MusicSearchFilter::Songs, None)
            .await
            .unwrap();
        assert!(page.songs.is_empty());
    }

    #[tokio::test]
    async fn refuses_redirect_and_does_not_follow_it() {
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };

        let server = MockServer::start().await;
        let config = YouTubeMusicConfig::public("key").unwrap();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let mut provider = YouTubeMusicProvider::with_client(config, client).unwrap();
        provider.base_url = Url::parse(&format!("{}/youtubei/v1/", server.uri())).unwrap();
        Mock::given(method("POST"))
            .and(path("/youtubei/v1/music/get_search_suggestions"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://other.example/collect"),
            )
            .mount(&server)
            .await;
        assert!(matches!(
            provider.music_suggestions("query").await,
            Err(Error::Provider { status: 302, .. })
        ));
    }

    #[test]
    fn config_validates_and_normalizes_locale() {
        let config = YouTubeMusicConfig::with_locale("key", "us", "EN-us").unwrap();
        assert_eq!(config.country, "US");
        assert_eq!(config.language, "en-us");
        assert!(YouTubeMusicConfig::with_locale("key", "USA", "en").is_err());
    }

    #[test]
    fn parses_suggestions_in_service_order_and_deduplicates() {
        let response = json!({"contents":[{"searchSuggestionsSectionRenderer":{"contents":[
            {"searchSuggestionRenderer":{"suggestion":{"runs":[{"text":"zebra"}]},"navigationEndpoint":{"searchEndpoint":{"query":"zebra"}}}},
            {"historySuggestionRenderer":{"suggestion":{"runs":[{"text":"apple"}]},"navigationEndpoint":{"searchEndpoint":{"query":"apple"}}}},
            {"searchSuggestionRenderer":{"suggestion":{"runs":[{"text":"zebra"}]},"navigationEndpoint":{"searchEndpoint":{"query":"zebra"}}}}
        ]}}]});
        let suggestions = parse_suggestions(&response);
        assert_eq!(
            suggestions
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["zebra", "apple"]
        );
    }

    #[test]
    fn each_search_filter_has_distinct_server_params() {
        let filters = [
            MusicSearchFilter::Songs,
            MusicSearchFilter::Videos,
            MusicSearchFilter::Albums,
            MusicSearchFilter::Artists,
            MusicSearchFilter::Playlists,
            MusicSearchFilter::CommunityPlaylists,
        ];
        let values = filters.map(|filter| search_filter_params(filter).unwrap());
        let unique = values.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), filters.len());
    }
}
