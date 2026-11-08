//! User-driven Google v3 account chooser in a dedicated installed browser.
//! Only normalized read-only YouTube results cross the browser boundary.
use crate::library_import::{self, ImportedPlaylist, Provider};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const ORIGIN: &str = "https://music.youtube.com";
const SCRIPT: &str = include_str!("../assets/youtube-browser.js");

#[derive(Clone, Deserialize)]
pub struct Identity {
    pub owner: String,
    pub label: String,
}
pub struct Connected {
    pub target: String,
    pub identity: Identity,
}
pub struct Library {
    pub playlists: Vec<ImportedPlaylist>,
    pub errors: Vec<String>,
}

fn cancelled(flag: &AtomicBool) -> Result<(), String> {
    if flag.load(Ordering::Relaxed) {
        Err("Скасовано".into())
    } else {
        Ok(())
    }
}
fn profile() -> PathBuf {
    crate::storage::data_dir().join("youtube-browser")
}
fn prepare_profile() -> Result<PathBuf, String> {
    let root = crate::storage::data_dir();
    std::fs::create_dir_all(&root).map_err(|_| "Не вдалося створити сховище Vey")?;
    let dir = profile();
    let marker = dir.join("vey-profile.txt");
    if dir.exists() && !marker.is_file() {
        return Err("Каталог youtube-browser уже зайнятий стороннім профілем".into());
    }
    std::fs::create_dir_all(&dir).map_err(|_| "Не вдалося створити профіль YouTube Music")?;
    let canonical =
        std::fs::canonicalize(&dir).map_err(|_| "Не вдалося перевірити профіль YouTube Music")?;
    let root = std::fs::canonicalize(root).map_err(|_| "Не вдалося перевірити сховище Vey")?;
    if canonical.parent() != Some(root.as_path()) {
        return Err("Профіль YouTube Music має залишатися у сховищі Vey".into());
    }
    if !marker.exists() {
        std::fs::write(&marker, b"Vey-owned YouTube Music browser profile\n")
            .map_err(|_| "Не вдалося зберегти профіль входу")?;
    }
    Ok(dir)
}
fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Не вдалося підключити браузер".into())
}
fn websocket(url: &str, port: u16, kind: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "Некоректна відповідь браузера")?;
    if parsed.scheme() != "ws"
        || parsed.host_str() != Some("127.0.0.1")
        || parsed.port() != Some(port)
        || !parsed.path().starts_with(&format!("/devtools/{kind}/"))
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("Браузер повернув сторонню адресу; підключення зупинено".into());
    }
    Ok(parsed.into())
}
struct Endpoint {
    port: u16,
    browser: String,
}
async fn endpoint(dir: &Path) -> Result<Endpoint, String> {
    if std::fs::read(dir.join("vey-profile.txt")).ok().as_deref()
        != Some(b"Vey-owned YouTube Music browser profile\n")
    {
        return Err("Відкрийте профіль YouTube Music через Vey".into());
    }
    // Only the profile Vey created is consulted. Never inspect ordinary profiles.
    let bytes = std::fs::read(dir.join("DevToolsActivePort"))
        .map_err(|_| "Відкрийте вхід YouTube Music у Vey знову")?;
    if bytes.len() > 4096 {
        return Err("Некоректний файл підключення браузера".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "Некоректний файл підключення браузера")?;
    let mut lines = text.lines();
    let port = lines
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|p| *p > 0)
        .ok_or("Некоректний порт браузера")?;
    let path = lines
        .next()
        .filter(|s| s.starts_with("/devtools/browser/"))
        .ok_or("Некоректне підключення браузера")?;
    let version: Value = http()?
        .get(format!("http://127.0.0.1:{port}/json/version"))
        .send()
        .await
        .map_err(|_| "Браузер закрито. Натисніть Підключити YouTube Music знову.")?
        .error_for_status()
        .map_err(|_| "Браузер не відповідає")?
        .json()
        .await
        .map_err(|_| "Некоректна відповідь браузера")?;
    let browser = websocket(
        version["webSocketDebuggerUrl"]
            .as_str()
            .ok_or("Браузер не підтримує підключення")?,
        port,
        "browser",
    )?;
    if reqwest::Url::parse(&browser)
        .ok()
        .is_none_or(|url| url.path() != path)
    {
        return Err("Профіль браузера змінився. Відкрийте вхід знову.".into());
    }
    Ok(Endpoint { port, browser })
}
#[derive(Deserialize)]
struct Target {
    id: String,
    #[serde(default)]
    url: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(rename = "webSocketDebuggerUrl", default)]
    websocket: String,
}
async fn targets(endpoint: &Endpoint) -> Result<Vec<Target>, String> {
    http()?
        .get(format!("http://127.0.0.1:{}/json/list", endpoint.port))
        .send()
        .await
        .map_err(|_| "Браузер закрито. Відкрийте вхід YouTube Music знову.")?
        .error_for_status()
        .map_err(|_| "Браузер не відповідає")?
        .json()
        .await
        .map_err(|_| "Некоректні вкладки браузера".into())
}
fn music_origin(url: &str) -> bool {
    reqwest::Url::parse(url).ok().is_some_and(|u| {
        u.origin().ascii_serialization() == ORIGIN
            && u.username().is_empty()
            && u.password().is_none()
    })
}
struct Cdp {
    socket: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    serial: u64,
}
impl Cdp {
    async fn open(url: &str) -> Result<Self, String> {
        let (socket, _) = connect_async(url)
            .await
            .map_err(|_| "Не вдалося підключитися до вкладки браузера")?;
        Ok(Self { socket, serial: 0 })
    }
    async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.serial += 1;
        let id = self.serial;
        self.socket
            .send(Message::Text(
                json!({"id":id,"method":method,"params":params})
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|_| "Браузер закрив підключення")?;
        tokio::time::timeout(Duration::from_secs(120), async {
            for _ in 0..2000 {
                let message = self
                    .socket
                    .next()
                    .await
                    .ok_or("Браузер закрито")?
                    .map_err(|_| "Браузер закрив підключення")?;
                if !message.is_text() {
                    continue;
                }
                let value: Value = serde_json::from_str(
                    message
                        .to_text()
                        .map_err(|_| "Некоректна відповідь браузера")?,
                )
                .map_err(|_| "Некоректна відповідь браузера")?;
                if value["id"].as_u64() != Some(id) {
                    continue;
                }
                if value.get("error").is_some() {
                    return Err("Команда браузера недоступна".to_owned());
                }
                return Ok(value["result"].clone());
            }
            Err("Забагато подій браузера".into())
        })
        .await
        .map_err(|_| "Браузер не завершив запит. Спробуйте знову.".to_owned())?
    }
    async fn evaluate(&mut self, request: Value) -> Result<Value, String> {
        let expression = format!("({SCRIPT})({request})");
        let result = self
            .call(
                "Runtime.evaluate",
                json!({"expression":expression,"awaitPromise":true,"returnByValue":true}),
            )
            .await?;
        if result.get("exceptionDetails").is_some() {
            return Err("YouTube Music ще завантажується. Спробуйте знову.".into());
        }
        let value = result
            .pointer("/result/value")
            .cloned()
            .ok_or("Вкладка не повернула дані YouTube Music")?;
        if value["ok"] != true {
            return Err(script_error(
                value["error"].as_str().unwrap_or("request_failed"),
            ));
        }
        Ok(value)
    }
}
fn script_error(code: &str) -> String {
    match code {
        "wrong_origin" => "Поверніться до YouTube Music у вікні входу Vey.",
        "sign_in_required" => "Завершіть звичайний вхід Google у браузері.",
        "session_unavailable" => "Дочекайтеся завантаження YouTube Music у браузері.",
        "account_changed" => {
            "Акаунт у браузері змінився. Натисніть Підключити, щоб підтвердити новий акаунт."
        }
        "rate_limited" => "YouTube Music обмежив запити. Спробуйте пізніше.",
        "access_denied" => "YouTube Music не надав доступ до цього плейліста.",
        "unknown_row" => "Композиція не має відомого ID; неповний імпорт не збережено.",
        "page_cycle" => "YouTube Music повторив сторінку; неповний імпорт не збережено.",
        "too_large" => "Перевищено ліміт 500 плейлістів або 20 000 композицій.",
        _ => {
            "Не вдалося прочитати бібліотеку YouTube Music. Перевірте вкладку браузера й повторіть."
        }
    }
    .into()
}

