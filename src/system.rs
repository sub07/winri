use windows::Win32::{
    Graphics::Gdi::{COLOR_HIGHLIGHT, GetSysColor},
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
};

use crate::{utils::color::Color, wincall_into_result};

pub fn screen_size() -> anyhow::Result<(i32, i32)> {
    Ok((
        wincall_into_result!(GetSystemMetrics(SM_CXSCREEN))?,
        wincall_into_result!(GetSystemMetrics(SM_CYSCREEN))?,
    ))
}

pub fn highlight_color() -> Color {
    Color::from_abgr_packed(unsafe { GetSysColor(COLOR_HIGHLIGHT) })
}
