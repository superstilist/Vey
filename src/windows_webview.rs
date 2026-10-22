//! Slint visibility can precede native HWND creation by an event-loop turn.
//! Child WebViews own a separate Win32 keyboard focus from Slint's text focus.
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;

type Hwnd = *mut c_void;
type SubclassProc = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;
const SHELL_FOCUS_SUBCLASS: usize = 0x56455946;

#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(
        window: Hwnd,
        callback: Option<SubclassProc>,
        id: usize,
        data: usize,
    ) -> i32;
    fn RemoveWindowSubclass(window: Hwnd, callback: Option<SubclassProc>, id: usize) -> i32;
    fn DefSubclassProc(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> isize;
}
#[link(name = "user32")]
extern "system" {
    fn GetFocus() -> Hwnd;
    fn SetFocus(window: Hwnd) -> Hwnd;
    fn IsChild(parent: Hwnd, window: Hwnd) -> i32;
}

unsafe extern "system" fn shell_focus_proc(
    window: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    _data: usize,
) -> isize {
    // Winit forwards mouse input to Slint without restoring SetFocus from a
    // child HWND. Reclaim it only on a press delivered to the shell itself;
    // clicks and typing inside lyrics/the editor remain owned by the WebView.
    if matches!(message, 0x0201 | 0x0204 | 0x0207 | 0x020b | 0x0246) && GetFocus() != window {
        SetFocus(window);
    }
    if message == 0x0082 {
        // WM_NCDESTROY
        RemoveWindowSubclass(window, Some(shell_focus_proc), id);
    }
    DefSubclassProc(window, message, wparam, lparam)
}

pub(crate) fn parent(window: &slint::Window) -> Option<slint::WindowHandle> {
    let parent = window.window_handle();
    let Ok(RawWindowHandle::Win32(handle)) = parent.window_handle().map(|handle| handle.as_raw())
    else {
        return None;
    };
    // Reusing the same callback/id is idempotent for lyrics and video hosts.
    unsafe {
        SetWindowSubclass(
            handle.hwnd.get() as Hwnd,
            Some(shell_focus_proc),
            SHELL_FOCUS_SUBCLASS,
            0,
        );
    }
    Some(parent)
}

pub(crate) fn set_visible(view: &wry::WebView, visible: bool) -> wry::Result<()> {
    if !visible {
        use wry::WebViewExtWindows;
        unsafe {
            let mut child = Default::default();
            if view.controller().ParentWindow(&mut child).is_ok() {
                let focused = GetFocus();
                // A hidden child cannot receive keyboard input. Release only
                // its own focus, never another window's or the shell's focus.
                if !focused.is_null() && (focused == child.0 || IsChild(child.0, focused) != 0) {
                    view.focus_parent()?;
                }
            }
        }
    }
    view.set_visible(visible)
}
