use serde::{Deserialize, Serialize};

/// A provider-neutral video/song result. Music-specific fields are optional
/// because the official YouTube Data API does not guarantee them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub video_id: String,
    pub title: String,
    pub artists: Vec<Artist>,
    pub album: Option<String>,
    pub description: Option<String>,
    pub duration: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
    pub channel_id: Option<String>,
    pub published_at: Option<String>,
    pub source: String,
}

impl Track {
    /// Canonical YouTube watch URL used for supported player handoff.
    #[must_use]
    pub fn watch_url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.video_id)
    }

    /// Embed URL for a YouTube player. Playback restrictions are enforced by YouTube.
    #[must_use]
    pub fn embed_url(&self) -> String {
        format!("https://www.youtube-nocookie.com/embed/{}", self.video_id)
    }
}

/// A YouTube Music catalog identifier. IDs are provider-specific and should not
/// be treated as YouTube Data API video IDs unless a matching video ID exists.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MusicId(pub String);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicSearchFilter {
    #[default]
    Songs,
    Videos,
    Albums,
    Artists,
    Playlists,
    CommunityPlaylists,
}

/// A normalized YouTube Music search response. Results are intentionally split
/// by entity type rather than pretending artists and albums are tracks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MusicSearchPage {
    pub songs: Vec<MusicTrack>,
    pub videos: Vec<MusicTrack>,
    pub albums: Vec<MusicAlbum>,
    pub artists: Vec<MusicArtist>,
    pub playlists: Vec<MusicPlaylist>,
    pub continuation: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicTrack {
    pub id: MusicId,
    pub video_id: Option<String>,
    pub title: String,
    pub artists: Vec<Artist>,
    pub album: Option<MusicAlbumRef>,
    pub duration: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
    pub is_explicit: Option<bool>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicAlbumRef {
    pub id: Option<MusicId>,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicAlbum {
    pub id: MusicId,
    pub title: String,
    pub artists: Vec<Artist>,
    pub year: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicArtist {
    pub id: MusicId,
    pub name: String,
    pub thumbnails: Vec<Thumbnail>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicPlaylist {
    pub id: MusicId,
    pub title: String,
    pub author: Option<String>,
    pub thumbnails: Vec<Thumbnail>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicArtistDetails {
    pub artist: MusicArtist,
    pub description: Option<String>,
    pub top_tracks: Vec<MusicTrack>,
    pub albums: Vec<MusicAlbum>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicAlbumDetails {
    pub album: MusicAlbum,
    pub tracks: Vec<MusicTrack>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicChartPage {
    pub tracks: Vec<MusicTrack>,
    pub region: String,
    pub title: Option<String>,
    pub continuation: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    pub name: String,
    pub channel_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thumbnail {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchPage {
    pub items: Vec<Track>,
    pub next_page_token: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    pub text: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChartPage {
    pub items: Vec<Track>,
    pub region: Option<String>,
    /// True for YouTube's general popular-video chart, not a Music chart.
    pub is_music_chart: bool,
    pub source: String,
}

/// Playback information intentionally contains no direct media/audio URL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackHandoff {
    pub video_id: String,
    pub watch_url: String,
    pub embed_url: String,
}

impl From<&Track> for PlaybackHandoff {
    fn from(track: &Track) -> Self {
        Self {
            video_id: track.video_id.clone(),
            watch_url: track.watch_url(),
            embed_url: track.embed_url(),
        }
    }
}
