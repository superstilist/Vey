//! Native keyboard regression with real WebView2 video updates.
#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use base64::Engine;
    use playervf::{
        video::{install_source, VideoSource},
        MainWindow,
    };
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::ComponentHandle;
    use std::{
        cell::{Cell, RefCell},
        ffi::c_void,
        rc::Rc,
        time::{Duration, Instant},
    };
    #[link(name = "user32")]
    extern "system" {
        fn GetFocus() -> *mut c_void;
        fn GetWindow(window: *mut c_void, command: u32) -> *mut c_void;
        fn SetFocus(window: *mut c_void) -> *mut c_void;
        fn PostMessageW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> i32;
    }
    fn click(w: &MainWindow) {
        let handle = w.window().window_handle();
        let RawWindowHandle::Win32(raw) = handle.window_handle().unwrap().as_raw() else {
            panic!("expected HWND")
        };
        let scale = w.window().scale_factor();
        let position = ((32. * scale) as isize) << 16 | (450. * scale) as isize;
        let hwnd = raw.hwnd.get() as *mut c_void;
        unsafe {
            PostMessageW(hwnd, 0x200, 0, position);
            PostMessageW(hwnd, 0x201, 1, position);
            PostMessageW(hwnd, 0x202, 0, position);
        }
    }
    unsafe fn type_key(ch: char) {
        let target = GetFocus();
        let scan = 1 | (0x1e << 16);
        let key = if ch.is_ascii_alphabetic() {
            ch.to_ascii_uppercase() as usize
        } else if ch == '\r' {
            0x0d
        } else {
            0xe7
        }; // VK_PACKET for Unicode/text
        PostMessageW(target, 0x100, key, scan);
        PostMessageW(target, 0x102, ch as usize, scan);
        PostMessageW(target, 0x101, key, scan | (0xc0000000u32 as isize));
    }
    let w = MainWindow::new()?;
    playervf::i18n::install(&w);
    w.set_lyrics_reduced_motion(true);
    w.set_current_track_key("fixture".into());
    w.set_current_title("Search keyboard fixture".into());
    w.set_active_view("lyrics".into());
    w.set_art_mode("cover".into());
    let submissions = Rc::new(RefCell::new(Vec::new()));
    let submitted = submissions.clone();
    w.on_search(move |query| submitted.borrow_mut().push(query.to_string()));
    w.show()?;
    let _surface = install_source(
        &w,
        VideoSource {
            url: format!(
                "data:video/mp4;base64,{}",
                base64::engine::general_purpose::STANDARD
                    .encode(std::fs::read("target/video-surface-fixture.mp4")?)
            ),
            mime: "video/mp4".into(),
        },
    );
    let weak = w.as_weak();
    let phase = Rc::new(Cell::new(0));
    let result = phase.clone();
    let started = Instant::now();
    let cycles = Cell::new(0);
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(180),
        move || {
            let w = weak.upgrade().unwrap();
            if started.elapsed() > Duration::from_secs(40) {
                panic!("native input timed out at phase {}", phase.get());
            }
            let handle = w.window().window_handle();
            let RawWindowHandle::Win32(raw) = handle.window_handle().unwrap().as_raw() else {
                panic!("expected HWND")
            };
            let hwnd = raw.hwnd.get() as *mut c_void;
            match phase.get() {
                0 => {
                    unsafe {
                        SetFocus(hwnd);
                    }
                    click(&w);
                    phase.set(8);
                }
                8 => {
                    unsafe {
                        type_key('a');
                    }
                    phase.set(1);
                }
                1 => {
                    assert_eq!(w.get_search_query(), "a", "native input before playback");
                    println!("PASS native input before playback");
                    w.set_art_mode("clip".into());
                    w.set_player_state("playing".into());
                    phase.set(2);
                }
                2 if w.get_video_ready() => {
                    unsafe {
                        let child = GetWindow(hwnd, 5);
                        assert!(!child.is_null(), "real WebView child must exist");
                        SetFocus(child);
                        assert_ne!(GetFocus(), hwnd, "fixture must first focus the WebView");
                    }
                    click(&w);
                    w.set_search_query("".into());
                    phase.set(3);
                }
                3 => {
                    assert_eq!(
                        unsafe { GetFocus() },
                        hwnd,
                        "clicking the shell must return native keyboard focus from WebView"
                    );
                    unsafe {
                        type_key('b');
                    }
                    phase.set(4);
                }
                4 => {
                    assert_eq!(
                        w.get_search_query(),
                        "b",
                        "native input must survive video ticks"
                    );
                    println!("PASS native input during playback");
                    unsafe {
                        SetFocus(GetWindow(hwnd, 5));
                    }
                    w.set_art_mode("cover".into());
                    phase.set(5);
                }
                5 => {
                    assert_eq!(
                        unsafe { GetFocus() },
                        hwnd,
                        "hiding a focused WebView must release keyboard focus"
                    );
                    click(&w);
                    unsafe {
                        type_key('c');
                    }
                    phase.set(6);
                }
                6 => {
                    assert_eq!(
                        w.get_search_query(),
                        "bc",
                        "native input after hiding video"
                    );
                    println!("PASS native input after hiding video");
                    cycles.set(cycles.get() + 1);
                    if cycles.get() < 8 {
                        w.set_current_track_key(format!("fixture-{}", cycles.get()).into());
                        w.set_art_mode("clip".into());
                        w.set_player_state(
                            if cycles.get() % 2 == 0 {
                                "playing"
                            } else {
                                "paused"
                            }
                            .into(),
                        );
                        phase.set(2);
                    } else {
                        w.set_search_query("".into());
                        unsafe {
                            type_key('ї');
                        }
                        phase.set(9);
                    }
                }
                9 => {
                    assert_eq!(
                        w.get_search_query(),
                        "ї",
                        "Unicode input after playback cycles"
                    );
                    unsafe {
                        type_key('-');
                    }
                    phase.set(10);
                }
                10 => {
                    assert_eq!(
                        w.get_search_query(),
                        "ї-",
                        "punctuation input after playback cycles"
                    );
                    unsafe {
                        type_key('\r');
                    }
                    phase.set(11);
                }
                11 => {
                    assert_eq!(
                        &*submissions.borrow(),
                        &["ї-"],
                        "Enter submits the edited query once"
                    );
                    println!(
                        "PASS 8 playback/focus cycles, Unicode, punctuation and search submission"
                    );
                    phase.set(7);
                    slint::quit_event_loop().unwrap();
                }
                _ => {}
            }
        },
    );
    w.run()?;
    assert_eq!(result.get(), 7);
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("search_input requires Windows/WebView2");
}
