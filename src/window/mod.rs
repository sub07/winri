pub mod filter;

use anyhow::ensure;
use windows::Win32::{
    Foundation::{GetLastError, HWND},
    UI::WindowsAndMessaging::{
        GA_ROOT, GWL_STYLE, GetAncestor, GetClassNameA, GetClassNameW, GetWindowLongA,
        GetWindowTextLengthA, GetWindowTextW, IsWindowVisible, MoveWindow, SW_RESTORE, ShowWindow,
        WINDOW_STYLE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    hwnd: HWND,
}

impl Window {
    pub fn from(hwnd: HWND) -> anyhow::Result<Self> {
        ensure!(!hwnd.is_invalid(), "Invalid window handle");
        Ok(Self { hwnd })
    }

    pub const fn as_inner(&self) -> HWND {
        self.hwnd
    }

    pub fn title(&self) -> anyhow::Result<Option<String>> {
        unsafe {
            ensure!(!self.as_inner().is_invalid(), "Invalid window handle");

            let result = GetWindowTextLengthA(self.as_inner());
            ensure!(
                result >= 0,
                "Unexpected error, window title length is negative: {result}"
            );

            #[allow(clippy::cast_sign_loss)]
            let result = result as usize;
            if result == 0 {
                return Ok(GetLastError().ok()?).map(|()| Option::None);
            }
            ensure!(result > 0, "Failed to get window title");

            let mut title = vec![0u16; result + 1];

            let result = GetWindowTextW(self.as_inner(), &mut title);
            GetLastError().ok()?;
            ensure!(
                result != 0,
                "Expected title of length {} but got 0",
                title.len()
            );

            let title = windows_strings::PCWSTR::from_raw(title.as_ptr());

            Ok(Some(title.to_string()?))
        }
    }

    pub fn class(&self) -> anyhow::Result<String> {
        unsafe {
            ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
            let mut class = vec![0u16; 256];

            let result = GetClassNameW(self.as_inner(), &mut class);
            let class = windows_strings::PCWSTR::from_raw(class.as_ptr());
            ensure!(result != 0, "Failed to get window class");

            Ok(class.to_string()?)
        }
    }

    pub fn is_visible(&self) -> anyhow::Result<bool> {
        unsafe {
            ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
            let style = GetWindowLongA(self.as_inner(), GWL_STYLE);
            if style <= 0 {
                return Ok(false);
            }
            #[allow(clippy::cast_sign_loss)]
            let style = WINDOW_STYLE(style as u32);
            Ok(style.contains(WS_VISIBLE) || style.contains(WS_OVERLAPPEDWINDOW))
        }
    }

    pub fn ancestor(&self) -> anyhow::Result<Self> {
        unsafe {
            ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
            let ancestor = GetAncestor(self.as_inner(), GA_ROOT);
            ensure!(!ancestor.is_invalid(), "Failed to get window ancestor");
            Self::from(ancestor)
        }
    }

    pub fn is_ancestor(&self) -> anyhow::Result<bool> {
        Ok(*self == self.ancestor()?)
    }

    pub fn move_window(&self, x: i32, y: i32, width: i32, height: i32) -> anyhow::Result<()> {
        ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
        unsafe {
            dbg!(ShowWindow(self.as_inner(), SW_RESTORE).ok())?;
            dbg!(MoveWindow(self.as_inner(), x, y, width, height, true))?;
        }
        Ok(())
    }

    pub fn is_uwp(&self) -> anyhow::Result<bool> {
        ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
        Ok(self.class()? == "Windows.UI.Core.CoreWindow"
            && !matches!(self.title()?.as_deref(), Some("DesktopWindowXamlSource")))
    }
}
