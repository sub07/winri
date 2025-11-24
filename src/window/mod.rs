pub mod filter;
pub mod manager;

use std::{ffi::c_void, hash::Hash, thread, time::Duration};

use anyhow::{Context, ensure};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT, WPARAM},
        Graphics::Dwm::{
            DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWMWINDOWATTRIBUTE, DwmGetWindowAttribute,
        },
        System::{
            ProcessStatus::GetModuleFileNameExW,
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
        UI::{
            Input::KeyboardAndMouse::{KEYBD_EVENT_FLAGS, keybd_event},
            WindowsAndMessaging::{
                CloseWindow, EnumWindows, GA_ROOT, GWL_STYLE, GetAncestor, GetClassNameW,
                GetClientRect, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
                GetWindowThreadProcessId, HWND_TOP, IsIconic, IsWindow, IsWindowVisible,
                MoveWindow, PostMessageW, SW_RESTORE, SW_SHOW, SWP_NOMOVE, SWP_NOSIZE,
                SetForegroundWindow, SetWindowPos, ShowWindow, WINDOW_LONG_PTR_INDEX, WINDOW_STYLE,
                WM_CLOSE, WS_DLGFRAME, WS_POPUP,
            },
        },
    },
    core::BOOL,
};

use crate::{
    try_cast,
    utils::{Position, Rectangle, Size, cast::FaillibleCastUtils},
    wincall_into_result, wincall_result,
};

pub type SafeHWND = isize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub hwnd: SafeHWND,
}

impl Hash for Window {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        format!("{:?}", self.hwnd).hash(state);
    }
}

impl From<RECT> for Rectangle {
    fn from(rect: RECT) -> Self {
        Self {
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left).cast(),
            height: (rect.bottom - rect.top).cast(),
        }
    }
}

macro_rules! ensure_valid {
    ($s:expr) => {
        ensure!(
            $s.is_valid()?,
            "[{}] Invalid window handle: {:?}",
            crate::function!(),
            $s.handle()
        );
    };
}

impl Window {
    pub fn from_hwnd(hwnd: HWND) -> anyhow::Result<Self> {
        ensure!(!hwnd.is_invalid(), "Invalid window handle");
        Ok(Self {
            hwnd: hwnd.0 as isize,
        })
    }

    pub fn focused() -> anyhow::Result<Self> {
        let hwnd =
            wincall_into_result!(windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow())?;
        Self::from_hwnd(hwnd)
    }

    pub const fn handle(self) -> HWND {
        HWND(self.hwnd as *mut c_void)
    }

    pub fn is_valid(self) -> anyhow::Result<bool> {
        Ok(!self.handle().is_invalid()
            && wincall_into_result!(IsWindow(Some(self.handle())))?.as_bool())
    }

    pub fn enumerate() -> anyhow::Result<Vec<Self>> {
        unsafe extern "system" fn enum_callback(window: HWND, out_list: LPARAM) -> BOOL {
            let list = unsafe { &mut *(out_list.0 as *mut Vec<Window>) };
            if let Ok(win) = Window::from_hwnd(window) {
                list.push(win);
            }
            true.into() // Continue enumeration
        }

        let mut result = Vec::new();

        wincall_result!(EnumWindows(
            Some(enum_callback),
            LPARAM(&raw mut result as isize)
        ))?;

        Ok(result)
    }

