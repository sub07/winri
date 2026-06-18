//! Thin, app-level wrappers over the OS for system-wide queries and actions
//! (screen geometry, theme colour, modifier state, message boxes) that aren't
//! tied to a specific [`Window`].
use anyhow::bail;
use log::warn;
use windows::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, RECT},
    Graphics::Gdi::{COLOR_HIGHLIGHT, GetSysColor},
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW,
    },
    System::Threading::CreateMutexW,
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_LWIN, VK_MENU,
            VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{
            GetDesktopWindow, GetSystemMetrics, IDYES, MB_OK, MB_YESNO, SM_CXSCREEN, SM_CYSCREEN,
            SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
    },
};
use windows_strings::PCWSTR;
use windows_strings::w;

use crate::{
    utils::math::{Bounds, Position, Size},
    winapi::{self},
    wincall, wincall_into_result, wincall_result,
    window::{self, Window},
};

/// Primary monitor's full pixel dimensions (including the taskbar area). For
/// the tileable region use [`work_area`] instead.
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

/// Primary monitor's work area: screen rectangle minus the taskbar and any
/// other docked `AppBars`. Tiling inside this rectangle keeps windows from
/// overlapping the taskbar regardless of which edge it sits on.
pub fn work_area() -> anyhow::Result<Bounds> {
    let mut rect = RECT::default();
    wincall_result!(SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        Some(std::ptr::from_mut::<RECT>(&mut rect).cast::<std::ffi::c_void>()),
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
    ))?;
    Ok(rect.into())
}

/// The system accent/highlight colour, used for the focused-window border so
/// winri matches the user's Windows theme.
pub fn highlight_color() -> anyhow::Result<iced::Color> {
    // argb
    let packed = wincall_into_result!(GetSysColor(COLOR_HIGHLIGHT))?;

    let r = (packed & 0x0000_00FF) as u8;
    let g = ((packed & 0x0000_FF00) >> 8) as u8;
    let b = ((packed & 0x00FF_0000) >> 16) as u8;
    Ok(iced::Color::from_rgb8(r, g, b))
}

/// Restore all tiled windows to a cascading position for user convenience.
/// Typically called on application exit (nominal or error), so that windows are not lost off-screen.
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

/// The desktop window. Focusing it is winri's way of dropping focus to "no
/// app" (e.g. after the overlay would otherwise grab it).
pub fn get_desktop_window() -> anyhow::Result<Window> {
    Window::from_hwnd(wincall_into_result!(GetDesktopWindow())?)
}

/// The modifier keys currently held, read directly from the OS. Used to seed
/// the hook's modifier state at startup so it isn't wrong until the first
/// keypress.
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

#[allow(dead_code, reason = "Could be useful at some point")]
pub fn message_box_info(title: &str, message: &str) {
    winapi::message_box(title, message, MB_OK);
}

pub fn message_box_query(title: &str, message: &str) -> bool {
    winapi::message_box(title, message, MB_YESNO) == IDYES
}

pub fn aquire_winri_running_lock() -> anyhow::Result<Option<HANDLE>> {
    const WINRI_MUTEX_NAME: PCWSTR = windows_strings::w!("WinriRunning");
    let mutex = wincall!(CreateMutexW(None, false, WINRI_MUTEX_NAME))?;
    let last_error = unsafe { GetLastError() };
    if last_error == ERROR_ALREADY_EXISTS {
        Ok(None)
    } else {
        Ok(Some(mutex))
    }
}

/// Return the lock enabled state from the registry.
pub fn is_lock_enabled() -> anyhow::Result<bool> {
    let hkey = HKEY_CURRENT_USER;
    let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System");

    let mut key = HKEY::default();
    let status = wincall_into_result!(RegOpenKeyExW(hkey, subkey, None, KEY_READ, &raw mut key))?;

    // The retrieval is a bit arcane: when the DisableLockWorkstation value is not enabled, the key does not exist, so we return true in that case.
    if status.is_ok() {
        let mut value = 0u32;
        let mut data_size = std::mem::size_of::<u32>() as u32;
        let status = wincall_into_result!(RegQueryValueExW(
            key,
            w!("DisableLockWorkstation"),
            None,
            None,
            Some((&raw mut value).cast::<u8>()),
            Some(&raw mut data_size),
        ))?;
        let _ = wincall_into_result!(RegCloseKey(key))?;

        // If the read is successful, we consider the lock disabled. Again a bit arcane.
        if status.is_ok() { Ok(false) } else { Ok(true) }
    } else {
        Ok(true)
    }
}

/// Disable the locking ability
///
/// Must only be called with user approval.
pub fn disable_lock() -> anyhow::Result<()> {
    if !is_lock_enabled()? {
        return Ok(());
    }
    let hkey = HKEY_CURRENT_USER;
    let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System");

    let mut key = HKEY::default();
    let status = wincall_into_result!(RegCreateKeyExW(
        hkey,
        subkey,
        None,
        None,
        REG_OPTION_NON_VOLATILE,
        KEY_WRITE,
        None,
        &raw mut key,
        None,
    ))?;

    if status.is_ok() {
        let value = [1, 0, 0, 0];
        let status = wincall_into_result!(RegSetValueExW(
            key,
            w!("DisableLockWorkstation"),
            None,
            REG_DWORD,
            Some(&value),
        ))?;
        let _ = wincall_into_result!(RegCloseKey(key))?;
        if status.is_ok() {
            Ok(())
        } else {
            bail!("Failed to set the registry value for disabling lock workstation: {status:?}")
        }
    } else {
        bail!("Failed to create/open the registry key for disabling lock workstation: {status:?}")
    }
}

/// Mainly used to restore the lock status after the user has approved it to be disabled.
#[allow(dead_code, reason = "Might be used later to restore the lock status")]
pub fn enable_lock() -> anyhow::Result<()> {
    if is_lock_enabled()? {
        return Ok(());
    }
    let hkey = HKEY_CURRENT_USER;
    let subkey = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System");
    let mut key = HKEY::default();
    let status = wincall_into_result!(RegOpenKeyExW(hkey, subkey, None, KEY_WRITE, &raw mut key))?;
    if status.is_ok() {
        let _ = wincall_into_result!(RegDeleteValueW(key, w!("DisableLockWorkstation")))?;
        let _ = wincall_into_result!(RegCloseKey(key))?;
        Ok(())
    } else {
        bail!("Failed to open the registry key for enabling lock workstation: {status:?}")
    }
}
