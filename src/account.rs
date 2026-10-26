//! Vey account metadata and private audio. Credentials never enter Store/exports.
use crate::{
    services::{History, Playlist, Settings, Store, Track},
    MainWindow,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    time::Duration,
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Session {
    api: String,
    token: String,
    id: String,
    email: String,
    name: String,
    expires_at: u64,
    premium: bool,
    revision: Option<u64>,
    auto_sync: bool,
    auto_upload: bool,
    uploaded: BTreeSet<String>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct CloudLibrary {
    tracks: Vec<Track>,
    likes: BTreeSet<String>,
    playlists: Vec<Playlist>,
    history: Vec<History>,
    settings: Settings,
    offsets: BTreeMap<String, i64>,
    lyrics_choices: BTreeMap<String, crate::services::LyricsChoice>,
    #[serde(rename = "autoUpload")]
    auto_upload: bool,
}
fn cloud_key(key: &str) -> bool {
    key.strip_prefix("yt://").is_some_and(|id| {
        id.len() == 11
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }) || key
        .strip_prefix("cloud://")
        .is_some_and(|id| id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))
}
fn portable_track(track: &Track) -> Track {
    let mut result = track.clone();
    let public_art = reqwest::Url::parse(&result.thumbnail_url)
        .ok()
        .is_some_and(|url| {
            url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
        });
    if !public_art {
        result.thumbnail_url.clear();
    }
    result
}
impl CloudLibrary {
    fn capture(store: &Store, auto_upload: bool) -> Self {
        Self {
            tracks: store
                .tracks
                .iter()
                .filter(|t| cloud_key(&t.path))
                .map(portable_track)
                .collect(),
            likes: store
                .likes
                .iter()
                .filter(|k| cloud_key(k))
                .cloned()
                .collect(),
            playlists: store
                .playlists
                .iter()
                .map(|p| Playlist {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    tracks: p.tracks.iter().filter(|k| cloud_key(k)).cloned().collect(),
                })
                .collect(),
            history: store
                .history
                .iter()
                .filter(|h| cloud_key(&h.track.path))
                .take(5000)
                .map(|h| History {
                    track: portable_track(&h.track),
                    ..h.clone()
                })
                .collect(),
            settings: store.settings.clone(),
            offsets: store
                .offsets
                .iter()
                .filter(|(k, _)| cloud_key(k))
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
            lyrics_choices: store
                .lyrics_choices
                .iter()
                .filter(|(k, _)| cloud_key(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            auto_upload,
        }
    }
    fn restore(self, store: &mut Store) {
        store.tracks.retain(|t| !cloud_key(&t.path));
        store
            .tracks
            .extend(self.tracks.into_iter().filter(|t| cloud_key(&t.path)));
        store.likes.retain(|k| !cloud_key(k));
        store
            .likes
            .extend(self.likes.into_iter().filter(|k| cloud_key(k)));
        let local: Vec<Playlist> = store
            .playlists
            .iter()
            .map(|p| Playlist {
                id: p.id.clone(),
                name: p.name.clone(),
                tracks: p.tracks.iter().filter(|k| !cloud_key(k)).cloned().collect(),
            })
            .filter(|p| !p.tracks.is_empty())
            .collect();
        store.playlists = self.playlists;
        for p in local {
            if let Some(remote) = store.playlists.iter_mut().find(|r| r.id == p.id) {
                remote.tracks.extend(p.tracks);
            } else {
                store.playlists.push(p);
            }
        }
        store.history.retain(|h| !cloud_key(&h.track.path));
        store.history.extend(self.history);
        store.history.sort_by(|a, b| b.played_at.cmp(&a.played_at));
        store.offsets.retain(|k, _| !cloud_key(k));
        store.offsets.extend(self.offsets);
        store.lyrics_choices.retain(|k, _| !cloud_key(k));
        store.lyrics_choices.extend(self.lyrics_choices);
        store.settings = self.settings;
    }
}
#[derive(Clone, Default, Deserialize)]
struct FileInfo {
    id: String,
    name: String,
    size: u64,
}
#[derive(Clone)]
enum Action {
    Login {
        email: String,
        password: String,
        name: String,
        register: bool,
    },
    Push(CloudLibrary),
    Pull,
    Account,
    Files,
    Device,
    Poll(String),
    Logout,
    Upload(String),
    Auto(Vec<String>),
    Check,
    Download(FileInfo),
}
enum Output {
    Unchanged,
    Session(Session),
    Pushed(u64, String),
    Pulled(u64, CloudLibrary, bool),
    Account(Value),
    Files(Vec<FileInfo>),
    Device(Value),
    Pending(u64),
    Logout,
    Uploaded(FileInfo, String, String),
    Downloaded(FileInfo, String),
}
fn base(value: &str) -> Result<String, String> {
    let u = reqwest::Url::parse(value.trim()).map_err(|_| "Укажіть адресу API Vey".to_string())?;
    let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (u.scheme() != "https" && !(local && u.scheme() == "http"))
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("Потрібна HTTPS-адреса API без паролів і параметрів".into());
    }
    Ok(u.as_str().trim_end_matches('/').to_string())
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/web/account.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=accountRequest)]
    fn browser_request(url: &str, method: &str, token: &str, body: &str) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=uploadAccountFile)]
    fn browser_upload(api: &str, token: &str, path: &str) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=downloadAccountFile)]
    fn browser_download(api: &str, token: &str, id: &str) -> js_sys::Promise;
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=cancelAccountUpload)]
    fn browser_cancel();
    #[wasm_bindgen::prelude::wasm_bindgen(js_name=openAccountLogin)]
    fn browser_open_login(url: &str);
}
async fn request(
    s: &Session,
    path: &str,
    method: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let url = format!("{}/vey/{}", base(&s.api)?, path);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Не вдалося створити запит".to_string())?;
        let mut req = client.request(
            method.parse().map_err(|_| "Невідомий запит".to_string())?,
            url,
        );
        if !s.token.is_empty() {
            req = req.bearer_auth(&s.token);
        }
        if let Some(data) = body {
            req = req.json(&data);
        }
        let mut response = req
            .send()
            .await
            .map_err(|e| format!("Помилка мережі: {}", e.without_url()))?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Відповідь API перервана".to_string())?
        {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err("Відповідь API завелика".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let data: Value = serde_json::from_slice(&bytes)
            .map_err(|_| format!("API повернув помилкову відповідь ({status})"))?;
        if status == 401 && !s.token.is_empty() {
            return Err("Сесія завершилася. Увійдіть знову.".into());
        }
        if status == 400
            && path == "oauth/token"
            && matches!(
                data["error"].as_str(),
                Some("authorization_pending" | "slow_down")
            )
        {
            return Ok(data);
        }
        if status >= 300 {
            if let Some(description) = data["error_description"].as_str() {
                return Err(description.into());
            }
            return Err(data["error"]["message"]
                .as_str()
                .unwrap_or("Запит відхилено")
                .to_string());
        }
        Ok(data)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let data = wasm_bindgen_futures::JsFuture::from(browser_request(
            &url,
            method,
            &s.token,
            &body.map(|b| b.to_string()).unwrap_or_default(),
        ))
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| "Помилка мережі".into()))?;
        serde_json::from_str(&data.as_string().unwrap_or_default())
            .map_err(|_| "Некоректна відповідь API".into())
    }
}
fn login_result(mut s: Session, value: Value) -> Result<Session, String> {
    s.token = value
        .get("access_token")
        .or_else(|| value.get("token"))
        .and_then(Value::as_str)
        .filter(|v| v.starts_with("vey_"))
        .ok_or("Відсутня сесія")?
        .into();
    s.id = value["user"]["id"]
        .as_str()
        .ok_or("Відсутній акаунт")?
        .into();
    s.email = value["user"]["email"].as_str().unwrap_or_default().into();
    s.name = value["user"]["display_name"]
        .as_str()
        .unwrap_or_default()
        .into();
    s.expires_at = value["expires_at"]
        .as_u64()
        .ok_or("Відсутній термін сесії")?;
    s.premium = value["premium"].as_bool().unwrap_or(false);
    s.revision = None;
    s.uploaded.clear();
    Ok(s)
}
async fn work(s: Session, action: Action) -> Result<Output, String> {
    match action {
        Action::Login {
            email,
            password,
            name,
            register,
        } => {
            let value = request(
                &Session {
                    token: String::new(),
                    ..s.clone()
                },
                if register {
                    "auth/register"
                } else {
                    "auth/login"
                },
                "POST",
                Some(json!({"email":email,"password":password,"display_name":name})),
            )
            .await?;
            Ok(Output::Session(login_result(s, value)?))
        }
        Action::Push(data) => {
            let revision = match s.revision {
                Some(r) => r,
                None => request(&s, "library", "GET", None).await?["revision"]
                    .as_u64()
                    .ok_or("Немає версії бібліотеки")?,
            };
            let sent =
                serde_json::to_string(&data).map_err(|_| "Помилка бібліотеки".to_string())?;
            let v = request(
                &s,
                "library",
                "PUT",
                Some(json!({"revision":revision,"library":data})),
            )
            .await?;
            Ok(Output::Pushed(
                v["revision"].as_u64().ok_or("Немає версії бібліотеки")?,
                sent,
            ))
        }
        Action::Pull => {
            let v = request(&s, "library", "GET", None).await?;
            let revision = v["revision"].as_u64().ok_or("Немає версії бібліотеки")?;
            if revision == 0 {
                return Err("Хмарна бібліотека порожня. Збережіть дані цього пристрою.".into());
            }
            let data = serde_json::from_value(v["library"].clone())
                .map_err(|_| "Непідтримувана бібліотека".to_string())?;
            Ok(Output::Pulled(revision, data, true))
        }
        Action::Account => Ok(Output::Account(request(&s, "account", "GET", None).await?)),
        Action::Files => {
            let v = request(&s, "files", "GET", None).await?;
            Ok(Output::Files(
                serde_json::from_value(v["items"].clone())
                    .map_err(|_| "Непідтримуваний список файлів".to_string())?,
            ))
        }
        Action::Device => Ok(Output::Device(
            request(
                &Session {
                    token: String::new(),
                    ..s
                },
                "oauth/device_authorization",
                "POST",
                Some(json!({"client_id":"vey","scope":"vey"})),
            )
            .await?,
        )),
        Action::Poll(code) => {
            let v = request(
                &Session {
                    token: String::new(),
                    ..s.clone()
                },
                "oauth/token",
                "POST",
                Some(json!({"client_id":"vey","grant_type":"urn:ietf:params:oauth:grant-type:device_code","device_code":code})),
            )
            .await?;
            if matches!(
                v["error"].as_str(),
                Some("authorization_pending" | "slow_down")
            ) {
                Ok(Output::Pending(if v["error"] == "slow_down" {
                    5000
                } else {
                    0
                }))
            } else {
                Ok(Output::Session(login_result(s, v)?))
            }
        }
        Action::Logout => {
            request(&s, "auth/logout", "POST", Some(json!({}))).await?;
            Ok(Output::Logout)
        }
        Action::Upload(path) => upload_file(&s, path).await,
        Action::Check => check_library(&s).await,
        Action::Auto(paths) => {
            for path in paths {
                let stamp = fingerprint(&path);
                if !stamp.is_empty() && !s.uploaded.contains(&stamp) {
                    return upload_file(&s, path).await;
                }
            }
            if s.auto_sync && s.revision.is_some() {
                check_library(&s).await
            } else {
                Ok(Output::Unchanged)
            }
        }
        Action::Download(file) => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                tokio::select! {
                    result=download_file(&s,file)=>result,
                    _=async {while !CANCEL_UPLOAD.load(std::sync::atomic::Ordering::Relaxed){tokio::time::sleep(Duration::from_millis(100)).await;}}=>Err("Завантаження скасовано".into())
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                download_file(&s, file).await
            }
        }
    }
}
async fn check_library(s: &Session) -> Result<Output, String> {
    let v = request(s, "library", "GET", None).await?;
    let revision = v["revision"].as_u64().ok_or("Відсутня версія бібліотеки")?;
    if Some(revision) == s.revision {
        return Ok(Output::Unchanged);
    }
    let cloud = serde_json::from_value(v["library"].clone())
        .map_err(|_| "Непідтримувана бібліотека".to_string())?;
    Ok(Output::Pulled(revision, cloud, false))
}
#[cfg(not(target_arch = "wasm32"))]
fn fingerprint(path: &str) -> String {
    use sha1::{Digest, Sha1};
    let Ok(meta) = std::fs::metadata(path) else {
        return String::new();
    };
    if !meta.is_file() {
        return String::new();
    }
    let modified = meta
        .modified()
        .ok()
        .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|v| v.as_millis())
        .unwrap_or(0);
    format!(
        "{:x}",
        Sha1::digest(format!("{path}:{}:{modified}", meta.len()).as_bytes())
    )
}
#[cfg(target_arch = "wasm32")]
fn fingerprint(path: &str) -> String {
    path.to_string()
}
static TRANSFER_DONE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static TRANSFER_TOTAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CANCEL_UPLOAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(crate) fn snapshot(store: &Store) -> Store {
    Store {
        settings: store.settings.clone(),
        tracks: store.tracks.clone(),
        likes: store.likes.clone(),
        playlists: store.playlists.clone(),
        history: store.history.clone(),
        offsets: store.offsets.clone(),
        downloads: store.downloads.clone(),
        lyrics_choices: store.lyrics_choices.clone(),
        ..Store::default()
    }
}
fn load_session() -> Session {
    #[cfg(not(target_arch = "wasm32"))]
    let text =
        std::fs::read_to_string(crate::storage::data_dir().join("account-session.json")).ok();
    #[cfg(target_arch = "wasm32")]
    let text = web_sys::window()
        .and_then(|w| w.session_storage().ok().flatten())
        .and_then(|s| s.get_item("vey.account.session").ok().flatten());
    text.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
fn save_session(s: &Session) -> Result<(), String> {
    let text = serde_json::to_string(s).map_err(|_| "Не вдалося зберегти сесію".to_string())?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Write;
        let dir = crate::storage::data_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("account-session.pending.json");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&path).map_err(|e| e.to_string())?;
        f.write_all(text.as_bytes())
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(path, dir.join("account-session.json")).map_err(|e| e.to_string())?;
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        storage
            .set_item("vey.account.session", &text)
            .map_err(|_| "Не вдалося зберегти сесію".to_string())?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn upload_file(s: &Session, path: String) -> Result<Output, String> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(&path).map_err(|_| "Не вдалося відкрити аудіофайл".to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > 512 * 1024 * 1024 {
        return Err("Потрібен аудіофайл розміром до 512 MiB".into());
    }
    TRANSFER_DONE.store(0, std::sync::atomic::Ordering::Relaxed);
    TRANSFER_TOTAL.store(meta.len() as usize, std::sync::atomic::Ordering::Relaxed);
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("Некоректне ім’я файлу")?;
    let fingerprint = fingerprint(&path);
    if s.uploaded.contains(&fingerprint) {
        return Err("Цю версію файлу вже збережено у хмарі".into());
    }
    let start = request(
        s,
        "uploads",
        "POST",
        Some(json!({"name":name,"size":meta.len()})),
    )
    .await?;
    let id = start["id"]
        .as_str()
        .ok_or("Відсутній ID завантаження")?
        .to_string();
    let part_size = start["part_size"]
        .as_u64()
        .filter(|v| *v == 5 * 1024 * 1024)
        .ok_or("Непідтримуваний розмір частини")? as usize;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let result = async {
        let mut remaining = meta.len();
        let mut part = 1;
        while remaining > 0 {
            if CANCEL_UPLOAD.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("Завантаження скасовано".to_string());
            }
            let count = remaining.min(part_size as u64) as usize;
            let mut buffer = vec![0u8; count];
            file.read_exact(&mut buffer)
                .map_err(|_| "Читання аудіо перервано".to_string())?;
            let response = client
                .put(format!("{}/vey/uploads/{id}/parts/{part}", s.api))
                .bearer_auth(&s.token)
                .header("Content-Type", "application/octet-stream")
                .body(buffer)
                .send()
                .await
                .map_err(|e| e.without_url().to_string())?;
            if !response.status().is_success() {
                return Err(format!("Частину файлу відхилено ({})", response.status()));
            }
            remaining -= count as u64;
            TRANSFER_DONE.store(
                (meta.len() - remaining) as usize,
                std::sync::atomic::Ordering::Relaxed,
            );
            part += 1;
        }
        let value = request(
            s,
            &format!("uploads/{id}/complete"),
            "POST",
            Some(json!({})),
        )
        .await?;
        serde_json::from_value::<FileInfo>(value).map_err(|_| "Некоректні дані файлу".to_string())
    }
    .await;
    if result.is_err() {
        let _ = request(s, &format!("files/{id}"), "DELETE", None).await;
    }
    Ok(Output::Uploaded(result?, path, fingerprint))
}
#[cfg(target_arch = "wasm32")]
async fn upload_file(s: &Session, path: String) -> Result<Output, String> {
    let value = wasm_bindgen_futures::JsFuture::from(browser_upload(&s.api, &s.token, &path))
        .await
        .map_err(|e| {
            e.as_string()
                .unwrap_or_else(|| "Завантаження не вдалося".into())
        })?;
    let file: FileInfo = serde_json::from_str(&value.as_string().unwrap_or_default())
        .map_err(|_| "Некоректні дані файлу".to_string())?;
    Ok(Output::Uploaded(file, path.clone(), path))
}
#[cfg(not(target_arch = "wasm32"))]
struct PartialFile(std::path::PathBuf);
#[cfg(not(target_arch = "wasm32"))]
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
async fn download_file(s: &Session, file: FileInfo) -> Result<Output, String> {
    if file.id.len() != 36 || !file.id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return Err("Некоректний файл".into());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use sha1::{Digest, Sha1};
        use std::io::Write;
        let scope = format!(
            "{:x}",
            Sha1::digest(format!("{}:{}", s.api, s.id).as_bytes())
        );
        let dir = crate::storage::data_dir().join("cloud-audio").join(scope);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let ext = std::path::Path::new(&file.name)
            .extension()
            .and_then(|v| v.to_str())
            .filter(|s| s.len() <= 8 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
            .unwrap_or("audio");
        let path = dir.join(format!("{}.{}", file.id, ext));
        let pending = path.with_extension("pending");
        let _cleanup = PartialFile(pending.clone());
        TRANSFER_DONE.store(0, std::sync::atomic::Ordering::Relaxed);
        TRANSFER_TOTAL.store(file.size as usize, std::sync::atomic::Ordering::Relaxed);
        let result = async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?;
            let mut response = client
                .get(format!("{}/vey/files/{}", s.api, file.id))
                .bearer_auth(&s.token)
                .send()
                .await
                .map_err(|e| e.without_url().to_string())?
                .error_for_status()
                .map_err(|e| e.without_url().to_string())?;
            if file.size > 512 * 1024 * 1024 {
                return Err("Файл завеликий".to_string());
            }
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut output = options.open(&pending).map_err(|e| e.to_string())?;
            let mut received = 0;
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "Завантаження перервано".to_string())?
            {
                received += chunk.len() as u64;
                TRANSFER_DONE.store(received as usize, std::sync::atomic::Ordering::Relaxed);
                if received > file.size {
                    return Err("Невірний розмір файлу".into());
                }
                output.write_all(&chunk).map_err(|e| e.to_string())?;
            }
            if received != file.size {
                return Err("Неповний аудіофайл".into());
            }
            output.sync_all().map_err(|e| e.to_string())?;
            drop(output);
            std::fs::rename(&pending, &path).map_err(|e| e.to_string())?;
            Ok(path.to_string_lossy().to_string())
        }
        .await;
        if result.is_err() {
            let _ = std::fs::remove_file(&pending);
        }
        Ok(Output::Downloaded(file, result?))
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::JsFuture::from(browser_download(&s.api, &s.token, &file.id))
            .await
            .map_err(|e| {
                e.as_string()
                    .unwrap_or_else(|| "Завантаження не вдалося".into())
            })?;
        let key = format!("cloud://{}", file.id);
        Ok(Output::Downloaded(file, key))
    }
}

