//! YouTube Music browser connection plus public playlist URL imports.
use crate::{
    library_import::{self as import, ImportedPlaylist, Provider},
    services::{now_ms, Store},
    MainWindow,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock, RwLock,
    },
    time::Duration,
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Connection {
    browser_target: String,
    auto: bool,
    owner: String,
    label: String,
    baselines: BTreeMap<String, String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Sessions {
    youtube: Connection,
}

// Only the Vey-owned browser target and expected owner cross into the Home
// fetch worker. Bump the generation on every connection change so a late
// response from an old account cannot replace the current feed.
static HOME_CONNECTION: OnceLock<RwLock<(u64, Option<(String, String)>)>> = OnceLock::new();
pub(crate) fn home_connection() -> (u64, Option<(String, String)>) {
    HOME_CONNECTION.get_or_init(|| RwLock::new((0, None))).read()
        .map(|value| value.clone()).unwrap_or_default()
}
fn set_home_connection(connection: &Connection) {
    if let Ok(mut value) = HOME_CONNECTION.get_or_init(|| RwLock::new((0, None))).write() {
        value.0 = value.0.wrapping_add(1);
        value.1 = (!connection.browser_target.is_empty() && !connection.owner.is_empty())
            .then(|| (connection.browser_target.clone(), connection.owner.clone()));
    }
}

fn save(sessions: &Sessions) -> Result<(), String> {
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
    file.write_all(&serde_json::to_vec(sessions).map_err(|_| "Помилка формату підключення")?)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Не вдалося зберегти підключення")?;
    drop(file);
    std::fs::rename(path, dir.join("music-accounts.json"))
        .map_err(|_| "Не вдалося оновити підключення".into())
}

struct SessionWriter {
    sender: Option<std::sync::mpsc::Sender<Sessions>>,
    errors: std::sync::mpsc::Receiver<String>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl SessionWriter {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<Sessions>();
        let (error_tx, errors) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            while let Ok(mut sessions) = receiver.recv() {
                while let Ok(newer) = receiver.try_recv() { sessions = newer; }
                if let Err(error) = save(&sessions) { let _ = error_tx.send(error); }
            }
        });
        Self { sender: Some(sender), errors, worker: Some(worker) }
    }
    fn request(&self, sessions: &Sessions) -> Result<(), String> {
        self.sender.as_ref().ok_or("Збереження підключень зупинено")?
            .send(sessions.clone()).map_err(|_| "Не вдалося зберегти підключення".into())
    }
}
impl Drop for SessionWriter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

/// Read the current YTM browser connection and remove the retired Spotify OAuth
/// record from older installations. The account file is rewritten without the
/// old tokens; imported playlists and the Spotify-to-YTM ID cache are separate.
fn load() -> (Sessions, Option<String>) {
    let bytes = match std::fs::read(crate::storage::data_dir().join("music-accounts.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (Sessions::default(), None)
        }
        Err(_) => return (
            Sessions::default(),
            Some(
                "Не вдалося прочитати локальні підключення; перевірте доступ до папки даних Vey."
                    .into(),
            ),
        ),
    };
    let raw: Value =
        match serde_json::from_slice(&bytes) {
            Ok(raw) => raw,
            Err(_) => return (
                Sessions::default(),
                Some(
                    "Не вдалося перевірити локальні підключення; старий файл залишився без змін."
                        .into(),
                ),
            ),
        };
    let had_spotify_account = raw.get("spotify").is_some();
    if had_spotify_account {
        crate::spotify_import::migrate_legacy_matches(&raw);
    }
    let sessions =
        match serde_json::from_value(raw) {
            Ok(sessions) => sessions,
            Err(_) => return (
                Sessions::default(),
                Some(
                    "Формат локальних підключень не підтримується; старий файл залишився без змін."
                        .into(),
                ),
            ),
        };
    if had_spotify_account {
        let status = save(&sessions).err().map(|_| {
            "Не вдалося очистити застарілі дані Spotify з локального сховища. Перевірте доступ до папки даних Vey.".to_owned()
        });
        (sessions, status)
    } else {
        (sessions, None)
    }
}

enum Job {
    Connect,
    Public(String),
    Pull(Option<String>),
}

#[derive(Clone)]
struct Output {
    connection: Connection,
    playlists: Vec<ImportedPlaylist>,
    errors: Vec<String>,
    connected: bool,
    provider_name: String,
}

