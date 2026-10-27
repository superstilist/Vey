//! Keep native fixture timers running without taking keyboard focus.
pub fn ready(window: &slint::Window) -> bool {
    if !window.is_minimized() {
        return true;
    }
    // Production intentionally suspends lyric work while minimized. Restore
    // only this fixture's own window, leaving the user's foreground app alone.
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        #[link(name = "user32")]
        extern "system" {
            fn ShowWindow(hwnd: *mut std::ffi::c_void, command: i32) -> i32;
        }
        const SW_SHOWNOACTIVATE: i32 = 4;
        let handle = window.window_handle();
        if let RawWindowHandle::Win32(raw) = handle.window_handle().unwrap().as_raw() {
            unsafe {
                ShowWindow(raw.hwnd.get() as *mut std::ffi::c_void, SW_SHOWNOACTIVATE);
            }
        }
    }
    false
}
