//! Same-origin host for the canvas/WASM app. Public YTM only; no session APIs.
use crate::{lyrics_service, native, services::Track};
use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use std::collections::HashMap;
type ApiError = (StatusCode, String);
fn error(e: String) -> ApiError {
    (StatusCode::BAD_GATEWAY, e)
}
fn from_song(s: native::YtmSong) -> Track {
    Track {
        path: format!("yt://{}", s.video_id),
        title: s.title,
        artist: s.artist,
        album: s.album,
        duration_secs: s.duration_secs as f64,
        video_id: Some(s.video_id),
        thumbnail_url: s.thumbnail_url,
    }
}
async fn search(Query(q): Query<HashMap<String, String>>) -> Result<Json<Vec<Track>>, ApiError> {
    let query = q.get("q").cloned().unwrap_or_default();
    if query.trim().is_empty() {
        return Ok(Json(vec![]));
    }
    native::ytm_search_songs(query, Some(40))
        .await
        .map(|rows| {
            Json(
                rows.into_iter()
                    .filter(|s| {
                        !s.video_id.is_empty()
                            && ["song", "video", "top"].contains(&s.kind.as_str())
                    })
                    .map(from_song)
                    .collect(),
            )
        })
        .map_err(error)
}
async fn browse(Query(q): Query<HashMap<String, String>>) -> Result<Json<Vec<Track>>, ApiError> {
    let rows = if q.contains_key("radio") {
        native::ytm_recommendations(q.get("v").cloned(), None, Some(40)).await
    } else {
        native::ytm_browse(q.get("id").cloned(), Some(40)).await
    }
    .map_err(error)?;
    Ok(Json(
        rows.into_iter()
            .filter(|s| !s.video_id.is_empty())
            .map(|s| Track {
                path: format!("yt://{}", s.video_id),
                title: s.title,
                artist: s.artist,
                album: s.album,
                duration_secs: s.duration_secs as f64,
                video_id: Some(s.video_id),
                thumbnail_url: s.thumbnail_url,
            })
            .collect(),
    ))
}
async fn audio(Path(id): Path<String>, request: axum::extract::Request) -> Response {
    match native::ytm_cache_stream(id).await {
        Ok(Some(track)) => match tower_http::services::ServeFile::new(track.path)
            .try_call(request)
            .await
        {
            Ok(r) => r.into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, "Audio unavailable").into_response(),
        Err(e) => error(e).into_response(),
    }
}
async fn lyrics(
    Query(query): Query<HashMap<String, String>>,
    Json(track): Json<Track>,
) -> Result<Json<Vec<crate::lyrics::LyricsAlternative>>, ApiError> {
    let jwt = lyrics_service::JwtCache::load().token();
    lyrics_service::fetch(
        &track,
        query.get("provider").map(String::as_str).unwrap_or("auto"),
        query.get("refresh").is_some_and(|v| v == "true"),
        jwt,
    )
    .await
    .map(Json)
    .map_err(error)
}
async fn verify(Json(body): Json<HashMap<String, String>>) -> Result<StatusCode, ApiError> {
    let token = body
        .get("token")
        .cloned()
        .ok_or((StatusCode::BAD_REQUEST, "Missing token".into()))?;
    let mut jwt = lyrics_service::JwtCache::load();
    jwt.exchange(token).await.map_err(error)?;
    Ok(StatusCode::NO_CONTENT)
}
pub fn run() -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        tokio::spawn(async {
            let _ = lyrics_service::prewarm_authentication().await;
        });
        let app = Router::new()
            .route("/api/search", get(search))
            .route("/api/browse", get(browse))
            .route("/api/audio/{id}", get(audio))
            .route("/api/lyrics", post(lyrics))
            .route("/api/verify", post(verify))
            .fallback_service(tower_http::services::ServeDir::new("web"));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:4173")
            .await
            .map_err(|e| e.to_string())?;
        println!("PlayerVF web: http://127.0.0.1:4173");
        axum::serve(listener, app).await.map_err(|e| e.to_string())
    })
}
