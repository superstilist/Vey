//! Independent read-only provider connections: normal YouTube browser login and Spotify OAuth.
use crate::{
    library_import::{self as import, ImportedPlaylist, Provider},
    services::{now_ms, Store, Track},
    MainWindow,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const REDIRECT: &str = "http://127.0.0.1:43827/callback";
const MAX_PLAYLISTS: usize = 500;
const MAX_TRACKS: usize = 20000;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Connection {
    browser_target: String,
    auto: bool,
    client_id: String,
    client_secret: String,
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    owner: String,
    label: String,
    baselines: BTreeMap<String, String>,
    matches: BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Sessions {
    youtube: Connection,
    spotify: Connection,
}
impl Default for Sessions {
    fn default() -> Self {
        Self {
            youtube: Connection::default(),
            spotify: Connection::default(),
        }
    }
}
impl Sessions {
    fn connection(&self, p: Provider) -> &Connection {
        match p {
            Provider::Youtube => &self.youtube,
            Provider::Spotify => &self.spotify,
        }
    }
    fn connection_mut(&mut self, p: Provider) -> &mut Connection {
        match p {
            Provider::Youtube => &mut self.youtube,
            Provider::Spotify => &mut self.spotify,
        }
    }
}
fn load() -> Sessions {
    std::fs::read(crate::storage::data_dir().join("music-accounts.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn save(s: &Sessions) -> Result<(), String> {
    let dir = crate::storage::data_dir();
    std::fs::create_dir_all(&dir).map_err(|_| "Не вдалося створити сховище підключень")?;
    let path = dir.join("music-accounts.pending.json");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options
        .open(&path)
        .map_err(|_| "Не вдалося зберегти підключення")?;
    file.write_all(&serde_json::to_vec(s).map_err(|_| "Помилка формату підключення")?)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Не вдалося зберегти підключення")?;
    drop(file);
    std::fs::rename(path, dir.join("music-accounts.json"))
        .map_err(|_| "Не вдалося оновити підключення".into())
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Не вдалося створити HTTP-клієнт".into())
}
async fn json(response: Result<reqwest::Response, reqwest::Error>) -> Result<Value, String> {
    let response = response.map_err(|_| "Немає зв’язку з музичним сервісом. Спробуйте знову.")?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401=>"Сесію завершено. Підключіть акаунт знову.".into(),
            403=>"Сервіс заборонив доступ. Перевірте дозвіл OAuth, доступ до плейліста та обмеження застосунку.".into(),
            404=>"Плейліст недоступний або видалений.".into(),
            429=>format!("Забагато запитів. Повторіть через {} с.",response.headers().get("retry-after").and_then(|h|h.to_str().ok()).unwrap_or("60")),
            _=>format!("Музичний сервіс повернув HTTP {}.",status.as_u16()),
        });
    }
    // Do not include HTTP errors/URLs or provider bodies (which may contain tokens) in logs/UI.
    response
        .json()
        .await
        .map_err(|_| "Некоректна відповідь музичного сервісу".into())
}
fn token_url(p: Provider) -> &'static str {
    match p {
        Provider::Youtube => "https://oauth2.googleapis.com/token",
        Provider::Spotify => "https://accounts.spotify.com/api/token",
    }
}
fn secret(p: Provider, configured: &str) -> Option<String> {
    if p == Provider::Youtube {
        if !configured.is_empty() {
            return Some(configured.into());
        }
        std::env::var("VEY_GOOGLE_CLIENT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}
fn accept_token(c: &mut Connection, token: Value) -> Result<(), String> {
    c.access_token = token["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Сервіс не надав токен доступу")?
        .into();
    if let Some(refresh) = token["refresh_token"].as_str() {
        c.refresh_token = refresh.into();
    }
    c.expires_at = now_ms() + token["expires_in"].as_u64().unwrap_or(3600) * 1000;
    Ok(())
}
async fn refresh(p: Provider, c: &mut Connection, http: &reqwest::Client) -> Result<(), String> {
    if c.expires_at > now_ms() + 60000 && !c.access_token.is_empty() {
        return Ok(());
    }
    if c.refresh_token.is_empty() {
        return Err("Підключіть музичний акаунт".into());
    }
    let mut form = vec![
        ("grant_type", "refresh_token".to_owned()),
        ("refresh_token", c.refresh_token.clone()),
        ("client_id", c.client_id.clone()),
    ];
    if let Some(s) = secret(p, &c.client_secret) {
        form.push(("client_secret", s));
    }
    accept_token(
        c,
        json(http.post(token_url(p)).form(&form).send().await).await?,
    )
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Скасовано".into())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "android"))]
async fn authorize(
    p: Provider,
    client_id: String,
    client_secret: String,
    cancel: Arc<AtomicBool>,
) -> Result<Connection, String> {
    if client_id.trim().is_empty() {
        return Err("Вкажіть OAuth Client ID для цього сервісу".into());
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:43827")
        .map_err(|_| "Порт входу 43827 зайнятий. Завершіть попередній вхід і повторіть.")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "Не вдалося почати вхід")?;
    let mut random = [0u8; 64];
    rand::rng().fill_bytes(&mut random);
    let state = URL_SAFE_NO_PAD.encode(&random[..32]);
    let verifier = URL_SAFE_NO_PAD.encode(&random[32..]);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = reqwest::Url::parse(match p {
        Provider::Youtube => "https://accounts.google.com/o/oauth2/v2/auth",
        Provider::Spotify => "https://accounts.spotify.com/authorize",
    })
    .unwrap();
    url.query_pairs_mut().extend_pairs(&[
        ("client_id", client_id.as_str()),
        ("redirect_uri", REDIRECT),
        ("response_type", "code"),
        ("state", state.as_str()),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        (
            "scope",
            match p {
                Provider::Youtube => "https://www.googleapis.com/auth/youtube.readonly",
                Provider::Spotify => "playlist-read-private playlist-read-collaborative",
            },
        ),
    ]);
    if p == Provider::Youtube {
        url.query_pairs_mut()
            .extend_pairs(&[("access_type", "offline"), ("prompt", "consent")]);
    }
    crate::native::open_url(url.as_str())?;
    let code = tokio::task::spawn_blocking(move || wait_for_code(listener, &state, &cancel))
        .await
        .map_err(|_| "Помилка очікування входу")??;
    let http = client()?;
    let mut form = vec![
        ("client_id", client_id.clone()),
        ("grant_type", "authorization_code".into()),
        ("code", code),
        ("redirect_uri", REDIRECT.into()),
        ("code_verifier", verifier),
    ];
    if let Some(s) = secret(p, &client_secret) {
        form.push(("client_secret", s));
    }
    let mut connection = Connection {
        auto: true,
        client_id,
        client_secret,
        ..Default::default()
    };
    accept_token(
        &mut connection,
        json(http.post(token_url(p)).form(&form).send().await).await?,
    )?;
    let identity = match p {
        Provider::Spotify => {
            json(
                http.get("https://api.spotify.com/v1/me")
                    .bearer_auth(&connection.access_token)
                    .send()
                    .await,
            )
            .await?
        }
        Provider::Youtube => json(
            http.get("https://www.googleapis.com/youtube/v3/channels")
                .query(&[("part", "snippet"), ("mine", "true")])
                .bearer_auth(&connection.access_token)
                .send()
                .await,
        )
        .await?["items"][0]
            .clone(),
    };
    connection.owner = identity["id"]
        .as_str()
        .ok_or("Акаунт не має доступного профілю/каналу")?
        .into();
    connection.label = identity["display_name"]
        .as_str()
        .or_else(|| identity.pointer("/snippet/title").and_then(Value::as_str))
        .unwrap_or(p.name())
        .into();
    Ok(connection)
}
#[cfg(target_os = "android")]
async fn authorize(
    _: Provider,
    _: String,
    _: String,
    _: Arc<AtomicBool>,
) -> Result<Connection, String> {
    Err("Підключення музичних акаунтів доступне в настільній Vey. Імпортовані плейлісти можна відновити через акаунт Vey.".into())
}

#[cfg(not(target_os = "android"))]
fn wait_for_code(
    listener: std::net::TcpListener,
    state: &str,
    cancel: &AtomicBool,
) -> Result<String, String> {
    use std::io::{Read, Write};
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    while std::time::Instant::now() < deadline {
        cancelled(cancel)?;
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let mut bytes = vec![0; 16384];
                let mut used = 0;
                while used < bytes.len() {
                    match stream.read(&mut bytes[used..]) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => used += n,
                    }
                    if bytes[..used].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let line = String::from_utf8_lossy(&bytes[..used]);
                let target = line
                    .lines()
                    .next()
                    .and_then(|l| l.strip_prefix("GET "))
                    .and_then(|l| l.split_whitespace().next())
                    .unwrap_or("");
                let result = callback(target, state);
                let text = if result.is_ok() {
                    "Vey: sign-in received. You can close this tab."
                } else {
                    "Vey: invalid or denied sign-in. Return to Vey."
                };
                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",text.len(),text);
                match result {
                    Ok(code) => return Ok(code),
                    Err(e) if e == "Вхід відхилено" => return Err(e),
                    _ => {}
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(80))
            }
            Err(_) => return Err("Не вдалося отримати відповідь входу".into()),
        }
    }
    Err("Час входу минув. Спробуйте знову.".into())
}
fn callback(target: &str, state: &str) -> Result<String, String> {
    if !target.starts_with("/callback?") {
        return Err("Неправильна адреса входу".into());
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1:43827{target}"))
        .map_err(|_| "Некоректна відповідь входу")?;
    let pairs: Vec<_> = url.query_pairs().collect();
    if pairs.iter().filter(|(k, _)| k == "state").count() != 1
        || !pairs.iter().any(|(k, v)| k == "state" && v == state)
    {
        return Err("Некоректний стан входу".into());
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        return Err("Вхід відхилено".into());
    }
    if pairs.iter().filter(|(k, _)| k == "code").count() != 1 {
        return Err("Некоректний код входу".into());
    }
    pairs
        .into_iter()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned())
        .filter(|s| !s.is_empty())
        .ok_or("Немає коду входу".into())
}

async fn get(
    http: &reqwest::Client,
    c: &Connection,
    url: &str,
    query: &[(&str, String)],
) -> Result<Value, String> {
    json(
        http.get(url)
            .query(query)
            .bearer_auth(&c.access_token)
            .send()
            .await,
    )
    .await
}
#[derive(Clone)]
struct Remote {
    id: String,
    name: String,
}
async fn playlists(
    p: Provider,
    c: &Connection,
    http: &reqwest::Client,
    cancel: &AtomicBool,
) -> Result<Vec<Remote>, String> {
    let mut result = Vec::new();
    let mut page = String::new();
    let mut seen = HashSet::new();
    let mut page_tokens = HashSet::new();
    let mut offset = 0;
    loop {
        cancelled(cancel)?;
        let data = match p {
            Provider::Youtube => {
                get(
                    http,
                    c,
                    "https://www.googleapis.com/youtube/v3/playlists",
                    &[
                        ("part", "snippet".into()),
                        ("mine", "true".into()),
                        ("maxResults", "50".into()),
                        ("pageToken", page.clone()),
                    ],
                )
                .await?
            }
            Provider::Spotify => {
                get(
                    http,
                    c,
                    "https://api.spotify.com/v1/me/playlists",
                    &[("limit", "50".into()), ("offset", offset.to_string())],
                )
                .await?
            }
        };
        let rows = data["items"]
            .as_array()
            .ok_or("Некоректний список плейлістів")?;
        for row in rows {
            let id = row["id"].as_str().ok_or("Некоректний ID плейліста")?;
            if !seen.insert(id.to_owned()) {
                continue;
            }
            let name = row["name"]
                .as_str()
                .or_else(|| row.pointer("/snippet/title").and_then(Value::as_str))
                .unwrap_or("Playlist");
            result.push(Remote {
                id: id.into(),
                name: name.into(),
            });
        }
        if result.len() > MAX_PLAYLISTS {
            return Err("Ліміт — 500 плейлістів. Імпортуйте потрібні за посиланням.".into());
        }
        match p {
            Provider::Youtube => {
                let next = data["nextPageToken"].as_str().unwrap_or("");
                if next.is_empty() {
                    break;
                }
                if !page_tokens.insert(next.to_owned()) {
                    return Err("Сервіс повторив сторінку плейлістів".into());
                }
                page = next.into();
            }
            Provider::Spotify => {
                if data["next"].is_null() {
                    break;
                }
                if rows.is_empty() {
                    return Err("Порожня сторінка плейлістів".into());
                }
                offset += rows.len();
                if offset > MAX_PLAYLISTS {
                    return Err("Ліміт — 500 плейлістів".into());
                }
            }
        }
    }
    Ok(result)
}
fn youtube_track(row: &Value) -> Option<Track> {
    let snippet = &row["snippet"];
    let id = snippet.pointer("/resourceId/videoId")?.as_str()?;
    let title = snippet["title"].as_str()?;
    if id.len() != 11
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    Some(Track {
        path: format!("yt://{id}"),
        title: title.into(),
        artist: snippet["videoOwnerChannelTitle"]
            .as_str()
            .unwrap_or("")
            .into(),
        video_id: Some(id.into()),
        thumbnail_url: snippet
            .pointer("/thumbnails/high/url")
            .or_else(|| snippet.pointer("/thumbnails/default/url"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        ..Default::default()
    })
}
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
async fn spotify_track(row: &Value, c: &mut Connection) -> Result<Track, String> {
    let item = row
        .get("item")
        .filter(|v| !v.is_null())
        .or_else(|| row.get("track"))
        .ok_or("Недоступна композиція Spotify")?;
    if item["type"] != "track" || row["is_local"] == true || item["is_local"] == true {
        return Err("Подкаст або локальний файл Spotify".into());
    }
    let title = item["name"].as_str().ok_or("Немає назви композиції")?;
    let artist = item
        .pointer("/artists/0/name")
        .and_then(Value::as_str)
        .ok_or("Немає виконавця")?;
    let duration = item["duration_ms"].as_f64().unwrap_or(0.) / 1000.;
    let source_id = item["id"].as_str().ok_or("Немає ID композиції Spotify")?;
    let from_id = |id: String| Track {
        path: format!("yt://{id}"),
        video_id: Some(id),
        title: title.into(),
        artist: artist.into(),
        album: item
            .pointer("/album/name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        duration_secs: duration,
        thumbnail_url: item
            .pointer("/album/images/0/url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        ..Default::default()
    };
    if let Some(id) = c.matches.get(source_id).filter(|id| {
        id.len() == 11
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }) {
        return Ok(from_id(id.clone()));
    }
    let songs = crate::native::ytm_search_songs(format!("{title} {artist}"), Some(12)).await?;
    let t = normalize(title);
    let a = normalize(artist);
    let song = songs
        .into_iter()
        .find(|s| {
            s.kind == "song"
                && normalize(&s.title) == t
                && normalize(&s.artist) == a
                && (duration == 0. || (s.duration_secs as f64 - duration).abs() < 8.)
        })
        .ok_or_else(|| format!("Не знайдено точний збіг: {artist} — {title}"))?;
    if c.matches.len() < MAX_TRACKS {
        c.matches.insert(source_id.into(), song.video_id.clone());
    }
    Ok(from_id(song.video_id))
}
async fn fetch_playlist(
    p: Provider,
    c: &mut Connection,
    http: &reqwest::Client,
    r: Remote,
    cancel: &AtomicBool,
) -> Result<ImportedPlaylist, String> {
    let mut tracks = Vec::new();
    let mut page = String::new();
    let mut tokens = HashSet::new();
    let mut offset = 0;
    let mut missing = 0;
    let mut spotify_cache = BTreeMap::<String, Track>::new();
    loop {
        cancelled(cancel)?;
        let data = match p {
            Provider::Youtube => {
                get(
                    http,
                    c,
                    "https://www.googleapis.com/youtube/v3/playlistItems",
                    &[
                        ("part", "snippet".into()),
                        ("playlistId", r.id.clone()),
                        ("maxResults", "50".into()),
                        ("pageToken", page.clone()),
                    ],
                )
                .await?
            }
            Provider::Spotify => {
                get(
                    http,
                    c,
                    &format!("https://api.spotify.com/v1/playlists/{}/items", r.id),
                    &[("limit", "50".into()), ("offset", offset.to_string())],
                )
                .await?
            }
        };
        let rows = data["items"]
            .as_array()
            .ok_or("Некоректні композиції плейліста")?;
        for row in rows {
            cancelled(cancel)?;
            match p {
                Provider::Youtube => {
                    if let Some(track) = youtube_track(row) {
                        tracks.push(track);
                    } else {
                        missing += 1;
                    }
                }
                Provider::Spotify => {
                    let key = row
                        .pointer("/item/id")
                        .or_else(|| row.pointer("/track/id"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    if let Some(track) = spotify_cache.get(&key) {
                        tracks.push(track.clone());
                        continue;
                    }
                    let track = spotify_track(row, c).await?;
                    spotify_cache.insert(key, track.clone());
                    tracks.push(track);
                }
            }
        }
        offset += rows.len();
        if offset > MAX_TRACKS {
            return Err("Плейліст перевищує ліміт 20 000 композицій; зміни не застосовано".into());
        }
        match p {
            Provider::Youtube => {
                let next = data["nextPageToken"].as_str().unwrap_or("");
                if next.is_empty() {
                    break;
                }
                if !tokens.insert(next.to_owned()) {
                    return Err("Сервіс повторив сторінку композицій".into());
                }
                page = next.into();
            }
            Provider::Spotify => {
                if data["next"].is_null() {
                    break;
                }
                if rows.is_empty() {
                    return Err("Порожня сторінка композицій".into());
                }
            }
        }
    }
    if missing > 0 {
        return Err(format!(
            "{}: {missing} недоступних композицій. Плейліст збережено без змін.",
            r.name
        ));
    }
    Ok(ImportedPlaylist {
        id: import::local_id(p, &c.owner, &r.id),
        name: r.name,
        tracks,
    })
}

enum Job {
    Connect(String, String),
    Public(String),
    Pull(Option<String>),
}
struct Output {
    connection: Connection,
    playlists: Vec<ImportedPlaylist>,
    errors: Vec<String>,
    connected: bool,
}
async fn work(
    p: Provider,
    mut c: Connection,
    job: Job,
    cancel: Arc<AtomicBool>,
) -> Result<Output, String> {
    let job = if let Job::Public(link) = job {
        match crate::native::ytm_import_playlist(link.clone()).await {
            Ok(playlist) => {
                cancelled(&cancel)?;
                return Ok(Output {
                    connection: c,
                    playlists: vec![playlist],
                    errors: vec![],
                    connected: false,
                });
            }
            Err(error) if c.refresh_token.is_empty() && c.browser_target.is_empty() => {
                return Err(error)
            }
            Err(_) => Job::Pull(Some(import::playlist_link(&link)?.1)),
        }
    } else {
        job
    };
    if let Job::Connect(id, secret) = job {
        if p == Provider::Youtube {
            let signed = crate::youtube_browser::connect(&cancel).await?;
            return Ok(Output {
                connection: Connection {
                    browser_target: signed.target,
                    owner: signed.identity.owner,
                    label: signed.identity.label,
                    auto: true,
                    ..Default::default()
                },
                playlists: vec![],
                errors: vec![],
                connected: true,
            });
        }
        return Ok(Output {
            connection: authorize(p, id, secret, cancel).await?,
            playlists: vec![],
            errors: vec![],
            connected: true,
        });
    }
    if p == Provider::Youtube {
        let id = if let Job::Pull(id) = job { id } else { None };
        let library =
            crate::youtube_browser::pull(&c.browser_target, &c.owner, id, &cancel).await?;
        return Ok(Output {
            connection: c,
            playlists: library.playlists,
            errors: library.errors,
            connected: false,
        });
    }
    let http = client()?;
    refresh(p, &mut c, &http).await?;
    let remotes = async {
        if let Job::Pull(Some(id)) = job {
            let url = match p {
                Provider::Youtube => "https://www.googleapis.com/youtube/v3/playlists".into(),
                Provider::Spotify => format!("https://api.spotify.com/v1/playlists/{id}"),
            };
            let data = get(
                &http,
                &c,
                &url,
                &if p == Provider::Youtube {
                    vec![("part", "snippet".into()), ("id", id.clone())]
                } else {
                    vec![]
                },
            )
            .await?;
            let row = if p == Provider::Youtube {
                &data["items"][0]
            } else {
                &data
            };
            let name = row["name"]
                .as_str()
                .or_else(|| row.pointer("/snippet/title").and_then(Value::as_str))
                .ok_or("Плейліст не знайдено")?;
            Ok(vec![Remote {
                id,
                name: name.into(),
            }])
        } else {
            playlists(p, &c, &http, &cancel).await
        }
    }
    .await;
    let mut output = Output {
        connection: c,
        playlists: vec![],
        errors: vec![],
        connected: false,
    };
    // Return refreshed credentials even if the subsequent API call fails.
    // Only the controller may persist a generation-checked result.
    let remotes = match remotes {
        Ok(rows) => rows,
        Err(e) => {
            output.errors.push(e);
            return Ok(output);
        }
    };
    for remote in remotes {
        cancelled(&cancel)?;
        match fetch_playlist(p, &mut output.connection, &http, remote, &cancel).await {
            Ok(playlist) => output.playlists.push(playlist),
            Err(e) => {
                let stop = e.starts_with("Забагато запитів")
                    || e.starts_with("Немає зв’язку")
                    || e.starts_with("Сесію завершено");
                output.errors.push(e);
                if stop {
                    break;
                }
            }
        }
    }
    Ok(output)
}

pub fn install(
    w: &MainWindow,
    get: Rc<dyn Fn() -> Store>,
    apply: Rc<dyn Fn(Store)>,
    runtime: Arc<tokio::runtime::Runtime>,
) -> slint::Timer {
    struct State {
        sessions: Sessions,
        busy: bool,
        generation: u64,
        cancel: Arc<AtomicBool>,
        next: u64,
        pending: Vec<Provider>,
        task: Option<tokio::task::JoinHandle<()>>,
    }
    let sessions = load();
    w.set_music_supported(!cfg!(target_os = "android"));
    w.set_music_auto_sync(sessions.youtube.auto);
    w.set_youtube_account(sessions.youtube.label.clone().into());
    w.set_spotify_account(sessions.spotify.label.clone().into());
    let state = Rc::new(RefCell::new(State {
        sessions,
        busy: false,
        generation: 0,
        cancel: Arc::new(AtomicBool::new(false)),
        next: now_ms() + 3000,
        pending: vec![],
        task: None,
    }));
    let (tx, rx) = std::sync::mpsc::channel();
    let dispatch = {
        let runtime = runtime.clone();
        Rc::new(move |generation, p, c, job, cancel| {
            let tx = tx.clone();
            runtime.spawn(async move {
                let result = work(p, c, job, cancel).await;
                let _ = tx.send((generation, p, result));
            })
        })
    };
    let weak = w.as_weak();
    let shared = state.clone();
    let send = dispatch.clone();
    w.on_music_command(move |command| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut s = shared.borrow_mut();
        if command == "youtube-browser-login" {
            ui.set_music_status(match crate::native::open_ytm_login() {
                Ok(()) => "Вхід YouTube Music відкрито у типовому браузері. Це не підключає автоматичну синхронізацію Vey.".into(),
                Err(error) => error.into(),
            });
            return;
        }
        let p = if ui.get_music_provider() == 1 {
            Provider::Spotify
        } else {
            Provider::Youtube
        };
        if command == "select" {
            ui.set_music_auto_sync(s.sessions.connection(p).auto);
            ui.set_music_status("".into());
            return;
        }
        if command == "cancel" || command == "disconnect" {
            s.cancel.store(true, Ordering::Relaxed);
            if let Some(task) = s.task.take() {
                task.abort();
            }
            s.generation += 1;
            s.busy = false;
            s.pending.clear();
            ui.set_music_busy(false);
            if command == "disconnect" {
                *s.sessions.connection_mut(p) = Connection::default();
                if p == Provider::Youtube {
                    ui.set_youtube_account("".into());
                } else {
                    ui.set_spotify_account("".into());
                }
                if let Err(e) = save(&s.sessions) {
                    ui.set_music_status(e.into());
                    return;
                }
            }
            ui.set_music_status(
                if command == "cancel" {
                    "Скасовано"
                } else {
                    "Акаунт відключено. Імпортовані плейлісти збережено."
                }
                .into(),
            );
            return;
        }
        if command == "auto" {
            s.sessions.connection_mut(p).auto = ui.get_music_auto_sync();
            s.next = now_ms() + 900000;
            if let Err(e) = save(&s.sessions) {
                ui.set_music_status(e.into());
            }
            return;
        }
        if s.busy {
            return;
        }
        if command == "sync-all" {
            s.pending = vec![Provider::Spotify, Provider::Youtube];
            ui.set_music_status("Оновлення підключених акаунтів…".into());
            return;
        }
        let (p, job) = match command.as_str() {
            "connect" if p == Provider::Youtube => (p, Job::Connect(String::new(), String::new())),
            "connect" => {
                let configured = ui.get_music_client_id().trim().to_owned();
                let id = if configured.is_empty() {
                    std::env::var("VEY_SPOTIFY_CLIENT_ID")
                        .unwrap_or_else(|_| s.sessions.connection(p).client_id.clone())
                } else {
                    configured
                };
                let secret = if ui.get_music_client_secret().is_empty() {
                    s.sessions.connection(p).client_secret.clone()
                } else {
                    ui.get_music_client_secret().to_string()
                };
                ui.set_music_client_secret("".into());
                (p, Job::Connect(id, secret))
            }
            "import" => match import::playlist_link(&ui.get_music_link()) {
                Ok((Provider::Youtube, _)) => (
                    Provider::Youtube,
                    Job::Public(ui.get_music_link().to_string()),
                ),
                Ok((p, id)) => (p, Job::Pull(Some(id))),
                Err(e) => {
                    ui.set_music_status(e.into());
                    return;
                }
            },
            _ => (p, Job::Pull(None)),
        };
        s.cancel = Arc::new(AtomicBool::new(false));
        s.busy = true;
        s.generation += 1;
        ui.set_music_busy(true);
        ui.set_music_status(
            format!(
                "{} · {}",
                p.name(),
                if matches!(job, Job::Connect(..)) {
                    "Завершіть вхід у браузері…"
                } else {
                    "Імпорт плейлістів…"
                }
            )
            .into(),
        );
        s.task = Some(send(
            s.generation,
            p,
            s.sessions.connection(p).clone(),
            job,
            s.cancel.clone(),
        ));
    });
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut s = state.borrow_mut();
            while let Ok((generation, p, result)) = rx.try_recv() {
                if generation != s.generation {
                    continue;
                }
                s.busy = false;
                ui.set_music_busy(false);
                match result {
                    Ok(mut output) => {
                        if output.connected && output.connection.auto {
                            s.pending.push(p);
                        }
                        // Reconnecting the same account retains conflict baselines.
                        if (p == Provider::Youtube
                            || output.connection.owner == s.sessions.connection(p).owner)
                            && output.connection.baselines.is_empty()
                        {
                            output.connection.baselines =
                                s.sessions.connection(p).baselines.clone();
                            output.connection.matches = s.sessions.connection(p).matches.clone();
                        }
                        let mut store = get();
                        let mut count = 0;
                        for playlist in output.playlists {
                            match import::apply(
                                &mut store,
                                &playlist,
                                output
                                    .connection
                                    .baselines
                                    .get(&playlist.id)
                                    .map(String::as_str),
                            ) {
                                Ok(hash) => {
                                    output.connection.baselines.insert(playlist.id, hash);
                                    count += 1;
                                }
                                Err(e) => output.errors.push(e),
                            }
                        }
                        if count > 0 {
                            apply(store);
                        }
                        *s.sessions.connection_mut(p) = output.connection;
                        if let Err(e) = save(&s.sessions) {
                            output.errors.push(e);
                        }
                        ui.set_youtube_account(s.sessions.youtube.label.clone().into());
                        ui.set_spotify_account(s.sessions.spotify.label.clone().into());
                        let selected = if ui.get_music_provider() == 1 {
                            Provider::Spotify
                        } else {
                            Provider::Youtube
                        };
                        ui.set_music_auto_sync(s.sessions.connection(selected).auto);
                        let message = if output.errors.is_empty() {
                            format!(
                                "{} · імпортовано плейлістів: {count}. {}",
                                p.name(),
                                if count == 0 {
                                    "Акаунт підключено; натисніть Синхронізувати."
                                } else {
                                    "Доступні для синхронізації з Vey."
                                }
                            )
                        } else {
                            format!(
                                "{} · імпортовано: {count}. {}",
                                p.name(),
                                output.errors.join(" · ")
                            )
                        };
                        ui.set_music_status(message.chars().take(2000).collect::<String>().into());
                    }
                    Err(e) => {
                        ui.set_music_status(format!("{}: {e}", p.name()).into());
                    }
                }
            }
            if !s.busy && now_ms() >= s.next {
                s.next = now_ms() + 900000;
                s.pending = [Provider::Spotify, Provider::Youtube]
                    .into_iter()
                    .filter(|p| s.sessions.connection(*p).auto)
                    .collect();
            }
            if !s.busy {
                while let Some(p) = s.pending.pop() {
                    if s.sessions.connection(p).refresh_token.is_empty()
                        && s.sessions.connection(p).browser_target.is_empty()
                    {
                        continue;
                    }
                    s.busy = true;
                    s.generation += 1;
                    s.cancel = Arc::new(AtomicBool::new(false));
                    ui.set_music_busy(true);
                    s.task = Some(dispatch(
                        s.generation,
                        p,
                        s.sessions.connection(p).clone(),
                        Job::Pull(None),
                        s.cancel.clone(),
                    ));
                    break;
                }
            }
        },
    );
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oauth_callback_checks_state_and_denial() {
        assert_eq!(
            callback("/callback?code=hello&state=correct", "correct").unwrap(),
            "hello"
        );
        for target in [
            "/callback?code=x&state=wrong",
            "/callback?code=x&state=correct&state=wrong",
            "/evil?code=x&state=correct",
            "/callback?code=x&code=y&state=correct",
        ] {
            assert!(callback(target, "correct").is_err());
        }
        assert_eq!(
            callback("/callback?error=access_denied&state=correct", "correct").unwrap_err(),
            "Вхід відхилено"
        );
    }
    #[test]
    fn youtube_preserves_known_unavailable_references() {
        let row = serde_json::json!({"snippet":{"title":"Song","resourceId":{"videoId":"abcdefghijk"},"videoOwnerChannelTitle":"Artist"}});
        assert_eq!(youtube_track(&row).unwrap().path, "yt://abcdefghijk");
        let mut bad = row;
        bad["snippet"]["title"] = "Private video".into();
        assert_eq!(youtube_track(&bad).unwrap().title, "Private video");
        bad["snippet"]["resourceId"]["videoId"] = "invalid".into();
        assert!(youtube_track(&bad).is_none());
    }
    #[test]
    fn pkce_matches_rfc7636_example() {
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(
                b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
            )),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
}
