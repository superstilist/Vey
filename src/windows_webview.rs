//! Slint visibility can precede native HWND creation by an event-loop turn.
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub(crate) fn parent(window: &slint::Window) -> Option<slint::WindowHandle> {
    let parent = window.window_handle();
    let ready = matches!(
        parent.window_handle().map(|handle| handle.as_raw()),
        Ok(RawWindowHandle::Win32(_))
    );
    ready.then_some(parent)
}
