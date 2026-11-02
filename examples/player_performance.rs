//! Same-process controller microbenchmarks; no network, user data, audio or FPS claims.
use playervf::{
    services::{Playlist, Store, Track},
    track_model::TrackModel,
    TrackRow,
};
use slint::{Model, VecModel};
use std::{hint::black_box, time::Instant};

fn measure(mut f: impl FnMut()) -> f64 {
    let mut times = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        f();
        times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    times.sort_by(f64::total_cmp);
    times[2]
}
fn main() {
    let tracks: Vec<_> = (0..10_000)
        .map(|i| Track {
            path: format!("fixture://{i}"),
            title: format!("Song {i}"),
            artist: "Fixture artist".into(),
            album: "Fixture album".into(),
            duration_secs: 180.,
            ..Default::default()
        })
        .collect();
    let batch_old = measure(|| {
        let mut store = Store::default();
        for track in &tracks {
            store.remember(track.clone());
        }
        black_box(store);
    });
    let batch_new = measure(|| {
        let mut store = Store::default();
        store.remember_all(tracks.clone());
        black_box(store);
    });
    let mut store = Store {
        tracks: tracks.clone(),
        ..Default::default()
    };
    store.playlists.push(Playlist {
        id: "fixture".into(),
        name: "Fixture".into(),
        tracks: (0..2_000)
            .map(|i| tracks[(i * 7919) % tracks.len()].path.clone())
            .collect(),
        ..Default::default()
    });
    let playlist_old = measure(|| {
        let found: Vec<_> = store.playlists[0]
            .tracks
            .iter()
            .filter_map(|path| store.tracks.iter().find(|t| &t.path == path))
            .cloned()
            .collect();
        black_box(found);
    });
    let playlist_new = measure(|| {
        black_box(store.playlist_tracks("fixture"));
    });
    let rows: Vec<_> = tracks
        .iter()
        .map(|t| TrackRow {
            path: t.path.clone().into(),
            title: t.title.clone().into(),
            artist: t.artist.clone().into(),
            ..Default::default()
        })
        .collect();
    let legacy = VecModel::from(rows.clone());
    let indexed = TrackModel::new(rows);
    let paths: Vec<_> = (0..400)
        .map(|i| tracks[(i * 7919) % tracks.len()].path.clone())
        .collect();
    let image = slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
        &[255u8, 0, 0, 255],
        1,
        1,
    ));
    let covers_old = measure(|| {
        for path in &paths {
            for i in 0..legacy.row_count() {
                let mut row = legacy.row_data(i).unwrap();
                if row.path.as_str() == path && row.art != image {
                    row.art = image.clone();
                    legacy.set_row_data(i, row);
                }
            }
        }
    });
    let covers_new = measure(|| {
        for path in &paths {
            indexed.update_path(path, |row| row.art = image.clone());
        }
    });
    let likes_old = measure(|| {
        for path in paths.iter().take(100) {
            let rebuilt: Vec<_> = legacy
                .iter()
                .map(|mut row| {
                    if row.path.as_str() == path {
                        row.liked = !row.liked;
                    }
                    row
                })
                .collect();
            legacy.set_vec(rebuilt);
        }
    });
    let likes_new = measure(|| {
        for path in paths.iter().take(100) {
            indexed.update_path(path, |row| row.liked = !row.liked);
        }
    });
    println!(
        "{}",
        serde_json::json!({
            "workload": {"tracks": 10000, "playlist": 2000, "cover_events": 400, "like_events": 100, "samples": 5},
            "median_ms": {
                "catalog_merge": {"before": batch_old, "after": batch_new},
                "playlist_lookup": {"before": playlist_old, "after": playlist_new},
                "artwork_dispatch": {"before": covers_old, "after": covers_new},
                "like_rows": {"before": likes_old, "after": likes_new}
            }
        })
    );
}
