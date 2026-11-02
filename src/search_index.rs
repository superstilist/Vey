//! Reusable normalized local search; query execution allocates no metadata strings.
use crate::services::Track;
#[derive(Default)]
pub struct SearchIndex {
    entries: Vec<(Track, String)>,
}
impl SearchIndex {
    pub fn find(&mut self, tracks: &[Track], query: &str) -> Vec<Track> {
        // Refresh changed rows only. Metadata edits are rare; query typing is frequent.
        self.entries.resize_with(tracks.len(), Default::default);
        for ((identity, text), track) in self.entries.iter_mut().zip(tracks) {
            if identity.path != track.path
                || identity.title != track.title
                || identity.artist != track.artist
                || identity.album != track.album
            {
                *identity = track.clone();
                *text = format!("{} {} {}", track.title, track.artist, track.album).to_lowercase();
            }
        }
        let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
        tracks
            .iter()
            .zip(&self.entries)
            .filter(|(_, (_, text))| words.iter().all(|w| text.contains(w)))
            .take(500)
            .map(|(t, _)| t.clone())
            .collect()
    }
}

#[derive(Default)]
pub struct SearchCache {
    entries: std::collections::VecDeque<(String, web_time::Instant, Vec<Track>)>,
}
impl SearchCache {
    pub fn get(&self, query: &str) -> Option<Vec<Track>> {
        let key = query.trim().to_lowercase();
        self.entries
            .iter()
            .find(|(k, at, _)| k == &key && at.elapsed() < std::time::Duration::from_secs(300))
            .map(|(_, _, rows)| rows.clone())
    }
    pub fn put(&mut self, query: &str, tracks: Vec<Track>) {
        let key = query.trim().to_lowercase();
        self.entries.retain(|(k, _, _)| k != &key);
        if self.entries.len() >= 24 {
            self.entries.pop_front();
        }
        self.entries
            .push_back((key, web_time::Instant::now(), tracks));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_handles_reordered_terms_and_metadata_edits() {
        let mut index = SearchIndex::default();
        let mut songs = vec![Track {
            title: "Blue Bird".into(),
            artist: "Nick".into(),
            ..Default::default()
        }];
        assert_eq!(index.find(&songs, "NICK blue").len(), 1);
        songs[0].title = "Red Sun".into();
        assert!(index.find(&songs, "blue").is_empty());
        assert_eq!(index.find(&songs, "sun nick").len(), 1);
    }
}
