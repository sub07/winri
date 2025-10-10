pub mod filter;

use std::hash::Hash;

use anyhow::{Context, ensure};
use windows::Win32::{
    Foundation::{GetLastError, HWND, RECT},
    Graphics::Dwm::{DWMWA_CLOAKED, DwmExtendFrameIntoClientArea, DwmSetWindowAttribute},
    System::{
        ProcessStatus::GetModuleFileNameExW,
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
    UI::{
        Controls::MARGINS,
        WindowsAndMessaging::{
            GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetClassNameA, GetClassNameW,
            GetClientRect, GetWindowLongA, GetWindowRect, GetWindowTextLengthA, GetWindowTextW,
            GetWindowThreadProcessId, IsWindowVisible, MoveWindow, SW_RESTORE, SetWindowLongW,
            ShowWindow, WINDOW_STYLE, WS_EX_LAYERED, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    hwnd: HWND,
}

impl Hash for Window {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        format!("{:?}", self.hwnd).hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<RECT> for Rectangle {
    fn from(rect: RECT) -> Self {
        Self {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        }
    }
}

impl Window {
    pub fn from(hwnd: HWND) -> anyhow::Result<Self> {
        ensure!(!hwnd.is_invalid(), "Invalid window handle");
        Ok(Self { hwnd })
    }

    pub fn focused() -> anyhow::Result<Self> {
        unsafe {
            let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
            ensure!(!hwnd.is_invalid(), "Invalid window handle");
            Ok(Self { hwnd })
        }
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
            GetLastError().ok().context(format!("{self:?}"))?;
            ensure!(
                result != 0,
                "Expected title of length {} but got 0",
                title.len()
            );

            let title = windows_strings::PCWSTR::from_raw(title.as_ptr());

            Ok(Some(title.to_string()?))
        }
    }

    pub fn process_id(&self) -> anyhow::Result<u32> {
        unsafe {
            ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
            let mut process_id = 0;

            let result = GetWindowThreadProcessId(self.as_inner(), Some(&raw mut process_id));
            ensure!(result != 0, "Failed to get window process ID");

            Ok(process_id)
        }
    }

    pub fn process_name(&self) -> anyhow::Result<String> {
        unsafe {
            ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
            let process_id = self.process_id()?;
            let process = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                process_id,
            )?;

            let mut process_name = vec![0u16; 256];

            let result = GetModuleFileNameExW(Some(process), None, &mut process_name);
            ensure!(result != 0, "Failed to get process name");

            let process_file_path =
                windows_strings::PCWSTR::from_raw(process_name.as_ptr()).to_string()?;
            let process_name = process_file_path
                .split('\\')
                .next_back()
                .unwrap()
                .to_string();
            Ok(process_name)
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
            Ok(IsWindowVisible(self.as_inner()).as_bool())
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
            let (padding_x, padding_y) = (0, 0);
            // let (padding_x, padding_y) = self.padding()?;
            ShowWindow(self.as_inner(), SW_RESTORE).ok()?;
            MoveWindow(
                self.as_inner(),
                x - padding_x,
                y - padding_y,
                width + padding_x,
                height + padding_y,
                true,
            )?;
        }
        Ok(())
    }

    pub fn is_uwp(&self) -> anyhow::Result<bool> {
        ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
        Ok(self.class()? == "Windows.UI.Core.CoreWindow"
            && !matches!(self.title()?.as_deref(), Some("DesktopWindowXamlSource")))
    }

    pub fn client_rect(&self) -> anyhow::Result<Rectangle> {
        ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.as_inner(), &raw mut rect)?;
            Ok(rect.into())
        }
    }

    pub fn rect(&self) -> anyhow::Result<Rectangle> {
        ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
        unsafe {
            let mut rect = RECT::default();
            GetWindowRect(self.as_inner(), &raw mut rect)?;
            Ok(rect.into())
        }
    }

    pub fn padding(&self) -> anyhow::Result<(i32, i32)> {
        ensure!(!self.as_inner().is_invalid(), "Invalid window handle");
        let client_rect = self.client_rect()?;
        let rect = self.rect()?;
        Ok((
            rect.width - client_rect.width,
            rect.height - client_rect.height,
        ))
    }
}
