use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

use crate::wincall_into_result;

pub fn screen_size() -> anyhow::Result<(i32, i32)> {
    Ok((
        wincall_into_result!(GetSystemMetrics(SM_CXSCREEN))?,
        wincall_into_result!(GetSystemMetrics(SM_CYSCREEN))?,
    ))
}
