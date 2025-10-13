pub mod filter;

use std::{ffi::c_void, hash::Hash};

use anyhow::{Context, ensure};
use windows::{
    Win32::{
        Foundation::{GetLastError, HWND, RECT},
        Graphics::Dwm::{
            DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWINDOWATTRIBUTE, DwmGetWindowAttribute,
        },
        System::{
            ProcessStatus::GetModuleFileNameExW,
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
        UI::WindowsAndMessaging::{
            GA_ROOT, GetAncestor, GetClassNameW, GetClientRect, GetWindowRect,
            GetWindowTextLengthA, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
            IsWindowVisible, MoveWindow, SW_RESTORE, ShowWindow,
        },
    },
    core::BOOL,
};

use crate::function;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub hwnd: HWND,
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

macro_rules! check {
    ($s:tt) => {
        $s.ensure_valid(function!())?;
    };
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

    pub const fn handle(self) -> HWND {
        self.hwnd
    }

    pub fn is_valid(self) -> bool {
        !self.hwnd.is_invalid() && unsafe { IsWindow(Some(self.handle())).as_bool() }
    }

    fn get_dm_attribute<T>(
        self,
        attribute: DWMWINDOWATTRIBUTE,
        result: &mut T,
    ) -> anyhow::Result<()> {
        unsafe {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "size of small struct will never be large enough to be truncated"
            )]
            DwmGetWindowAttribute(
                self.handle(),
                attribute,
                std::ptr::from_mut::<T>(result).cast::<c_void>(),
                std::mem::size_of::<T>() as u32,
            )
        }?;
        Ok(())
    }

    #[inline]
    pub fn ensure_valid(self, caller_name: &str) -> anyhow::Result<()> {
        ensure!(
            self.is_valid(),
            "[{}] Invalid window handle: {:?}",
            caller_name,
            self.handle()
        );
        Ok(())
    }

    pub fn title(self) -> anyhow::Result<Option<String>> {
        check!(self);
        let result = unsafe { GetWindowTextLengthA(self.handle()) };
        ensure!(
            result >= 0,
            "Unexpected error, window title length is negative: {result}"
        );

        #[allow(clippy::cast_sign_loss)]
        let result = result as usize;
        if result == 0 {
            return Ok(unsafe { GetLastError().ok() }?).map(|()| Option::None);
        }
        ensure!(result > 0, "Failed to get window title");

        let mut title = vec![0u16; result + 1];

        let result = unsafe { GetWindowTextW(self.handle(), &mut title) };
        unsafe { GetLastError().ok().context(format!("{self:?}")) }?;
        ensure!(
            result != 0,
            "Expected title of length {} but got 0",
            title.len()
        );

        let title = windows_strings::PCWSTR::from_raw(title.as_ptr());

        Ok(Some(unsafe { title.to_string() }?))
    }

    pub fn process_id(self) -> anyhow::Result<u32> {
        check!(self);
        let mut process_id = 0;

        let result = unsafe { GetWindowThreadProcessId(self.handle(), Some(&raw mut process_id)) };
        ensure!(result != 0, "Failed to get window process ID");

        Ok(process_id)
    }

    pub fn process_name(self) -> anyhow::Result<String> {
        check!(self);
        let process_id = self.process_id()?;
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                process_id,
            )
        }?;

        let mut process_name = vec![0u16; 256];

        let result = unsafe { GetModuleFileNameExW(Some(process), None, &mut process_name) };
        ensure!(result != 0, "Failed to get process name");

        let process_file_path =
            unsafe { windows_strings::PCWSTR::from_raw(process_name.as_ptr()).to_string() }?;
        let process_name = process_file_path
            .split('\\')
            .next_back()
            .unwrap()
            .to_string();
        Ok(process_name)
    }

    pub fn class(self) -> anyhow::Result<String> {
        check!(self);
        let mut class = vec![0u16; 256];

        let result = unsafe { GetClassNameW(self.handle(), &mut class) };
        let class = windows_strings::PCWSTR::from_raw(class.as_ptr());
        ensure!(result != 0, "Failed to get window class");

        Ok(unsafe { class.to_string() }?)
    }

    pub fn is_visible(self) -> anyhow::Result<bool> {
        unsafe {
            check!(self);
            Ok(IsWindowVisible(self.handle()).as_bool())
        }
    }

    pub fn is_cloaked(self) -> anyhow::Result<bool> {
        check!(self);
        let mut is_cloaked = BOOL::default();
        self.get_dm_attribute(DWMWA_CLOAKED, &mut is_cloaked)?;
        Ok(is_cloaked.as_bool())
    }

    pub fn ancestor(self) -> anyhow::Result<Self> {
        unsafe {
            check!(self);
            let ancestor = GetAncestor(self.handle(), GA_ROOT);
            ensure!(!ancestor.is_invalid(), "Failed to get window ancestor");
            Self::from(ancestor)
        }
    }

    pub fn is_ancestor(self) -> anyhow::Result<bool> {
        Ok(self == self.ancestor()?)
    }

    pub fn move_window(self, x: i32, y: i32, width: i32, height: i32) -> anyhow::Result<()> {
        check!(self);
        // TODO: detect weird border that leave one pixel on top and left
        // For now, here's a tweak
        let x = x - 1;
        let y = y - 1;
        let width = width + 1;
        let height = height + 1;

        let [left, top, right, bottom] = self.padding()?;
        unsafe {
            ShowWindow(self.handle(), SW_RESTORE).ok()?;
            MoveWindow(
                self.handle(),
                x - left,
                y - top,
                width + right + left,
                height + bottom + top,
                true,
            )?;
        }
        Ok(())
    }

    pub fn client_rect(self) -> anyhow::Result<Rectangle> {
        check!(self);
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.handle(), &raw mut rect)?;
        }
        Ok(rect.into())
    }

    pub fn desktop_manager_rect(self) -> anyhow::Result<RECT> {
        check!(self);
        let mut rect = RECT::default();
        self.get_dm_attribute(DWMWA_EXTENDED_FRAME_BOUNDS, &mut rect)?;
        Ok(rect)
    }

    pub fn rect(self) -> anyhow::Result<RECT> {
        check!(self);
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(self.handle(), &raw mut rect)?;
        }
        Ok(rect)
    }

    pub fn padding(self) -> anyhow::Result<[i32; 4]> {
        check!(self);
        let dm_rect = self.desktop_manager_rect()?;
        let rect = self.rect()?;
        Ok([
            (rect.left - dm_rect.left).abs(),
            (rect.top - dm_rect.top).abs(),
            (rect.right - dm_rect.right).abs(),
            (rect.bottom - dm_rect.bottom).abs(),
        ])
    }

    pub fn is_focused(self) -> anyhow::Result<bool> {
        check!(self);
        Ok(Self::focused()? == self)
    }

    pub fn print_extensive_info(self) -> String {
        use std::fmt::Write as _;

        let handle = self.handle();
        let is_valid = self.is_valid();
        let title = self.title();
        let process_id = self.process_id();
        let process_name = self.process_name();
        let class = self.class();
        let is_visible = self.is_visible();
        let is_cloaked = self.is_cloaked();
        let ancestor = self.ancestor();
        let is_ancestor = self.is_ancestor();
        let rect = self.rect();
        let client_rect = self.client_rect();
        let desktop_manager_rect = self.desktop_manager_rect();
        let padding = self.padding();

        let mut res = String::new();

        let _ = write!(res, "Window {handle:?} info:");

        macro_rules! push {
            ($var:tt) => {
                let _ = write!(res, "\n\t{}: {:?}", stringify!($var), $var);
            };
        }

        push!(is_valid);
        push!(title);
        push!(process_id);
        push!(process_name);
        push!(class);
        push!(is_visible);
        push!(is_cloaked);
        push!(ancestor);
        push!(is_ancestor);
        push!(rect);
        push!(client_rect);
        push!(desktop_manager_rect);
        push!(padding);

        res
    }
}