enum WorkResult {
    Fetched(Result<Output, String>),
    Prepared {
        original: Output,
        output: Output,
        store: Store,
        store_revision: u64,
        changed: bool,
        imported_count: usize,
        track_count: usize,
    },
}

fn prepare_import(mut store: Store, original: Output, store_revision: u64) -> WorkResult {
    let mut output = original.clone();
    let track_count = output.playlists.iter().map(|playlist| playlist.tracks.len()).sum();
    let mut imported_count = 0;
    let mut changed = false;
    for mut playlist in output.playlists.iter().cloned() {
        import::reuse_legacy_spotify_identity(&store, &mut playlist);
        match import::apply(&mut store, &playlist, output.connection.baselines.get(&playlist.id).map(String::as_str)) {
            Ok(hash) => {
                output.connection.baselines.insert(playlist.id, hash);
                imported_count += 1;
                changed = true;
            }
            Err(error) => output.errors.push(error),
        }
    }
    WorkResult::Prepared { original, output, store, store_revision, changed, imported_count, track_count }
}

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Скасовано".into())
    } else {
        Ok(())
    }
}

async fn work(connection: Connection, job: Job, cancel: Arc<AtomicBool>) -> Result<Output, String> {
    match job {
        Job::Public(link) => {
            let (provider, id) = import::playlist_link(&link)?;
            let provider_name = provider.name().to_owned();
            let playlists = match crate::native::import_public_playlist(link).await {
                Ok(playlist) => vec![playlist],
                Err(public_error)
                    if provider == Provider::Youtube && !connection.browser_target.is_empty() =>
                {
                    let library = crate::youtube_browser::pull(
                        &connection.browser_target,
                        &connection.owner,
                        Some(id),
                        &cancel,
                    )
                    .await?;
                    if !library.errors.is_empty() {
                        return Err(library.errors.join(" · "));
                    }
                    if library.playlists.is_empty() {
                        return Err(public_error);
                    }
                    library.playlists
                }
                Err(error) => return Err(error),
            };
            cancelled(&cancel)?;
            Ok(Output {
                connection,
                playlists,
                errors: vec![],
                connected: false,
                provider_name,
            })
        }
        Job::Connect => {
            let signed = crate::youtube_browser::connect(&cancel).await?;
            let auto = signed.identity.owner == connection.owner && connection.auto;
            Ok(Output {
                connection: Connection {
                    browser_target: signed.target,
                    owner: signed.identity.owner,
                    label: signed.identity.label,
                    auto,
                    ..Default::default()
                },
                playlists: vec![],
                errors: vec![],
                connected: true,
                provider_name: Provider::Youtube.name().into(),
            })
        }
        Job::Pull(id) => {
            let library = crate::youtube_browser::pull(
                &connection.browser_target,
                &connection.owner,
                id,
                &cancel,
            )
            .await?;
            Ok(Output {
                connection,
                playlists: library.playlists,
                errors: library.errors,
                connected: false,
                provider_name: Provider::Youtube.name().into(),
            })
        }
    }
}

