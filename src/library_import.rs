//! Import identities and conservative, conflict-aware playlist updates.
use crate::services::{Playlist, Store, Track};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Youtube,
    Spotify,
}
impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Self::Youtube => "YouTube Music",
            Self::Spotify => "Spotify",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Youtube => "youtube",
            Self::Spotify => "spotify",
        }
    }
}

/// Only accept provider URLs/URIs, never fetch a user-supplied host.
pub fn playlist_link(input: &str) -> Result<(Provider, String), String> {
    let input = input.trim();
    if let Some(id) = input.strip_prefix("spotify:playlist:") {
        return validate_id(Provider::Spotify, id);
    }
    let url = reqwest::Url::parse(input)
        .map_err(|_| "Вставте посилання на плейліст YouTube Music або Spotify")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("Потрібне HTTPS-посилання без облікових даних".into());
    }
    match url.host_str().unwrap_or_default() {
        "music.youtube.com" | "www.youtube.com" | "youtube.com" if url.path() == "/playlist" => {
            let id = url
                .query_pairs()
                .find(|(k, _)| k == "list")
                .map(|(_, v)| v.into_owned())
                .ok_or("Немає ID плейліста")?;
            validate_id(Provider::Youtube, &id)
        }
        "open.spotify.com" => {
            let parts: Vec<_> = url
                .path_segments()
                .into_iter()
                .flatten()
                .filter(|p| !p.is_empty())
                .collect();
            let parts = if parts.first().is_some_and(|s| s.starts_with("intl-")) {
                &parts[1..]
            } else {
                &parts[..]
            };
            if parts.len() != 2 || parts[0] != "playlist" {
                return Err("Потрібне посилання на плейліст".into());
            }
            validate_id(Provider::Spotify, parts[1])
        }
        _ => Err("Непідтримуване посилання на плейліст".into()),
    }
}
fn validate_id(provider: Provider, id: &str) -> Result<(Provider, String), String> {
    let valid = match provider {
        Provider::Spotify => id.len() == 22 && id.bytes().all(|b| b.is_ascii_alphanumeric()),
        Provider::Youtube => {
            (id == "LM" || (10..=63).contains(&id.len()))
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        }
    };
    if valid {
        Ok((provider, id.into()))
    } else {
        Err("Некоректний ID плейліста".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedPlaylist {
    pub id: String,
    pub name: String,
    pub tracks: Vec<Track>,
}
pub fn local_id(provider: Provider, owner: &str, id: &str) -> String {
    if provider == Provider::Youtube && id != "LM" {
        return format!("youtube-{id}");
    }
    // Account-specific identities prevent switching accounts from overwriting a library.
    format!(
        "{}-{:08x}-{}",
        provider.key(),
        u32::from_be_bytes(Sha256::digest(owner.as_bytes())[..4].try_into().unwrap()),
        id
    )
}
pub fn fingerprint(p: &Playlist) -> String {
    let value = serde_json::to_vec(&(&p.name, &p.tracks)).unwrap_or_default();
    format!("{:x}", Sha256::digest(value))
}
/// A first import never replaces an existing playlist. Later pulls require the
/// last accepted fingerprint. Remote removals never delete local playlists.
pub fn apply(
    store: &mut Store,
    incoming: &ImportedPlaylist,
    baseline: Option<&str>,
) -> Result<String, String> {
    if incoming.id.len() > 80 || incoming.name.trim().is_empty() {
        return Err("Некоректні дані плейліста".into());
    }
    if !store.playlists.iter().any(|p| p.id == incoming.id) && store.playlists.len() >= 500 {
        return Err("Ліміт бібліотеки — 500 плейлістів".into());
    }
    let keys: std::collections::HashSet<_> = store
        .tracks
        .iter()
        .map(|t| &t.path)
        .chain(incoming.tracks.iter().map(|t| &t.path))
        .filter(|p| p.starts_with("yt://") || p.starts_with("cloud://"))
        .collect();
    if keys.len() > 20000 {
        return Err("Ліміт синхронізації — 20 000 композицій".into());
    }
    if let Some(p) = store.playlists.iter().find(|p| p.id == incoming.id) {
        let current = fingerprint(p);
        let same =
            p.name == incoming.name && p.tracks.iter().eq(incoming.tracks.iter().map(|t| &t.path));
        if !same && baseline != Some(current.as_str()) {
            return Err(format!(
                "{}: локальні зміни збережено. Створіть копію або імпортуйте після узгодження.",
                p.name
            ));
        }
    }
    store.remember_all(incoming.tracks.clone());
    let playlist = Playlist {
        id: incoming.id.clone(),
        name: incoming.name.chars().take(160).collect(),
        tracks: incoming.tracks.iter().map(|t| t.path.clone()).collect(),
        updated_at: crate::services::now_ms(),
        ..Default::default()
    };
    let hash = fingerprint(&playlist);
    if let Some(p) = store.playlists.iter_mut().find(|p| p.id == incoming.id) {
        *p = playlist;
    } else {
        store.playlists.push(playlist);
    }
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_reject_foreign_hosts_credentials_and_wrong_resources() {
        for link in [
            "https://open.spotify.com.evil/playlist/123",
            "https://u@music.youtube.com/playlist?list=PL1234567890",
            "http://youtube.com/playlist?list=PL1234567890",
            "https://open.spotify.com/track/0123456789012345678901",
        ] {
            assert!(playlist_link(link).is_err(), "{link}");
        }
        assert_eq!(
            playlist_link("https://music.youtube.com/playlist?list=PL1234567890&si=x")
                .unwrap()
                .1,
            "PL1234567890"
        );
        assert_eq!(
            playlist_link("spotify:playlist:0123456789012345678901")
                .unwrap()
                .0,
            Provider::Spotify
        );
        assert!(playlist_link(
            "https://open.spotify.com/intl-de/playlist/0123456789012345678901?si=x"
        )
        .is_ok());
    }
    #[test]
    fn reimport_preserves_order_repeats_and_local_edits() {
        assert!(playlist_link("https://music.youtube.com/playlist?list=LM").is_ok());
        assert_ne!(
            local_id(Provider::Youtube, "account-a", "LM"),
            local_id(Provider::Youtube, "account-b", "LM")
        );
        let mut s = Store::default();
        let track = Track {
            path: "yt://abcdefghijk".into(),
            ..Default::default()
        };
        let mut p = ImportedPlaylist {
            id: "import".into(),
            name: "One".into(),
            tracks: vec![track.clone(), track],
        };
        let first = apply(&mut s, &p, None).unwrap();
        assert_eq!(s.tracks.len(), 1);
        assert_eq!(s.playlists[0].tracks.len(), 2);
        p.name = "Two".into();
        assert!(apply(&mut s, &p, None).is_err());
        apply(&mut s, &p, Some(&first)).unwrap();
        s.playlists[0].name = "Local edit".into();
        let before = serde_json::to_string(&s).unwrap();
        assert!(apply(&mut s, &p, Some(&first)).is_err());
        assert_eq!(before, serde_json::to_string(&s).unwrap());
        assert_ne!(
            local_id(Provider::Spotify, "a", "p"),
            local_id(Provider::Spotify, "b", "p")
        );
    }
}
