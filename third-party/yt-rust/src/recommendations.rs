//! Small deterministic recommendations over caller-supplied listening history.
//!
//! No history is retained by this module. Supply candidate tracks and an
//! ordered list of previously played video IDs on each call.

use std::collections::HashSet;

use crate::models::Track;

/// Rank candidates using simple artist/title affinity to the supplied history.
/// Excludes tracks already present in `listened_video_ids` and preserves input
/// order for equal scores.
#[must_use]
pub fn recommend<'a>(
    candidates: &'a [Track],
    listened_video_ids: &[String],
    limit: usize,
) -> Vec<&'a Track> {
    if limit == 0 {
        return Vec::new();
    }

    let listened: HashSet<&str> = listened_video_ids.iter().map(String::as_str).collect();
    let history_artists: HashSet<&str> = candidates
        .iter()
        .filter(|track| listened.contains(track.video_id.as_str()))
        .flat_map(|track| track.artists.iter().map(|artist| artist.name.as_str()))
        .collect();

    let mut ranked: Vec<(usize, usize, &Track)> = candidates
        .iter()
        .enumerate()
        .filter(|(_, track)| !listened.contains(track.video_id.as_str()))
        .map(|(index, track)| {
            let artist_match = track
                .artists
                .iter()
                .any(|artist| history_artists.contains(artist.name.as_str()));
            (usize::from(artist_match), usize::MAX - index, track)
        })
        .collect();

    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, _, track)| track)
        .collect()
}
