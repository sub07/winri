use log::warn;
use windows::Win32::{
    Graphics::Gdi::{COLOR_HIGHLIGHT, GetSysColor},
    UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN},
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
