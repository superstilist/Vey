#![cfg(not(target_arch = "wasm32"))]
use playervf::{native, services::*};
use std::{
    io::{Read, Write},
    time::Duration,
};

fn lyric_http_fixture(status: u16, body: String) -> (String, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut chunk = [0; 4096];
            let len = stream.read(&mut chunk).unwrap();
            if len == 0 {
                break;
            }
            request.extend_from_slice(&chunk[..len]);
            if let Some(end) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
                let size = headers
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + size {
                    break;
                }
            }
        }
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (base, server)
}

#[tokio::test]
async fn lyric_post_matches_webstorm_contract_and_keeps_401_distinct_from_not_found() {
    for status in [200, 401, 404, 204] {
        let (base, server) = lyric_http_fixture(
            status,
            if status == 200 {
                "[00:01.00]Hello".into()
            } else {
                String::new()
            },
        );
        let track = Track {
            title: "Song & +".into(),
            artist: "Artist".into(),
            album: "Record".into(),
            duration_secs: 132.,
            ..Default::default()
        };
        let form = playervf::lyrics_service::v2_lyrics_form(&track, "abcdefghijk", "fixture-token");
        let result = native::fetch_lyrics_get(form, Some(base)).await;
        match status {
            200 => assert_eq!(result.unwrap().as_deref(), Some("[00:01.00]Hello")),
            401 => assert_eq!(result.unwrap_err(), "v2/lyrics HTTP 401"),
            _ => assert!(result.unwrap().is_none()),
        }
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /v2/lyrics HTTP/1.1\r\n"));
        let (headers, body) = request.split_once("\r\n\r\n").unwrap();
        let headers = headers.to_ascii_lowercase();
        assert!(headers.contains("origin: https://music.youtube.com\r\n"));
        assert!(headers.contains("referer: https://music.youtube.com/\r\n"));
        assert!(headers.contains("content-type: application/x-www-form-urlencoded;charset=utf-8"));
        assert!(body.contains("alwaysFetchMetadata=false"));
        assert!(body.contains("videoId=abcdefghijk"));
        assert!(body.contains("song=Song+%26+%2B"));
        assert!(body.contains("token=fixture-token"));
    }
}

#[tokio::test]
async fn turnstile_post_uses_original_json_contract_and_accepts_both_response_fields() {
    for field in ["jwt", "token"] {
        let (base, server) = lyric_http_fixture(
            200,
            serde_json::json!({field:"fixture-issued-token"}).to_string(),
        );
        assert_eq!(
            native::verify_turnstile("fixture-challenge".into(), Some(base))
                .await
                .unwrap(),
            "fixture-issued-token"
        );
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /verify-turnstile HTTP/1.1\r\n"));
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(body).unwrap(),
            serde_json::json!({"token":"fixture-challenge"})
        );
    }
}

#[tokio::test]
async fn turnstile_exchange_rejects_missing_jwt_and_http_errors() {
    for (status, body) in [(200, "{}"), (403, "denied")] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 8192];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        assert!(
            native::verify_turnstile("fixture-challenge".into(), Some(base))
                .await
                .is_err()
        );
        server.join().unwrap();
    }
}

#[test]
fn backup_roundtrip_keeps_local_state_and_lyrics() {
    let mut store = Store::default();
    store.sources.push(Source {
        id: "a".into(),
        kind: "folder".into(),
        path: "C:/Music".into(),
    });
    let t = Track {
        path: "C:/Music/test.wav".into(),
        title: "Пісня".into(),
        ..Default::default()
    };
    store.record(&t);
    store.toggle_like(&t.path);
    store.offsets.insert(t.path.clone(), -300);
    let p = store.create_playlist("Мій список").unwrap();
    store.playlist_add(&p, &t.path);
    store.cache_lyrics(
        t.path.clone(),
        vec![playervf::lyrics::LyricsAlternative {
            provider: "fixture".into(),
            sync_type: playervf::lyrics::LyricsSyncType::Synced,
            format: "lrc".into(),
            lyrics: playervf::lyrics::parse_lrc("[00:01.00]Слова"),
        }],
    );
    let restored: Store = serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
    assert_eq!(restored.history.len(), 1);
    assert!(restored.likes.contains(&t.path));
    assert_eq!(restored.playlists[0].tracks, vec![t.path.clone()]);
    assert_eq!(restored.offset_for(&t), -300);
    assert_eq!(
        restored.cached_lyrics(&t.path).unwrap()[0].lyrics.lines[0].text,
        "Слова"
    );
}

#[test]
#[ignore = "requires a native output device; plays silent fixture audio"]
fn native_audio_play_pause_seek_rate_volume_and_failed_load() {
    let path =
        std::env::temp_dir().join(format!("playervf-audio-smoke-{}.wav", std::process::id()));
    let samples = vec![0u8; 32000];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8000u32.to_le_bytes());
    bytes.extend_from_slice(&16000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    bytes.extend(samples);
    std::fs::write(&path, bytes).unwrap();
    let audio = native::start_audio();
    native::player_load(&audio, path.to_string_lossy().into(), false).unwrap();
    std::thread::sleep(Duration::from_millis(180));
    assert_eq!(
        native::player_status(&audio).unwrap().state,
        native::PlaybackState::Paused
    );
    native::player_seek(&audio, 0.5).unwrap();
    native::player_volume(&audio, 0.25).unwrap();
    native::player_rate(&audio, 1.5).unwrap();
    native::player_play(&audio).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let s = native::player_status(&audio).unwrap();
    assert_eq!(s.state, native::PlaybackState::Playing);
    assert!(s.position_secs >= 0.5);
    assert_eq!(s.volume, 0.25);
    assert_eq!(s.rate, 1.5);
    assert!(native::player_load(&audio, "missing-test-fixture.flac".into(), true).is_err());
    assert_eq!(
        native::player_status(&audio)
            .unwrap()
            .current_path
            .as_deref(),
        Some(path.to_string_lossy().as_ref())
    );
    native::player_pause(&audio).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        native::player_status(&audio).unwrap().state,
        native::PlaybackState::Paused
    );
    native::player_stop(&audio).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        native::player_status(&audio).unwrap().state,
        native::PlaybackState::Stopped
    );
    drop(audio);
    let _ = std::fs::remove_file(path);
}
