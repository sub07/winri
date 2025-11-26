use log::{error, warn};
use windows::Win32::{
    Graphics::Gdi::{COLOR_HIGHLIGHT, GetSysColor},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_LWIN, VK_MENU,
            VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
    },
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

pub fn highlight_color() -> Color {
    Color::from_abgr_packed(unsafe { GetSysColor(COLOR_HIGHLIGHT) })
}

#[must_use]
pub fn current_modifiers() -> keyboard_types::Modifiers {
    fn is_vkey_down(key: VIRTUAL_KEY) -> bool {
        let key_state = match wincall_into_result!(GetKeyState(key.0.into())) {
            Ok(res) => u16::from_ne_bytes(res.to_ne_bytes()),
            Err(e) => {
                error!(
                    "Error while checking if {key:?} modifier is pressed, defaulting to not pressed: {e}"
                );
                0
            }
        };
        key_state & 0xFF80 == 0xFF80
    }

    let mut modifiers = keyboard_types::Modifiers::empty();

    if is_vkey_down(VK_SHIFT) || is_vkey_down(VK_LSHIFT) {
        modifiers.insert(keyboard_types::Modifiers::SHIFT);
    }

    if is_vkey_down(VK_CONTROL) || is_vkey_down(VK_LCONTROL) {
        modifiers.insert(keyboard_types::Modifiers::CONTROL);
    }

    if is_vkey_down(VK_LWIN) || is_vkey_down(VK_RWIN) {
        modifiers.insert(keyboard_types::Modifiers::META);
    }

    if is_vkey_down(VK_MENU) {
        modifiers.insert(keyboard_types::Modifiers::ALT);
    }

    modifiers
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