fn add_file(store: &mut Store, file: &FileInfo, path: String, source: Option<&str>) {
    let key = format!("cloud://{}", file.id);
    let mut track = source
        .or(Some(key.as_str()))
        .and_then(|old| store.tracks.iter().find(|t| t.path == old).cloned())
        .unwrap_or_default();
    track.path = key.clone();
    track.video_id = None;
    track.clip_video_id = None;
    if track.title.is_empty() {
        track.title = file.name.clone();
    }
    if let Some(old) = source {
        store.tracks.retain(|t| t.path != old);
        if store.likes.remove(old) {
            store.likes.insert(key.clone());
        }
        for p in &mut store.playlists {
            for t in &mut p.tracks {
                if t == old {
                    *t = key.clone();
                }
            }
        }
        for h in &mut store.history {
            if h.track.path == old {
                h.track = track.clone();
            }
        }
        if let Some(choice) = store.lyrics_choices.remove(old) {
            store.lyrics_choices.insert(key.clone(), choice);
        }
        if let Some(offset) = store.offsets.remove(old) {
            store.offsets.insert(key.clone(), offset);
        }
    }
    store.tracks.retain(|t| t.path != key);
    store.tracks.push(track);
    store.downloads.insert(key, path);
}

fn qr_image(uri: &str) -> Option<slint::Image> {
    use qrcode::{Color, EcLevel, QrCode};
    let qr = QrCode::with_error_correction_level(uri.as_bytes(), EcLevel::M).ok()?;
    let modules = qr.width();
    let quiet = 4usize;
    let scale = 4usize;
    let side = (modules + quiet * 2) * scale;
    let mut rgba = vec![255u8; side * side * 4];
    for y in 0..modules {
        for x in 0..modules {
            if qr[(x, y)] == Color::Dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let at = (((y + quiet) * scale + dy) * side + (x + quiet) * scale + dx) * 4;
                        rgba[at..at + 3].fill(0);
                    }
                }
            }
        }
    }
    Some(slint::Image::from_rgba8(slint::SharedPixelBuffer::<
        slint::Rgba8Pixel,
    >::clone_from_slice(
        &rgba, side as u32, side as u32
    )))
}
fn clear_device_ui(ui: &MainWindow) {
    ui.set_account_verification_url("".into());
    ui.set_account_verification_page("".into());
    ui.set_account_user_code("".into());
    ui.set_account_seconds_left(0);
    ui.set_account_qr(slint::Image::default());
}