fn browser_program() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("VEY_YOUTUBE_BROWSER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err("VEY_YOUTUBE_BROWSER має вказувати на встановлений Chrome або Edge".into());
    }
    #[cfg(target_os = "windows")]
    for root in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(root) {
            for relative in [
                "Google/Chrome/Application/chrome.exe",
                "Microsoft/Edge/Application/msedge.exe",
            ] {
                let path = PathBuf::from(&root).join(relative);
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    for path in [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ] {
        if Path::new(path).is_file() {
            return Ok(path.into());
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    for path in [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/microsoft-edge",
    ] {
        if Path::new(path).is_file() {
            return Ok(path.into());
        }
    }
    Err("Для входу потрібен встановлений Chrome або Edge. Можна вказати шлях у VEY_YOUTUBE_BROWSER.".into())
}

#[cfg(not(target_os = "android"))]
pub async fn connect(cancel: &AtomicBool) -> Result<Connected, String> {
    let dir = prepare_profile()?;
    // No headless/automation flags and no password handling. Google owns login.
    let mut process = std::process::Command::new(browser_program()?)
        .arg(format!("--user-data-dir={}", dir.display()))
        .args([
            "--remote-debugging-address=127.0.0.1",
            "--remote-debugging-port=0",
            "--no-first-run",
            "--no-default-browser-check",
            "about:blank",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "Не вдалося відкрити Chrome/Edge для входу")?;
    let _ = std::thread::Builder::new()
        .name("vey-youtube-browser".into())
        .spawn(move || {
            let _ = process.wait();
        });
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let endpoint = loop {
        cancelled(cancel)?;
        if let Ok(endpoint) = endpoint(&dir).await {
            break endpoint;
        }
        if std::time::Instant::now() > deadline {
            return Err(
                "Браузер не відкрив профіль входу Vey. Закрийте його вікно й повторіть.".into(),
            );
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    let mut browser = Cdp::open(&endpoint.browser).await?;
    let created = browser
        .call(
            "Target.createTarget",
            json!({"url":crate::native::YTM_LOGIN_URL}),
        )
        .await?;
    let target = created["targetId"]
        .as_str()
        .ok_or("Не вдалося відкрити сторінку входу Google")?
        .to_owned();
    drop(browser);
    // Bind completion to the newly created tab, never an older signed-in tab.
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    while std::time::Instant::now() < deadline {
        cancelled(cancel)?;
        let pages = targets(&endpoint).await?;
        let page = pages
            .iter()
            .find(|p| p.id == target)
            .ok_or("Вкладку входу закрито. Натисніть Підключити знову.")?;
        if page.kind == "page" && music_origin(&page.url) {
            let url = websocket(&page.websocket, endpoint.port, "page")?;
            let result = async {
                Cdp::open(&url)
                    .await?
                    .evaluate(json!({"operation":"identity"}))
                    .await
            }
            .await;
            if let Err(error) = &result {
                if error == &script_error("rate_limited") || error == &script_error("access_denied")
                {
                    return Err(error.clone());
                }
            }
            if let Ok(value) = result {
                let identity: Identity = serde_json::from_value(value["identity"].clone())
                    .map_err(|_| "Не вдалося визначити акаунт YouTube Music")?;
                return Ok(Connected { target, identity });
            }
        }
        tokio::time::sleep(Duration::from_millis(700)).await;
    }
    Err("Час входу минув. Завершіть вхід у браузері та натисніть Підключити знову.".into())
}
#[cfg(target_os = "android")]
pub async fn connect(_: &AtomicBool) -> Result<Connected, String> {
    Err("Вхід через браузер доступний у настільній Vey".into())
}

#[derive(Deserialize)]
struct Remote {
    id: String,
    name: String,
}

/// Read a bounded, normalized Home feed from the already verified browser tab.
/// Authentication and raw YouTube responses remain inside that tab.
pub async fn home(target: &str, owner: &str) -> Result<crate::discovery::Catalog, String> {
    if target.is_empty() || owner.is_empty() {
        return Err("Спочатку підключіть YouTube Music через браузер".into());
    }
    let endpoint = endpoint(&profile()).await?;
    let page = targets(&endpoint).await?.into_iter()
        .find(|page| page.id == target && page.kind == "page")
        .ok_or("Вкладку YouTube Music закрито. Натисніть Підключити знову.")?;
    if !music_origin(&page.url) { return Err(script_error("wrong_origin")); }
    let mut browser = Cdp::open(&websocket(&page.websocket, endpoint.port, "page")?).await?;
    let result = browser.evaluate(json!({"operation":"home","expectedOwner":owner})).await?;
    let mut catalog: crate::discovery::Catalog = serde_json::from_value(result["catalog"].clone())
        .map_err(|_| "Некоректні рекомендації YouTube Music")?;
    catalog.sections.truncate(8);
    for section in &mut catalog.sections {
        section.items.truncate(10);
        let section_kind = section.kind.clone();
        section.items.retain(|item| {
            if item.kind != section_kind || item.title.trim().is_empty() || item.title.len() > 2048 || item.subtitle.len() > 2048 { return false; }
            if !item.thumbnail_url.is_empty() && !reqwest::Url::parse(&item.thumbnail_url).ok()
                .is_some_and(|url| url.scheme() == "https" && url.username().is_empty() && url.password().is_none()) { return false; }
            match item.kind.as_str() {
                "song" | "video" => crate::services::video_id(&item.video_id)
                    && item.path == format!("yt://{}", item.video_id) && item.key == item.path,
                "album" | "artist" | "playlist" => !item.browse_id.is_empty() && item.browse_id.len() <= 200
                    && item.browse_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                    && item.path == format!("catalog://{}", item.browse_id) && item.key == item.path,
                _ => false,
            }
        });
    }
    catalog.sections.retain(|section| !section.items.is_empty());
    if catalog.sections.is_empty() { return Err("YouTube Music не повернув рекомендацій".into()); }
    catalog.updated_at = crate::services::now_ms();
    Ok(catalog)
}
pub async fn pull(
    target: &str,
    owner: &str,
    id: Option<String>,
    cancel: &AtomicBool,
) -> Result<Library, String> {
    if target.is_empty() || owner.is_empty() {
        return Err("Спочатку підключіть YouTube Music через браузер".into());
    }
    let endpoint = endpoint(&profile()).await?;
    let page = targets(&endpoint)
        .await?
        .into_iter()
        .find(|p| p.id == target && p.kind == "page")
        .ok_or("Вкладку YouTube Music закрито. Натисніть Підключити знову.")?;
    if !music_origin(&page.url) {
        return Err(script_error("wrong_origin"));
    }
    let mut browser = Cdp::open(&websocket(&page.websocket, endpoint.port, "page")?).await?;
    let remotes = if let Some(id) = id {
        vec![Remote {
            id,
            name: String::new(),
        }]
    } else {
        let data = browser
            .evaluate(json!({"operation":"playlists","expectedOwner":owner}))
            .await?;
        serde_json::from_value::<Vec<Remote>>(data["playlists"].clone())
            .map_err(|_| "Некоректні плейлісти YouTube Music")?
    };
    if remotes.len() > 500 {
        return Err(script_error("too_large"));
    }
    let mut library = Library {
        playlists: vec![],
        errors: vec![],
    };
    for remote in remotes {
        cancelled(cancel)?;
        match browser
            .evaluate(json!({"operation":"playlist","id":remote.id,"expectedOwner":owner}))
            .await
        {
            Ok(data) => {
                let tracks: Vec<crate::services::Track> =
                    serde_json::from_value(data["tracks"].clone())
                        .map_err(|_| "Некоректні композиції YouTube Music")?;
                if tracks.len() > 20000 {
                    return Err(script_error("too_large"));
                }
                library.playlists.push(ImportedPlaylist {
                    id: library_import::local_id(Provider::Youtube, owner, &remote.id),
                    name: if remote.name.is_empty() {
                        data["name"].as_str().unwrap_or(&remote.id).to_owned()
                    } else {
                        remote.name
                    },
                    tracks,
                });
            }
            Err(error) => {
                library.errors.push(error);
                break;
            }
        }
    }
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cdp_correlates_responses_and_never_echoes_raw_errors() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            for n in 0..2 {
                let command = socket.next().await.unwrap().unwrap();
                let value: Value = serde_json::from_str(command.to_text().unwrap()).unwrap();
                socket
                    .send(Message::Text(
                        json!({"method":"Runtime.consoleAPICalled","params":{"ignored":true}})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                let response = if n == 0 {
                    json!({"id":value["id"],"result":{"result":{"value":{"ok":true,"identity":{"owner":"fixture","label":"Browser fixture"}}}}})
                } else {
                    json!({"id":value["id"],"error":{"message":"cookie=private-secret"}})
                };
                socket
                    .send(Message::Text(response.to_string().into()))
                    .await
                    .unwrap();
            }
        });
        let mut cdp = Cdp::open(&format!("ws://{address}")).await.unwrap();
        let result = cdp.evaluate(json!({"operation":"identity"})).await.unwrap();
        assert_eq!(result["identity"]["owner"], "fixture");
        let error = cdp
            .evaluate(json!({"operation":"identity"}))
            .await
            .unwrap_err();
        assert!(!error.contains("private-secret"));
        server.await.unwrap();
    }
    #[test]
    fn browser_addresses_are_loopback_and_profile_scoped() {
        assert!(websocket("ws://127.0.0.1:4567/devtools/page/abc", 4567, "page").is_ok());
        for url in [
            "ws://example.com:4567/devtools/page/a",
            "ws://127.0.0.1:4568/devtools/page/a",
            "ws://user@127.0.0.1:4567/devtools/page/a",
            "ws://127.0.0.1:4567/devtools/browser/a",
            "ws://127.0.0.1:4567/devtools/page/a?token=x",
        ] {
            assert!(websocket(url, 4567, "page").is_err());
        }
        assert!(music_origin("https://music.youtube.com/library"));
        assert!(!music_origin("https://music.youtube.com.evil/library"));
        assert!(!music_origin(
            "https://accounts.google.com/v3/signin/accountchooser"
        ));
    }
    #[test]
    fn returned_errors_never_echo_browser_payloads() {
        assert!(!script_error("cookie=private-token").contains("private-token"));
        assert!(crate::native::YTM_LOGIN_URL
            .starts_with("https://accounts.google.com/v3/signin/accountchooser?"));
        assert!(!crate::native::YTM_LOGIN_URL.contains("client_id="));
    }
}
