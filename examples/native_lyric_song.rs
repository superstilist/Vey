//! Replay supplied lyrics without network/user-state writes; optional local audio.
#[path = "support/native_window.rs"]
mod native_window;
use playervf::{braccato, lyrics::LyricsAlternative, MainWindow};
use slint::{ComponentHandle, Model, VecModel};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        return Err("Usage: native_lyric_song ALTERNATIVE.json OUTPUT_DIRECTORY [--sampled-audio] [--assert-smooth] [--confirm-seek] [--narrow] [--audio=WAV_PATH]".into());
    }
    let alt: LyricsAlternative = serde_json::from_str(&std::fs::read_to_string(&args[1])?)?;
    let output = std::path::PathBuf::from(&args[2]);
    if output.join("trace.json").exists() {
        return Err("Trace already exists; choose a fresh output directory".into());
    }
    let sampled_audio = args.iter().any(|a| a == "--sampled-audio");
    let confirm_seek = args.iter().any(|a| a == "--confirm-seek");
    let assert_smooth = args.iter().any(|a| a == "--assert-smooth");
    let start = alt
        .lyrics
        .lines
        .first()
        .map_or(0., |line| (line.start_ms as f64 / 1000. - 0.317).max(0.));
    let audio = if let Some(path) = args.iter().find_map(|a| a.strip_prefix("--audio=")) {
        let audio = Rc::new(playervf::native::start_audio());
        playervf::native::player_load(&audio, path.into(), false)?;
        playervf::native::player_volume(&audio, 0.)?;
        playervf::native::player_seek(&audio, start)?;
        Some(audio)
    } else {
        None
    };
    let cleanup_audio = audio.clone();
    std::fs::create_dir_all(&output)?;
    let w = MainWindow::new()?;
    playervf::native_lyrics::install(&w);
    w.set_active_view("lyrics".into());
    w.set_lyrics_timed(true);
    w.set_duration(300.);
    w.set_position(start as f32);
    w.window().set_size(slint::LogicalSize::new(
        if args.iter().any(|a| a == "--narrow") {
            1100.
        } else {
            1440.
        },
        900.,
    ));
    let rows = braccato::rows(&alt, 300000);
    w.set_lyrics_word_timed(rows.iter().any(|r| r.words.row_count() > 0));
    w.set_lyric_lines(Rc::new(VecModel::from(rows)).into());
    let samples = Rc::new(RefCell::new(vec![]));
    let collected = samples.clone();
    let weak = w.as_weak();
    let mut began = None;
    let origin = Instant::now();
    let mut sent = 0u128;
    let mut last_model = w.get_native_lyric_lines();
    let mut media_shift = 0.;
    let mut seek_requested = false;
    let mut audio_seek_serial = 0;
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(16),move||{
        let w=weak.upgrade().unwrap();if !native_window::ready(w.window()){return;}
        let Some(at)=began else{if origin.elapsed().as_millis()>1200 {
            if let Some(audio)=&audio {
                audio_seek_serial=playervf::native::player_status(audio).unwrap().seek_serial;
                playervf::native::player_play(audio).unwrap();
            }
            last_model=w.get_native_lyric_lines();began=Some(Instant::now());w.set_player_state("playing".into());}return;};
        let wall=Instant::now().duration_since(at).as_millis();
        let poll = if sampled_audio || audio.is_some() {112} else {100};
        let media_wall=if sampled_audio {wall/100*100} else {wall};
        if wall/poll>sent {
            sent=wall/poll;
            if let Some(audio)=&audio {
                let status=playervf::native::player_status(audio).unwrap();
                if status.seek_serial!=audio_seek_serial {
                    audio_seek_serial=status.seek_serial;
                    w.set_native_seek_revision(w.get_native_seek_revision().wrapping_add(1));
                }
                w.set_position(status.position_secs as f32);
            } else { w.set_position((start+media_wall as f64/1000.+media_shift) as f32); }
        }
        if confirm_seek && !seek_requested && wall>=4700 {
            let target=(w.get_lyrics_position()-25).max(0) as f64/1000.;
            media_shift=target-start-media_wall as f64/1000.;
            if let Some(audio)=&audio {
                playervf::native::player_seek(audio, target).unwrap();
            } else {
                w.set_position(target as f32);
                w.set_native_seek_revision(w.get_native_seek_revision().wrapping_add(1));
            }
            seek_requested=true;
        }
        let model=w.get_native_lyric_lines();let rebuilt=model!=last_model;last_model=model;
        collected.borrow_mut().push(serde_json::json!({"wall":wall,"ms":w.get_lyrics_position(),"y":w.get_native_content_y(),"camera":w.get_native_camera_position(),"target":w.get_native_scroll_position(),"native":w.get_native_active_line(),"anchor":w.get_active_lyric_index(),"snap":w.get_native_scroll_snap(),"rebuild":rebuilt,"driven":w.get_native_camera_driven(),"width":w.get_lyrics_widget_width(),"seek_revision":w.get_native_seek_revision(),"render_start":w.get_native_render_start(),"render_end":w.get_native_render_end()}));
        if wall>=7500 {slint::quit_event_loop().unwrap();}
    });
    w.run()?;
    if let Some(audio) = cleanup_audio {
        playervf::native::player_stop(&audio)?;
    }
    std::fs::write(
        output.join("trace.json"),
        serde_json::to_vec_pretty(&*samples.borrow())?,
    )?;
    if assert_smooth {
        let samples = samples.borrow();
        for pair in samples
            .windows(2)
            .filter(|pair| pair[1]["seek_revision"] == 0)
        {
            assert!(
                pair[1]["ms"].as_i64().unwrap() >= pair[0]["ms"].as_i64().unwrap(),
                "audio polling rewound the lyric clock"
            );
            assert_eq!(
                pair[1]["snap"], false,
                "ordinary audio polling cancelled the camera"
            );
            assert_eq!(
                pair[1]["rebuild"], false,
                "cue handoff rebuilt the lyric document"
            );
        }
        if confirm_seek {
            assert!(
                samples.iter().any(|s| s["seek_revision"] == 1
                    && s["snap"] == true
                    && (s["camera"].as_f64().unwrap() - s["target"].as_f64().unwrap()).abs() < 1.),
                "confirmed small seek did not snap exactly"
            );
        }
    }
    for pair in samples.borrow().windows(2) {
        let a = &pair[0];
        let b = &pair[1];
        let target_changed = a["target"] != b["target"];
        let jump = (b["y"].as_f64().unwrap() - a["y"].as_f64().unwrap()).abs();
        if target_changed || jump > 40. || b["snap"].as_bool().unwrap() {
            println!("{b}");
        }
    }
    Ok(())
}
