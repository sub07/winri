use windows::Win32::{
    Graphics::Gdi::{COLOR_HIGHLIGHT, GetSysColor},
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
};

use crate::{
    utils::{Size, cast::FaillibleCastUtils, color::Color},
    wincall_into_result,
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
