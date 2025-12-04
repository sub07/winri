use log::warn;
use windows::Win32::{
    Graphics::Gdi::{COLOR_HIGHLIGHT, GetSysColor},
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
};

use crate::{
    utils::{Position, Size, cast::FaillibleCastUtils, color::Color},
    wincall_into_result,
    window::{Window, filter::is_managed_window},
};

pub fn screen_size() -> anyhow::Result<Size> {
    Ok([
        wincall_into_result!(GetSystemMetrics(SM_CXSCREEN).try_cast()?)?,
        wincall_into_result!(GetSystemMetrics(SM_CYSCREEN).try_cast()?)?,
    ]
    .into())
}

pub fn highlight_color() -> anyhow::Result<Color> {
    wincall_into_result!(GetSysColor(COLOR_HIGHLIGHT))
        .map(Color::from_abgr_packed)
        .map(Color::without_alpha)
}

pub fn restore_windows() {
    let mut windows = Window::enumerate().unwrap_or_else(|e| {
        warn!("Could not enumerate windows to restore them: {e}");
        vec![]
    });

    windows.retain(|w| is_managed_window(*w).unwrap_or(false));

    let mut pos = Position([100, 100]);
    for window in windows {
        if let Err(err) = window.move_to(pos, [800, 600].into()) {
            warn!("Failed to move window {window:?}: {err}");
        }
        pos += 100;
    }
}