    fn get_dm_attribute<T>(
        self,
        attribute: DWMWINDOWATTRIBUTE,
        result: &mut T,
    ) -> anyhow::Result<()> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "size of small struct will never be large enough to be truncated"
        )]
        wincall_result!(DwmGetWindowAttribute(
            self.handle(),
            attribute,
            std::ptr::from_mut::<T>(result).cast::<c_void>(),
            std::mem::size_of::<T>() as u32,
        ))
        .context(attribute.0)?;
        Ok(())
    }

    fn get_window_long(self, attribute: WINDOW_LONG_PTR_INDEX) -> anyhow::Result<i32> {
        ensure_valid!(self);
        wincall_into_result!(GetWindowLongW(self.handle(), attribute))
    }

    pub fn is_dialog(self) -> anyhow::Result<bool> {
        ensure_valid!(self);
        let style = self.get_window_long(GWL_STYLE)?;

        #[allow(clippy::cast_sign_loss, reason = "WINDOW_STYLE is u32")]
        let style = WINDOW_STYLE(style as u32);

        Ok(style.contains(WS_POPUP) && style.contains(WS_DLGFRAME))
    }

    pub fn title(self) -> anyhow::Result<Option<String>> {
        ensure_valid!(self);

        let title_len = wincall_into_result!(GetWindowTextLengthW(self.handle()))?;
        ensure!(
            title_len >= 0,
            "Unexpected error, window title length is negative: {title_len}"
        );
        if title_len == 0 {
            return Ok(None);
        }

        #[allow(clippy::cast_sign_loss)]
        let title_len = title_len as usize;

        let mut title = vec![0u16; title_len + 1];

        let title_len_read = wincall_into_result!(GetWindowTextW(self.handle(), &mut title))?;
        ensure!(
            title_len_read != 0,
            "Expected reading title of length {} but read 0",
            title.len()
        );

        let title = unsafe { windows_strings::PCWSTR::from_raw(title.as_ptr()).to_string() }?;

        Ok(Some(title))
    }

    pub fn process_id(self) -> anyhow::Result<u32> {
        ensure_valid!(self);
        let mut process_id = 0;

        let _ = wincall_into_result!(GetWindowThreadProcessId(
            self.handle(),
            Some(&raw mut process_id)
        ))?;

        Ok(process_id)
    }

    pub fn process_name(self) -> anyhow::Result<String> {
        ensure_valid!(self);
        let process_id = self.process_id()?;
        let process = wincall_result!(OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            process_id,
        ))?;

        let mut process_name = vec![0u16; 256];

        let _ = wincall_into_result!(GetModuleFileNameExW(Some(process), None, &mut process_name))?;

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
        ensure_valid!(self);
        let mut class = vec![0u16; 256];

        let _ = wincall_into_result!(GetClassNameW(self.handle(), &mut class))?;
        let class = unsafe { windows_strings::PCWSTR::from_raw(class.as_ptr()).to_string() }?;

        Ok(class)
    }

    pub fn is_visible(self) -> anyhow::Result<bool> {
        ensure_valid!(self);
        wincall_into_result!(IsWindowVisible(self.handle()).as_bool())
    }

    pub fn is_cloaked(self) -> anyhow::Result<bool> {
        ensure_valid!(self);
        let mut is_cloaked = BOOL::default();
        self.get_dm_attribute(DWMWA_CLOAKED, &mut is_cloaked)?;
        Ok(is_cloaked.as_bool())
    }

    pub fn ancestor(self) -> anyhow::Result<Self> {
        ensure_valid!(self);
        let ancestor = wincall_into_result!(GetAncestor(self.handle(), GA_ROOT))?;
        Self::from_hwnd(ancestor)
    }

    pub fn is_ancestor(self) -> anyhow::Result<bool> {
        Ok(self == self.ancestor()?)
    }

    pub fn move_to(self, pos: Position, size: Size) -> anyhow::Result<()> {
        ensure_valid!(self);
        // TODO: detect weird border that leave one pixel on top and left
        // For now, here's a tweak
        let pos = pos - 1;
        let size = size + 1;

        let [left, top, right, bottom] = self.padding()?;

        try_cast! {
            left => i32 as left_i32,
            top => i32 as top_i32,
        }

        let x = pos.x() - left_i32;
        let y = pos.y() - top_i32;
        let w = size.width() + right + left;
        let h = size.height() + bottom + top;

        try_cast! {
            w => i32,
            h => i32,
        }

        let _ = wincall_into_result!(ShowWindow(self.handle(), SW_RESTORE))?;
        wincall_result!(MoveWindow(self.handle(), x, y, w, h, true,))?;
        Ok(())
    }

    pub fn close(self) -> anyhow::Result<()> {
        ensure_valid!(self);
        wincall_result!(PostMessageW(
            Some(self.handle()),
            WM_CLOSE,
            WPARAM::default(),
            LPARAM::default()
        ))?;
        Ok(())
    }

    pub fn move_offscreen(self) -> anyhow::Result<()> {
        ensure_valid!(self);
        let size = self.desktop_manager_rect()?;
        wincall_result!(SetWindowPos(
            self.handle(),
            None,
            -(size.right - size.left),
            0,
            0,
            0,
            SWP_NOSIZE
        ))?;
        Ok(())
    }

    pub fn show(self) -> anyhow::Result<()> {
        ensure_valid!(self);
        let _ = wincall_into_result!(ShowWindow(self.handle(), SW_SHOW))?;
        Ok(())
    }

    pub fn set_max_zindex(self) -> anyhow::Result<()> {
        ensure_valid!(self);
        wincall_result!(SetWindowPos(
            self.handle(),
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE,
        ))?;
        Ok(())
    }

    pub fn client_rect(self) -> anyhow::Result<Rectangle> {
        ensure_valid!(self);
        let mut rect = RECT::default();
        wincall_result!(GetClientRect(self.handle(), &raw mut rect))?;
        Ok(rect.into())
    }

    pub fn desktop_manager_rect(self) -> anyhow::Result<RECT> {
        ensure_valid!(self);
        let mut rect = RECT::default();
        self.get_dm_attribute(DWMWA_EXTENDED_FRAME_BOUNDS, &mut rect)?;
        Ok(rect)
    }

    pub fn rect(self) -> anyhow::Result<RECT> {
        ensure_valid!(self);
        let mut rect = RECT::default();
        wincall_result!(GetWindowRect(self.handle(), &raw mut rect))?;
        Ok(rect)
    }

    pub fn padding(self) -> anyhow::Result<[u32; 4]> {
        ensure_valid!(self);
        let dm_rect = self.desktop_manager_rect()?;
        let rect = self.rect()?;
        Ok([
            (rect.left - dm_rect.left).abs().try_cast()?,
            (rect.top - dm_rect.top).abs().try_cast()?,
            (rect.right - dm_rect.right).abs().try_cast()?,
            (rect.bottom - dm_rect.bottom).abs().try_cast()?,
        ])
    }

    pub fn is_focused(self) -> anyhow::Result<bool> {
        ensure_valid!(self);
        Ok(Self::focused()? == self)
    }

    pub fn focus(self) -> anyhow::Result<()> {
        ensure_valid!(self);

        if self.is_focused()? {
            return Ok(());
        }

        if wincall_into_result!(IsIconic(self.handle()))?.as_bool() {
            let _ = wincall_into_result!(ShowWindow(self.handle(), SW_RESTORE))?;
            thread::sleep(Duration::from_millis(500));
        }

        // Simulate a key press to bypass focus stealing restrictions : https://stackoverflow.com/a/30572826
        wincall_into_result!(keybd_event(0, 0, KEYBD_EVENT_FLAGS(0), 0))?;
        let _ = wincall_into_result!(SetForegroundWindow(self.handle()))?;
        Ok(())
    }

    #[must_use]
    pub fn get_formatted_extensive_info(self) -> String {
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
        let is_focused = self.is_focused();
        let is_dialog = self.is_dialog();

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
        push!(is_focused);
        push!(is_dialog);

        res
    }
}
