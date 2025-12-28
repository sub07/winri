use log::warn;
use windows::Win32::{
    Graphics::Gdi::{COLOR_HIGHLIGHT, GetSysColor},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_LWIN, VK_MENU,
            VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{GetDesktopWindow, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
    },
};

use crate::{
    utils::math::{Position, Size},
    wincall_into_result,
    window::{self, Window},
};

pub fn screen_size() -> anyhow::Result<Size> {
    #[allow(
        clippy::cast_precision_loss,
        reason = "The values will stay within screen size orders of magnitude"
    )]
    Ok(Size([
        wincall_into_result!(GetSystemMetrics(SM_CXSCREEN))? as f32,
        wincall_into_result!(GetSystemMetrics(SM_CYSCREEN))? as f32,
    ]))
}

pub fn highlight_color() -> anyhow::Result<iced::Color> {
    // argb
    let packed = wincall_into_result!(GetSysColor(COLOR_HIGHLIGHT))?;

    let r = (packed & 0x0000_00FF) as u8;
    let g = ((packed & 0x0000_FF00) >> 8) as u8;
    let b = ((packed & 0x00FF_0000) >> 16) as u8;
    Ok(iced::Color::from_rgb8(r, g, b))
}

pub fn restore_windows() {
    let mut windows = Window::enumerate().unwrap_or_else(|e| {
        warn!("Could not enumerate windows to restore them: {e}");
        vec![]
    });

    windows.retain(|w| window::filter::should_be_tiled(*w).unwrap_or(false));

    let mut pos = Position([100.0, 100.0]);
    for window in windows {
        if let Err(err) = window.move_to(pos, [800.0, 600.0].into()) {
            warn!("Failed to move window {window:?}: {err}");
        }
        pos += 100.0;
    }
}

pub fn get_desktop_window() -> anyhow::Result<Window> {
    Window::from_hwnd(wincall_into_result!(GetDesktopWindow())?)
}

#[must_use]
pub fn current_modifiers() -> keyboard_types::Modifiers {
    fn is_vkey_down(key: VIRTUAL_KEY) -> bool {
        let key_state = match wincall_into_result!(GetKeyState(key.0.into())) {
            Ok(res) => u16::from_ne_bytes(res.to_ne_bytes()),
            Err(e) => {
                log::error!(
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
