use crate::{
    models::{
        ChartPage, MusicAlbumDetails, MusicArtistDetails, MusicChartPage, MusicId,
        MusicSearchFilter, MusicSearchPage, MusicTrack, SearchPage, Suggestion, Track,
    },
    Result,
};

/// Capability boundary for discovery providers. Implementations return
/// `Unsupported` for capabilities they do not expose.
pub trait DiscoveryProvider: Send + Sync {
    fn provider_name(&self) -> &'static str;

    /// Search videos, optionally restricted to music-oriented results where supported.
    fn search(
        &self,
        query: &str,
        page_token: Option<&str>,
    ) -> impl std::future::Future<Output = Result<SearchPage>> + Send;

    /// Search query suggestions. The official YouTube Data API provider does
    /// not support this operation.
    fn suggestions(
        &self,
        _query: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Suggestion>>> + Send {
        async { Err(crate::Error::Unsupported("search suggestions")) }
    }

    /// Popular/trending results, where a provider exposes an appropriate chart.
    fn trending(
        &self,
        _region: Option<&str>,
    ) -> impl std::future::Future<Output = Result<ChartPage>> + Send {
        async { Err(crate::Error::Unsupported("trending chart")) }
    }

    /// Look up one video/song by its provider identifier.
    fn video(
        &self,
        _video_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Track>>> + Send {
        async { Err(crate::Error::Unsupported("video metadata lookup")) }
    }
}

/// YouTube Music catalog operations, separate from the general YouTube video API.
pub trait YouTubeMusicProvider: Send + Sync {
    fn music_search(
        &self,
        query: &str,
        filter: MusicSearchFilter,
        continuation: Option<&str>,
    ) -> impl std::future::Future<Output = Result<MusicSearchPage>> + Send;

    fn music_suggestions(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Suggestion>>> + Send;

    fn music_charts(
        &self,
        region: Option<&str>,
        continuation: Option<&str>,
    ) -> impl std::future::Future<Output = Result<MusicChartPage>> + Send;

    fn music_track(
        &self,
        id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Option<MusicTrack>>> + Send;

    fn music_album(
        &self,
        id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Option<MusicAlbumDetails>>> + Send;

    fn music_artist(
        &self,
        id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Option<MusicArtistDetails>>> + Send;

    /// Generic related tracks; this is not necessarily personalized.
    fn related_tracks(
        &self,
        id: &MusicId,
    ) -> impl std::future::Future<Output = Result<Vec<MusicTrack>>> + Send;
}
