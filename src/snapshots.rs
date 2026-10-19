//! Deterministic native screenshots, separate from real user data and playback.
use crate::*;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::Cell, rc::Rc, time::Duration};
fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::from(Rc::new(VecModel::from(v)))
}
pub fn schedule<T>(w: &MainWindow, _: T) {
    // Optional real artwork for local visual QA; never read user data by default.
    let artwork: Vec<slint::Image> = std::env::var_os("PLAYERVF_SCREENSHOT_ART_DIR")
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .map(|files| {
            let mut files: Vec<_> = files.flatten().map(|f| f.path()).collect();
            files.sort();
            files
                .iter()
                .filter_map(|p| slint::Image::load_from_path(p).ok())
                .take(12)
                .collect()
        })
        .unwrap_or_default();
    if let Some(art) = artwork.first() {
        w.set_current_art(art.clone());
    }
    w.set_position(107.7);
    w.set_lyrics_position(107700);
    w.set_duration(223.);
    w.set_time_label("1:47 / 3:43".into());
    let tracks = [
        ("Bang Bang Bang", "BBpanzu"),
        ("Монополія", "Монеточка"),
        ("Toxic Gossip Train (feat. Kasane Teto)", "Snappydoodle"),
        ("Bumble Bee", "Bambee"),
        ("Remember son, dying is gay meme…", "NemTom"),
        ("WORLDWIDE ROCKSTARS", "MonochroMenace"),
        ("Run Rabbit (Russian Cover)", "VOL’ume"),
        ("次元通信", "AnythingBecomeMoe"),
        ("私の歯医者さん - My Dentist", "Yukopi"),
        ("О Житомир це не Місто", "Ponchik"),
        ("Blue Bird (Ukrainian Version)", "Nick Vershynin"),
        ("Василина (feat. Іван Попович)", "DZIDZIO"),
    ];
    let rows = tracks
        .iter()
        .enumerate()
        .map(|(i, (title, artist))| TrackRow {
            title: (*title).into(),
            artist: (*artist).into(),
            album: (*title).into(),
            duration: "3:24".into(),
            path: format!("fixture://{i}").into(),
            art: artwork
                .get(i % artwork.len().max(1))
                .cloned()
                .unwrap_or_default(),
            ..Default::default()
        })
        .collect();
    w.set_tracks(model(rows));
    w.set_playlists(model(vec![
        PlaylistRow {
            id: "liked".into(),
            name: "Музика, яка сподобалась".into(),
            detail: "Створено автоматично".into(),
        },
        PlaylistRow {
            id: "ss".into(),
            name: "ss".into(),
            detail: "11 композицій".into(),
        },
        PlaylistRow {
            id: "ds".into(),
            name: "ds".into(),
            detail: "Локальний список".into(),
        },
    ]));
    w.set_lyric_lines(model(
        [
            ("Поховалися стежки", "", "The paths have hidden away"),
            (
                "Знов не знаю, куди йти",
                "(куди йти…)",
                "Again I don't know where to go",
            ),
            (
                "Все навколо є не тим",
                "",
                "Everything around is not the same",
            ),
            (
                "Але музика лишається",
                "(лишається з нами)",
                "But the music remains",
            ),
            ("Знову місто засинає", "", "The city falls asleep again"),
            ("Ми залишимось на хвилі", "", "We will stay on the wave"),
            ("І нехай звучить ця пісня", "", "And let this song play on"),
        ]
        .iter()
        .enumerate()
        .map(|(i, (t, bg, tr))| LyricRow {
            start_ms: 105000 + i as i32 * 2000,
            end_ms: 107000 + i as i32 * 2000,
            words: model(
                t.split_inclusive(' ')
                    .enumerate()
                    .map(|(wi, word)| WordRow {
                        text: word.into(),
                        start_ms: 105000 + i as i32 * 2000 + wi as i32 * 300,
                        end_ms: 105300 + i as i32 * 2000 + wi as i32 * 300,
                        background: false,
                        ..Default::default()
                    })
                    .chain((!bg.is_empty()).then(|| WordRow {
                        text: (*bg).into(),
                        start_ms: 105000 + i as i32 * 2000,
                        end_ms: 107000 + i as i32 * 2000,
                        background: true,
                        ..Default::default()
                    }))
                    .collect(),
            ),
            text: (*t).into(),
            background_text: (*bg).into(),
            translation: (*tr).into(),
            romanization: if i == 1 {
                "Znov ne znaiu, kudy yty".into()
            } else {
                "".into()
            },
            ..Default::default()
        })
        .collect(),
    ));
    w.set_active_lyric_index(1);
    w.set_providers(model(vec![
        "Musixmatch · word".into(),
        "LRCLib · synced".into(),
    ]));
    w.set_lyrics_status("Musixmatch · синхронізовані слова".into());
    let shots = vec![
        ("01-home", "home", 1440, 900),
        ("03-explore", "explore", 1440, 900),
        ("04-library", "library", 1440, 900),
        ("05-liked", "liked", 1440, 900),
        ("06-search", "search", 1440, 900),
        ("08-settings", "settings", 1440, 900),
        ("09-playlist", "playlist", 1440, 900),
        ("10-nowplaying", "lyrics", 1440, 900),
        ("11-nowplaying-standard", "lyrics", 1440, 900),
        ("12-nowplaying-tools", "lyrics", 1440, 900),
        ("13-lyrics-stage", "lyrics", 1440, 900),
        ("14-lyrics-instrumental", "lyrics", 1440, 900),
        ("15-lyrics-editor", "lyrics", 1440, 900),
        ("19-mobile-home", "home", 390, 844),
        ("20-mobile-library", "library", 390, 844),
        ("21-mobile-settings", "settings", 390, 844),
        ("27-mobile-lyrics", "lyrics", 390, 844),
        ("25-tablet-home", "home", 900, 900),
    ];
    let preview = std::env::args().any(|a| a == "--widget-preview");
    let shots = if preview {
        vec![("11-nowplaying-standard", "lyrics", 1440, 900)]
    } else {
        shots
    };
    let fixture_lyrics = w.get_lyric_lines();
    let weak = w.as_weak();
    let index = Rc::new(Cell::new(0usize));
    let tick: Rc<RefCell<Option<slint::Timer>>> = Rc::new(RefCell::new(None));
    let keep = tick.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(600),
        move || {
            let Some(w) = weak.upgrade() else {
                return;
            };
            let i = index.get();
            let phase = i % 2;
            let idx = i / 2;
            if idx >= shots.len() {
                keep.borrow_mut().take();
                if !preview {
                    let _ = slint::quit_event_loop();
                }
                return;
            }
            let (name, view, width, height) = shots[idx];
            if phase == 0 {
                w.window().set_fullscreen(false);
                w.window().set_maximized(false);
                w.set_mobile(width < 800);
                w.set_active_view(view.into());
                w.set_dialog(if name.contains("editor") {
                    "paste".into()
                } else {
                    "".into()
                });
                if name.contains("editor") {
                    w.set_lyrics_paste(
                        "[00:01.00]<00:01.00>Hello <00:02.00>world\n[00:03.00]Another line".into(),
                    );
                }
                w.set_focus_player(name == "10-nowplaying");
                w.set_player_tools(name.contains("tools"));
                w.set_lyrics_stage(name.contains("stage"));
                w.set_lyric_lines(if name.contains("instrumental") {
                    model(vec![LyricRow {
                        start_ms: 105000,
                        end_ms: 115000,
                        instrumental: true,
                        ..Default::default()
                    }])
                } else {
                    fixture_lyrics.clone()
                });
                if view == "lyrics" {
                    w.set_current_title("Монополія".into());
                    w.set_current_artist("Монеточка".into());
                    w.set_current_album("Молитвы. Анекдоты. Тосты.".into());
                }
                // Apply the viewport after changing the layout. Preferred-size
                // recalculation must not replace the requested mobile viewport.
                w.window()
                    .set_size(slint::LogicalSize::new(width as f32, height as f32));
            } else {
                match w.window().take_snapshot() {
                    Ok(buffer) => {
                        let dir = std::path::Path::new("docs/screenshots");
                        let _ = std::fs::create_dir_all(dir);
                        let path = dir.join(format!("{name}.png"));
                        if let Err(e) = image::save_buffer(
                            path,
                            buffer.as_bytes(),
                            buffer.width(),
                            buffer.height(),
                            image::ColorType::Rgba8,
                        ) {
                            eprintln!("Screenshot failed: {e}");
                        }
                    }
                    Err(e) => eprintln!("Screenshot failed: {e}"),
                }
            }
            index.set(i + 1);
        },
    );
    *tick.borrow_mut() = Some(timer);
}
use std::cell::RefCell;