pub fn install(
    w: &MainWindow,
    get: Rc<dyn Fn() -> Store>,
    apply: Rc<dyn Fn(Store)>,
    #[cfg(not(target_arch = "wasm32"))] runtime: std::sync::Arc<tokio::runtime::Runtime>,
) -> slint::Timer {
    struct State {
        session: Session,
        active: bool,
        generation: u64,
        last_snapshot: String,
        device: Option<(String, u64)>,
        open_browser: bool,
        files: Vec<FileInfo>,
        next_poll: u64,
        poll_interval: u64,
        next_sync: u64,
        next_ui: u64,
        verified: bool,
        locals: Vec<String>,
    }
    let mut session = load_session();
    if session.api.is_empty() {
        session.api = "https://blueberry-melody.superstilist.workers.dev".into();
        session.auto_sync = true;
    }
    if session.expires_at < crate::services::now_ms() {
        session.token.clear();
        session.revision = None;
    }
    w.set_account_api(session.api.clone().into());
    w.set_account_email(session.email.clone().into());
    w.set_account_name(session.name.clone().into());
    w.set_account_signed_in(!session.token.is_empty());
    w.set_account_premium(session.premium);
    w.set_account_auto_sync(session.auto_sync);
    w.set_account_auto_upload(session.auto_upload);
    let state = Rc::new(RefCell::new(State {
        session,
        active: false,
        generation: 0,
        last_snapshot: String::new(),
        device: None,
        open_browser: true,
        files: vec![],
        next_poll: 0,
        poll_interval: 5000,
        next_sync: crate::services::now_ms() + 3000,
        next_ui: 0,
        verified: false,
        locals: Vec::new(),
    }));
    let (tx, rx) = std::sync::mpsc::channel::<(u64, Result<Output, String>)>();
    let dispatch: Rc<dyn Fn(u64, Session, Action)> = {
        let tx = tx.clone();
        Rc::new(move |generation, s, action| {
            let tx = tx.clone();
            let transfer = matches!(
                &action,
                Action::Upload(_) | Action::Auto(_) | Action::Download(_)
            );
            #[cfg(not(target_arch = "wasm32"))]
            runtime.spawn(async move {
                let result = work(s, action).await;
                if transfer {
                    TRANSFER_TOTAL.store(0, std::sync::atomic::Ordering::Relaxed);
                }
                let _ = tx.send((generation, result));
            });
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let result = work(s, action).await;
                if transfer {
                    TRANSFER_TOTAL.store(0, std::sync::atomic::Ordering::Relaxed);
                }
                let _ = tx.send((generation, result));
            });
        })
    };
    let start: Rc<dyn Fn(&MainWindow, Action)> = {
        let state = state.clone();
        let dispatch = dispatch.clone();
        Rc::new(move |ui, action| {
            let mut a = state.borrow_mut();
            if a.active {
                return;
            }
            a.active = true;
            ui.set_account_busy(true);
            ui.set_account_status("З’єднання…".into());
            dispatch(a.generation, a.session.clone(), action);
        })
    };
    {
        let weak = w.as_weak();
        let state = state.clone();
        let start = start.clone();
        let get = get.clone();
        let dispatch = dispatch.clone();
        w.on_account_command(move |command| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let command = command.as_str();
            if command == "website" {
                let url = ui.get_account_verification_url().to_string();
                if !url.is_empty() {
                    open_page(&url);
                }
                return;
            }
            if command == "cancel-login" {
                let mut a = state.borrow_mut();
                a.generation += 1;
                a.device = None;
                a.active = false;
                ui.set_account_busy(false);
                clear_device_ui(&ui);
                ui.set_account_status("Вхід скасовано".into());
                close_login_page();
                return;
            }
            if command == "cancel" {
                CANCEL_UPLOAD.store(true, std::sync::atomic::Ordering::Relaxed);
                #[cfg(target_arch = "wasm32")]
                browser_cancel();
                ui.set_account_auto_upload(false);
                ui.set_account_status("Скасування завантаження…".into());
                return;
            }
            if state.borrow().active {
                return;
            }
            if command == "logout" {
                let mut a = state.borrow_mut();
                let old = a.session.clone();
                let generation = a.generation;
                a.generation += 1;
                a.session = Session {
                    api: old.api.clone(),
                    email: old.email.clone(),
                    ..Session::default()
                };
                a.device = None;
                a.files.clear();
                a.last_snapshot.clear();
                ui.set_account_signed_in(false);
                ui.set_account_premium(false);
                ui.set_account_auto_upload(false);
                clear_device_ui(&ui);
                ui.set_account_cloud_files(slint::ModelRc::default());
                ui.set_account_status("Ви вийшли з акаунта".into());
                let _ = save_session(&a.session);
                dispatch(generation, old, Action::Logout);
                return;
            }
            if matches!(command, "login" | "register" | "device" | "qr") {
                let url = match base(&ui.get_account_api()) {
                    Ok(v) => v,
                    Err(e) => {
                        ui.set_account_status(e.into());
                        return;
                    }
                };
                let mut a = state.borrow_mut();
                a.generation += 1;
                a.session = Session {
                    api: url,
                    auto_sync: ui.get_account_auto_sync(),
                    ..Session::default()
                };
                a.last_snapshot.clear();
                a.device = None;
                a.open_browser = command == "device";
                a.files.clear();
                ui.set_account_cloud_files(slint::ModelRc::default());
                ui.set_account_signed_in(false);
                ui.set_account_premium(false);
                ui.set_account_auto_upload(false);
                clear_device_ui(&ui);
            }
            let action = match command {
                "login" | "register" => {
                    let email = ui.get_account_email().to_string();
                    let password = ui.get_account_password().to_string();
                    let name = ui.get_account_name().to_string();
                    ui.set_account_password("".into());
                    Action::Login {
                        email,
                        password,
                        name,
                        register: command == "register",
                    }
                }
                "device" => {
                    if !prepare_login_page() {
                        ui.set_account_status(
                            "Дозвольте відкриття вікна входу або скористайтеся посиланням нижче."
                                .into(),
                        );
                    }
                    Action::Device
                }
                "qr" => Action::Device,
                "logout" => Action::Logout,
                "refresh" => Action::Account,
                "files" => Action::Files,
                "push" => Action::Push(CloudLibrary::capture(&get(), ui.get_account_auto_upload())),
                "pull" => Action::Pull,
                "upload" => {
                    if !state.borrow().session.premium {
                        ui.set_account_status(
                            "Потрібне запрошення Premium для тестувальника або розробника".into(),
                        );
                        return;
                    }
                    let state = state.borrow();
                    let Some(path) = state
                        .locals
                        .get(ui.get_account_local_index().max(0) as usize)
                        .cloned()
                    else {
                        ui.set_account_status("Спершу додайте локальну пісню до бібліотеки".into());
                        return;
                    };
                    Action::Upload(path)
                }
                "download" => {
                    let a = state.borrow();
                    let Some(file) = a
                        .files
                        .get(ui.get_account_cloud_index().max(0) as usize)
                        .cloned()
                    else {
                        return;
                    };
                    Action::Download(file)
                }
                _ => return,
            };
            CANCEL_UPLOAD.store(false, std::sync::atomic::Ordering::Relaxed);
            start(&ui, action);
        });
    }
    let weak = w.as_weak();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(100),move||{
        let Some(ui)=weak.upgrade()else{return;};
        let total=TRANSFER_TOTAL.load(std::sync::atomic::Ordering::Relaxed);
        if total>0 {let sent=TRANSFER_DONE.load(std::sync::atomic::Ordering::Relaxed);ui.set_account_status(format!("Завантаження аудіо · {}%",sent*100/total).into());}
        while let Ok((generation,result))=rx.try_recv(){
            let mut a=state.borrow_mut();if generation!=a.generation{continue;}a.active=false;ui.set_account_busy(false);
            match result {
                Ok(Output::Unchanged)=>{}
                Ok(Output::Session(s))=>{a.verified=false;a.session=s;ui.set_account_email(a.session.email.clone().into());ui.set_account_name(a.session.name.clone().into());a.device=None;a.last_snapshot.clear();ui.set_account_signed_in(true);ui.set_account_premium(a.session.premium);clear_device_ui(&ui);close_login_page();ui.set_account_status("Вхід виконано. Збережіть дані пристрою або відновіть хмарну бібліотеку.".into());}
                Ok(Output::Pushed(revision,sent))=>{a.session.revision=Some(revision);a.last_snapshot=sent;ui.set_account_status("Бібліотеку збережено у хмарі".into());}
                Ok(Output::Pulled(revision,cloud,force))=>{if !force && a.last_snapshot!=serde_json::to_string(&CloudLibrary::capture(&get(),ui.get_account_auto_upload())).unwrap_or_default(){a.session.auto_sync=false;ui.set_account_auto_sync(false);ui.set_account_status("Дані змінилися на обох пристроях. Виберіть зберегти або відновити бібліотеку.".into());let _=save_session(&a.session);continue;}a.session.revision=Some(revision);a.session.auto_upload=cloud.auto_upload;ui.set_account_auto_upload(cloud.auto_upload);let mut store=get();cloud.restore(&mut store);apply(store);a.last_snapshot=serde_json::to_string(&CloudLibrary::capture(&get(),a.session.auto_upload)).unwrap_or_default();ui.set_account_status("Бібліотеку й налаштування відновлено".into());}
                Ok(Output::Account(value))=>{a.verified=true;a.session.premium=value["premium"].as_bool().unwrap_or(false);ui.set_account_premium(a.session.premium);ui.set_account_status(if a.session.premium{"Premium · доступ розробника або тестувальника"}else{"Базовий акаунт · синхронізація налаштувань, улюбленого та історії"}.into());}
                Ok(Output::Files(files))=>{ui.set_account_cloud_files(Rc::new(slint::VecModel::from(files.iter().map(|f|format!("{} · {:.1} MiB",f.name,f.size as f64/1048576.).into()).collect::<Vec<slint::SharedString>>())).into());a.files=files;ui.set_account_status("Хмарні файли оновлено".into());}
                Ok(Output::Device(value))=>{
                    if let (Some(code),Some(uri),Some(public_code))=(value["device_code"].as_str(),value.get("verification_uri_complete").or_else(||value.get("verification_uri")).and_then(Value::as_str),value["user_code"].as_str()) {
                        let trusted=reqwest::Url::parse(uri).ok().zip(reqwest::Url::parse(&a.session.api).ok()).is_some_and(|(u,b)|u.origin()==b.origin()&&u.username().is_empty()&&u.password().is_none());
                        if trusted&&public_code.len()==8&&public_code.bytes().all(|b|b.is_ascii_hexdigit()) {
                            let lifetime=value["expires_in"].as_u64().unwrap_or(600).clamp(1,600);
                            a.device=Some((code.into(),crate::services::now_ms()+lifetime*1000));
                            a.poll_interval=value["interval"].as_u64().unwrap_or(5).max(5)*1000;a.next_poll=0;
                            ui.set_account_user_code(format!("{} {}",&public_code[..4],&public_code[4..]).into());
                            ui.set_account_seconds_left(lifetime as i32);
                            ui.set_account_verification_page(value["verification_uri"].as_str().unwrap_or_default().into());
                            ui.set_account_qr(qr_image(uri).unwrap_or_default());
                            ui.set_account_verification_url(uri.into());
                            if a.open_browser {open_page(uri);}
                            ui.set_account_status("Порівняй код і підтвердь вхід у своєму акаунті Vey.".into());
                        } else {clear_device_ui(&ui);close_login_page();ui.set_account_status("Сервер повернув некоректний код входу".into());}
                    } else {clear_device_ui(&ui);close_login_page();ui.set_account_status("Не вдалося створити код входу".into());}
                }
                Ok(Output::Pending(extra))=>{a.poll_interval+=extra;a.next_poll=crate::services::now_ms()+a.poll_interval;}
                Ok(Output::Logout)=>{a.generation+=1;a.session=Session{api:a.session.api.clone(),email:a.session.email.clone(),..Session::default()};a.files.clear();a.device=None;ui.set_account_signed_in(false);ui.set_account_premium(false);ui.set_account_auto_upload(false);ui.set_account_status("Ви вийшли з акаунта".into());}
                Ok(Output::Uploaded(file,path,fingerprint))=>{a.session.uploaded.insert(fingerprint);let mut store=get();add_file(&mut store,&file,path.clone(),Some(&path));apply(store);a.files.push(file);ui.set_account_status("Аудіофайл збережено у приватній хмарі".into());}
                Ok(Output::Downloaded(file,path))=>{let mut store=get();add_file(&mut store,&file,path,None);apply(store);ui.set_account_status("Пісню додано до бібліотеки пристрою".into());}
                Err(error)=>{a.device=None;if a.session.token.is_empty(){close_login_page();clear_device_ui(&ui);}if error=="Сесія завершилася. Увійдіть знову."{a.session.token.clear();a.session.revision=None;a.files.clear();ui.set_account_signed_in(false);ui.set_account_premium(false);ui.set_account_cloud_files(slint::ModelRc::default());}ui.set_account_status(error.into());if a.session.revision.is_some(){a.session.auto_sync=false;ui.set_account_auto_sync(false);}}
            }
            if let Err(error)=save_session(&a.session){ui.set_account_status(error.into());}
        }
        let mut a=state.borrow_mut();let now=crate::services::now_ms();
        let changed=a.session.auto_sync!=ui.get_account_auto_sync()||a.session.auto_upload!=ui.get_account_auto_upload();
        a.session.auto_sync=ui.get_account_auto_sync();a.session.auto_upload=ui.get_account_auto_upload();
        if changed {let _=save_session(&a.session);}
        if let Some((_,expires))=a.device.as_ref(){
            ui.set_account_seconds_left(((expires.saturating_sub(now)+999)/1000) as i32);
            if now>=*expires{a.generation+=1;a.active=false;a.device=None;ui.set_account_busy(false);clear_device_ui(&ui);close_login_page();ui.set_account_status("Код входу застарів. Почніть знову.".into());return;}
        }
        if a.active{return;}
        if let Some((code,_))=a.device.clone(){
            if now>=a.next_poll{a.next_poll=now+a.poll_interval;a.active=true;ui.set_account_busy(true);dispatch(a.generation,a.session.clone(),Action::Poll(code));}return;
        }
        if now<a.next_sync && now<a.next_ui{return;}
        let showing=ui.get_active_view()=="settings"&&ui.get_settings_section()=="account";
        if !showing&&(a.session.token.is_empty()||now<a.next_sync){a.next_ui=now+3000;return;}
        let store=get();
        if now>=a.next_ui{
            a.next_ui=now+3000;
            a.locals=store.tracks.iter().filter(|t|!cloud_key(&t.path)).map(|t|t.path.clone()).collect();
            let names=store.tracks.iter().filter(|t|!cloud_key(&t.path)).map(|t|slint::SharedString::from(t.title.clone())).collect::<Vec<_>>();
            let model=ui.get_account_local_files();
            if model.row_count()!=names.len()||names.iter().enumerate().any(|(i,n)|model.row_data(i).as_ref()!=Some(n)){ui.set_account_local_files(Rc::new(slint::VecModel::from(names)).into());}
        }
        if !a.session.token.is_empty()&&now>=a.next_sync{
            a.next_sync=now+15000;
            let current=CloudLibrary::capture(&store,a.session.auto_upload);let text=serde_json::to_string(&current).unwrap_or_default();
            let action=if !a.verified{Some(Action::Account)}
                else if a.session.revision.is_none(){None}
                else if a.session.auto_sync&&a.last_snapshot!=text{Some(Action::Push(current))}
                else if a.session.auto_upload&&a.session.premium{Some(Action::Auto(store.tracks.iter().filter(|t|!cloud_key(&t.path)).map(|t|t.path.clone()).collect()))}
                else if a.session.auto_sync{Some(Action::Check)}else{None};
            if let Some(action)=action{CANCEL_UPLOAD.store(false,std::sync::atomic::Ordering::Relaxed);a.active=true;ui.set_account_busy(true);dispatch(a.generation,a.session.clone(),action);}
        }
    });
    timer
}
fn prepare_login_page() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        browser_open_login("");
        true
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}
fn close_login_page() {
    #[cfg(target_arch = "wasm32")]
    {
        browser_open_login("#close");
    }
}
fn open_page(url: &str) {
    #[cfg(target_arch = "wasm32")]
    browser_open_login(url);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = crate::native::open_url(url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_snapshot_excludes_local_paths_and_credentials_and_preserves_settings() {
        let mut store = Store::default();
        let remote = Track {
            path: "yt://abcdefghijk".into(),
            video_id: Some("abcdefghijk".into()),
            title: "Saved song".into(),
            thumbnail_url: "C:/private/cover.jpg".into(),
            ..Track::default()
        };
        let local = Track {
            path: "C:/private/music/song.flac".into(),
            title: "Local song".into(),
            ..Track::default()
        };
        store.tracks = vec![remote.clone(), local.clone()];
        store
            .likes
            .extend([remote.path.clone(), local.path.clone()]);
        store.playlists.push(Playlist {
            id: "mixed".into(),
            name: "My playlist".into(),
            tracks: vec![remote.path.clone(), local.path.clone()],
        });
        store.history = vec![
            History {
                track: remote.clone(),
                played_at: 123,
                play_count: 4,
            },
            History {
                track: local.clone(),
                played_at: 124,
                play_count: 2,
            },
        ];
        store
            .legacy
            .insert("password".into(), json!("private-password"));
        store.sources.push(crate::services::Source {
            id: "source".into(),
            kind: "folder".into(),
            path: "C:/private/music".into(),
        });
        store.settings.volume = 0.25;
        store.settings.lyrics_translation = true;
        let mut cloud = CloudLibrary::capture(&store, false);
        let text = serde_json::to_string(&cloud).unwrap();
        assert!(!text.contains("C:/private"));
        assert!(!text.contains("private-password"));
        assert_eq!(cloud.history.len(), 1);
        assert_eq!(cloud.history[0].play_count, 4);
        assert_eq!(cloud.settings.volume, 0.25);
        assert!(cloud.settings.lyrics_translation);
        cloud.likes.clear();
        cloud.settings.volume = 0.6;
        cloud.restore(&mut store);
        assert!(store.tracks.iter().any(|t| t.path == local.path));
        assert!(store.likes.contains(&local.path));
        assert!(!store.likes.contains(&remote.path));
        assert!(store.playlists[0].tracks.contains(&local.path));
        assert_eq!(store.settings.volume, 0.6);
        assert_eq!(store.history.len(), 2);
        assert!(store.legacy.contains_key("password"));
    }
    #[test]
    fn api_url_cannot_embed_credentials_or_downgrade_https() {
        assert_eq!(
            base("https://example.test/api/v1/").unwrap(),
            "https://example.test/api/v1"
        );
        assert!(base("http://127.0.0.1:4879").is_ok());
        for bad in [
            "http://example.test",
            "https://user:pass@example.test",
            "https://example.test?token=secret",
            "javascript:alert(1)",
        ] {
            assert!(base(bad).is_err(), "{bad}");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires isolated Miniflare fixture on 127.0.0.1:4879"]
    fn native_client_worker_contract() {
        let api =
            std::env::var("VEY_FIXTURE_API").expect("set VEY_FIXTURE_API for isolated fixture");
        assert_eq!(api, "http://127.0.0.1:4879");
        let dir = std::env::current_dir()
            .unwrap()
            .join("target/account-contract");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("PLAYERVF_DATA_DIR", &dir);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initial = Session {
                api,
                auto_sync: true,
                ..Session::default()
            };
            let email = format!("rust-{}@example.test", crate::services::now_ms());
            let Output::Session(mut signed) = work(
                initial.clone(),
                Action::Login {
                    email,
                    password: "Fixture-only-password-123".into(),
                    name: "Rust fixture".into(),
                    register: true,
                },
            )
            .await
            .unwrap() else {
                panic!("session expected")
            };
            let Output::Device(device) = work(signed.clone(), Action::Device).await.unwrap() else {
                panic!("OAuth device expected")
            };
            let code = device["device_code"].as_str().unwrap().to_owned();
            assert!(matches!(
                work(signed.clone(), Action::Poll(code.clone()))
                    .await
                    .unwrap(),
                Output::Pending(0)
            ));
            request(
                &signed,
                "auth/device/approve",
                "POST",
                Some(json!({"user_code":device["user_code"]})),
            )
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(5200)).await;
            let Output::Session(oauth) = work(signed.clone(), Action::Poll(code)).await.unwrap()
            else {
                panic!("OAuth session expected")
            };
            assert_eq!(oauth.id, signed.id);
            work(oauth, Action::Logout).await.unwrap();
            let mut store = Store::default();
            let track = Track {
                path: "yt://abcdefghijk".into(),
                video_id: Some("abcdefghijk".into()),
                title: "Rust contract".into(),
                ..Track::default()
            };
            store.tracks.push(track.clone());
            store.likes.insert(track.path.clone());
            store.history.push(History {
                track: track.clone(),
                played_at: 1234,
                play_count: 7,
            });
            store.lyrics_choices.insert(
                track.path.clone(),
                crate::services::LyricsChoice {
                    provider: "fixture".into(),
                    format: "lrc".into(),
                    sync_type: crate::lyrics::LyricsSyncType::Synced,
                    fingerprint: "a".repeat(40),
                },
            );
            let expected = CloudLibrary::capture(&store, false);
            let Output::Pushed(revision, _) = work(signed.clone(), Action::Push(expected.clone()))
                .await
                .unwrap()
            else {
                panic!("push expected")
            };
            signed.revision = Some(revision);
            let Output::Pulled(_, actual, _) = work(signed.clone(), Action::Pull).await.unwrap()
            else {
                panic!("pull expected")
            };
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
            let file = dir.join("contract.mp3");
            let mut audio = vec![0u8; 5 * 1024 * 1024 + 2048];
            audio[..3].copy_from_slice(b"ID3");
            std::fs::write(&file, &audio).unwrap();
            assert!(upload_file(&signed, file.to_string_lossy().into())
                .await
                .is_err());
            let Output::Session(admin) = work(
                initial,
                Action::Login {
                    email: "admin@example.test".into(),
                    password: "Fixture-only-password-123".into(),
                    name: String::new(),
                    register: false,
                },
            )
            .await
            .unwrap() else {
                panic!("admin expected")
            };
            request(
                &admin,
                &format!("admin/entitlements/{}", signed.id),
                "PUT",
                Some(json!({"premium":true,"reason":"tester"})),
            )
            .await
            .unwrap();
            let Output::Uploaded(uploaded, _, _) =
                upload_file(&signed, file.to_string_lossy().into())
                    .await
                    .unwrap()
            else {
                panic!("upload expected")
            };
            let Output::Downloaded(_, path) =
                download_file(&signed, uploaded.clone()).await.unwrap()
            else {
                panic!("download expected")
            };
            assert_eq!(std::fs::read(&path).unwrap(), audio);
            request(&signed, &format!("files/{}", uploaded.id), "DELETE", None)
                .await
                .unwrap();
            std::fs::remove_file(path).unwrap();
            std::fs::remove_file(file).unwrap();
            work(signed.clone(), Action::Logout).await.unwrap();
            assert!(request(&signed, "account", "GET", None).await.is_err());
        });
    }
}
