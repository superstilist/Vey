use yt_rust::{recommendations::recommend, Artist, PlaybackHandoff, Thumbnail, Track};

fn track(id: &str, title: &str, artists: &[&str]) -> Track {
    Track {
        video_id: id.to_owned(),
        title: title.to_owned(),
        artists: artists
            .iter()
            .map(|name| Artist {
                name: (*name).to_owned(),
                channel_id: None,
            })
            .collect(),
        album: None,
        description: None,
        duration: None,
        thumbnails: vec![Thumbnail {
            url: "https://example.test/thumb.jpg".into(),
            width: Some(320),
            height: Some(180),
        }],
        channel_id: None,
        published_at: None,
        source: "fixture".to_owned(),
    }
}

#[test]
fn watch_and_embed_urls_are_created_for_player_handoff() {
    let song = track("vid123", "Song", &["Artist"]);
    assert_eq!(song.watch_url(), "https://www.youtube.com/watch?v=vid123");
    assert_eq!(
        song.embed_url(),
        "https://www.youtube-nocookie.com/embed/vid123"
    );
    let handoff = PlaybackHandoff::from(&song);
    assert_eq!(handoff.video_id, "vid123");
    assert_eq!(handoff.watch_url, song.watch_url());
}

#[test]
fn local_recommendations_prioritize_artists_from_history_and_exclude_played() {
    let candidates = vec![
        track("played", "Already played", &["A"]),
        track("other", "Other artist", &["B"]),
        track("related", "Related artist", &["A"]),
    ];
    let result = recommend(&candidates, &["played".to_owned()], 10);
    assert_eq!(
        result
            .iter()
            .map(|song| song.video_id.as_str())
            .collect::<Vec<_>>(),
        ["related", "other"]
    );
}

#[test]
fn recommendations_respect_limit_and_empty_history() {
    let candidates = vec![track("one", "One", &["A"]), track("two", "Two", &["B"])];
    assert!(recommend(&candidates, &[], 0).is_empty());
    assert_eq!(recommend(&candidates, &[], 1)[0].video_id, "one");
}