pub fn install(
    w: &MainWindow,
    get: Rc<dyn Fn() -> Store>,
    apply: Rc<dyn Fn(Store)>,
    store_revision: Rc<dyn Fn() -> u64>,
    runtime: Arc<tokio::runtime::Runtime>,
    on_connection_change: Rc<dyn Fn()>,
) -> slint::Timer {
    struct State {
        sessions: Sessions,
        busy: bool,
        generation: u64,
        cancel: Arc<AtomicBool>,
        next: u64,
        pending_sync: bool,
        task: Option<tokio::task::JoinHandle<()>>,
    }

    let (sessions, migration_status) = load();
    set_home_connection(&sessions.youtube);
    w.set_music_supported(!cfg!(target_os = "android"));
    w.set_music_auto_sync(sessions.youtube.auto);
    w.set_youtube_account(sessions.youtube.label.clone().into());
    if let Some(status) = migration_status {
        w.set_music_status(status.into());
    }
    let state = Rc::new(RefCell::new(State {
        sessions,
        busy: false,
        generation: 0,
        cancel: Arc::new(AtomicBool::new(false)),
        next: now_ms() + 3000,
        pending_sync: false,
        task: None,
    }));
    let writer = Rc::new(SessionWriter::new());
    let (tx, rx) = std::sync::mpsc::channel();
    let prepare_tx = tx.clone();
    let prepare = {
        let runtime = runtime.clone();
        Rc::new(move |generation, revision, store, output| {
            let tx = prepare_tx.clone();
            runtime.spawn_blocking(move || {
                let prepared = prepare_import(store, output, revision);
                let _ = tx.send((generation, prepared));
            })
        })
    };
    let send = {
        let runtime = runtime.clone();
        Rc::new(move |generation, connection, job, cancel| {
            let tx = tx.clone();
            runtime.spawn(async move {
                let result = work(connection, job, cancel).await;
                let _ = tx.send((generation, WorkResult::Fetched(result)));
            })
        })
    };
    let weak = w.as_weak();
    let shared = state.clone();
    let dispatch = send.clone();
    let command_writer = writer.clone();
    let connection_changed = on_connection_change.clone();
    w.on_music_command(move |command| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let mut state = shared.borrow_mut();
        if command == "youtube-browser-login" {
            ui.set_music_status(match crate::native::open_ytm_login() {
                Ok(()) => "Вхід YouTube Music відкрито у типовому браузері. Це не підключає автоматичну синхронізацію Vey.".into(),
                Err(error) => error.into(),
            });
            return;
        }
        if command == "select" {
            ui.set_music_auto_sync(state.sessions.youtube.auto);
            ui.set_music_status("".into());
            return;
        }
        if command == "cancel" || command == "disconnect" {
            state.cancel.store(true, Ordering::Relaxed);
            if let Some(task) = state.task.take() {
                task.abort();
            }
            state.generation += 1;
            state.busy = false;
            state.pending_sync = false;
            ui.set_music_busy(false);
            if command == "disconnect" {
                state.sessions.youtube = Connection::default();
                set_home_connection(&state.sessions.youtube);
                connection_changed();
                ui.set_youtube_account("".into());
                if let Err(error) = command_writer.request(&state.sessions) {
                    ui.set_music_status(error.into());
                    return;
                }
            }
            ui.set_music_status(
                if command == "cancel" {
                    "Скасовано"
                } else {
                    "YouTube Music відключено. Імпортовані плейлісти збережено."
                }
                .into(),
            );
            return;
        }
        if command == "auto" {
            state.sessions.youtube.auto = ui.get_music_auto_sync();
            state.next = now_ms() + 900000;
            if let Err(error) = command_writer.request(&state.sessions) {
                ui.set_music_status(error.into());
            }
            return;
        }
        if command == "sync-all" {
            state.pending_sync = true;
            ui.set_music_status("Оновлення підключеного YouTube Music акаунта…".into());
            return;
        }
        if state.busy {
            return;
        }
        let (job, provider_name) = match command.as_str() {
            "connect" if ui.get_connection_provider() == 1 => {
                (Job::Connect, Provider::Youtube.name())
            }
            "connect" => {
                ui.set_music_status("Виберіть YouTube Music, щоб підключити його бібліотеку.".into());
                return;
            }
            "sync" if ui.get_connection_provider() == 1 => {
                (Job::Pull(None), Provider::Youtube.name())
            }
            "sync" => {
                ui.set_music_status("Спершу виберіть YouTube Music.".into());
                return;
            }
            "import" => match import::playlist_link(&ui.get_music_link()) {
                Ok((provider, _)) => (
                    Job::Public(ui.get_music_link().to_string()),
                    provider.name(),
                ),
                Err(error) => {
                    ui.set_music_status(error.into());
                    return;
                }
            },
            _ => return,
        };
        state.cancel = Arc::new(AtomicBool::new(false));
        state.busy = true;
        state.generation += 1;
        ui.set_music_busy(true);
        ui.set_music_status(
            format!(
                "{provider_name} · {}",
                if matches!(job, Job::Connect) {
                    "Завершіть вхід у браузері…"
                } else {
                    "Імпорт або синхронізація…"
                }
            )
            .into(),
        );
        state.task = Some(dispatch(
            state.generation,
            state.sessions.youtube.clone(),
            job,
            state.cancel.clone(),
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
            if let Ok(error) = writer.errors.try_recv() { ui.set_music_status(error.into()); }
            let mut state = state.borrow_mut();
            while let Ok((generation, result)) = rx.try_recv() {
                if generation != state.generation {
                    continue;
                }
                match result {
                    WorkResult::Fetched(Err(error)) => {
                        state.busy = false;
                        state.task = None;
                        ui.set_music_busy(false);
                        ui.set_music_status(error.into());
                    }
                    WorkResult::Fetched(Ok(mut output)) => {
                        if output.connection.owner == state.sessions.youtube.owner
                            && output.connection.baselines.is_empty()
                        {
                            output.connection.baselines =
                                state.sessions.youtube.baselines.clone();
                        }
                        state.task = Some(prepare(generation, store_revision(), get(), output));
                    }
                    WorkResult::Prepared { original, mut output, store, store_revision: base_revision, changed, imported_count, track_count } => {
                        if store_revision() != base_revision {
                            // Rebase the same fetched playlists on the latest store.
                            // Never replace edits made while preparation was running.
                            state.task = Some(prepare(generation, store_revision(), get(), original));
                            continue;
                        }
                        state.busy = false;
                        state.task = None;
                        ui.set_music_busy(false);
                        if output.connected && output.connection.auto {
                            state.pending_sync = true;
                        }
                        let provider_name = output.provider_name.clone();
                        if changed {
                            apply(store);
                        }
                        state.sessions.youtube = output.connection;
                        if output.connected {
                            set_home_connection(&state.sessions.youtube);
                            on_connection_change();
                        }
                        if let Err(error) = writer.request(&state.sessions) {
                            output.errors.push(error);
                        }
                        ui.set_youtube_account(state.sessions.youtube.label.clone().into());
                        ui.set_music_auto_sync(state.sessions.youtube.auto);
                        let message = if !output.errors.is_empty() {
                            format!(
                                "{provider_name} · імпортовано плейлістів: {imported_count}. {}",
                                output.errors.join(" · ")
                            )
                        } else if output.connected {
                            "YouTube Music підключено: його добірки доповнюють Головну. Синхронізація плейлістів вмикається окремо.".into()
                        } else if imported_count > 0 {
                            format!("{provider_name} · імпортовано плейлістів: {imported_count}; треків: {track_count}.")
                        } else {
                            "YouTube Music синхронізовано. Нових плейлістів немає.".into()
                        };
                        ui.set_music_status(message.chars().take(2000).collect::<String>().into());
                    }
                }
            }
            if !state.busy && now_ms() >= state.next {
                state.next = now_ms() + 900000;
                state.pending_sync = state.sessions.youtube.auto;
            }
            if !state.busy && state.pending_sync {
                state.pending_sync = false;
                if !state.sessions.youtube.browser_target.is_empty() {
                    state.busy = true;
                    state.generation += 1;
                    state.cancel = Arc::new(AtomicBool::new(false));
                    ui.set_music_busy(true);
                    state.task = Some(send(
                        state.generation,
                        state.sessions.youtube.clone(),
                        Job::Pull(None),
                        state.cancel.clone(),
                    ));
                }
            }
        },
    );
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Track;

    #[test]
    fn playlist_preparation_can_rebase_on_newer_library_without_losing_edits() {
        let source = Output {
            connection: Connection::default(),
            playlists: vec![ImportedPlaylist { id: "youtube-fixture".into(), name: "Imported".into(),
                tracks: vec![Track { path: "yt://abcdefghijk".into(), title: "Song".into(), ..Default::default() }] }],
            errors: Vec::new(), connected: false, provider_name: "YouTube Music".into(),
        };
        let WorkResult::Prepared { original, store: first, changed, imported_count, store_revision, .. } =
            prepare_import(Store::default(), source, 1) else { panic!("prepared output expected") };
        assert!(changed);
        assert_eq!((imported_count, store_revision), (1, 1));
        assert_eq!(first.playlists.len(), 1);
        let local = Track { path: "C:/Music/new.flac".into(), title: "New local song".into(), ..Default::default() };
        let newer = Store { tracks: vec![local.clone()], ..Default::default() };
        let WorkResult::Prepared { store: rebased, changed, store_revision, .. } =
            prepare_import(newer, original, 2) else { panic!("prepared output expected") };
        assert!(changed);
        assert_eq!(store_revision, 2);
        assert!(rebased.tracks.iter().any(|track| track.path == local.path));
        assert_eq!(rebased.playlists[0].tracks, ["yt://abcdefghijk"]);
    }
}
