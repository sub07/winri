//! A safe, copyable wrapper around a Windows window handle (`HWND`) and the
//! operations winri performs on windows (query, move, focus, close, …). Every
//! method validates the handle first, since handles can be invalidated at any
//! time by a window closing out from under us.

pub mod filter;

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
        UI::WindowsAndMessaging::{
            EnumWindows, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetClassNameW,
            GetClientRect, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, HWND_TOP, IsIconic, IsWindow, IsWindowVisible, MoveWindow,
            PostMessageW, SW_RESTORE, SW_SHOW, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_LONG_PTR_INDEX, WINDOW_STYLE,
            WM_CLOSE, WS_DLGFRAME, WS_EX_NOACTIVATE, WS_POPUP,
        },
    },
    core::BOOL,
};

use crate::{
    utils::math::{Bounds, Position, Size},
    wincall_into_result, wincall_result,
};

/// An `HWND` stored as a plain integer so [`Window`] can be `Send`/`Sync` and
/// hashed (raw `HWND` pointers are neither). Convert back with [`Window::handle`].
pub type SafeHWND = u64;

/// A handle to an OS window. Cheap to copy and pass around; carries no
/// ownership, so the underlying window may cease to exist while a `Window`
/// value still refers to it (hence the validity checks on every operation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub hwnd: SafeHWND,
}

impl Hash for Window {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        format!("{:?}", self.hwnd).hash(state);
    }
}

impl From<RECT> for Bounds {
    fn from(rect: RECT) -> Self {
        #[allow(
            clippy::cast_precision_loss,
            reason = "The values will stay within screen size orders of magnitude"
        )]
        Self {
            left: rect.left as f32,
            top: rect.top as f32,
            right: rect.right as f32,
            bottom: rect.bottom as f32,
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
            hwnd: hwnd.0 as SafeHWND,
        })
    }

    pub fn from_safe_hwnd(safe_hwnd: SafeHWND) -> anyhow::Result<Self> {
        let hwnd = HWND(safe_hwnd as *mut c_void);
        Self::from_hwnd(hwnd)
    }

    pub fn focused() -> anyhow::Result<Self> {
        let hwnd =
            wincall_into_result!(windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow())?;
        Self::from_hwnd(hwnd)
    }

    pub const fn handle(self) -> HWND {
        HWND(self.hwnd as *mut c_void)
    }

    /// Whether the handle still refers to an existing window.
    pub fn is_valid(self) -> anyhow::Result<bool> {
        Ok(!self.handle().is_invalid()
            && wincall_into_result!(IsWindow(Some(self.handle())))?.as_bool())
    }

    /// Enumerates every top-level window on the desktop. Filtering down to the
    /// tileable ones is [`filter::should_be_tiled`]'s job.
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

    /// Whether the window is a dialog (popup with a dialog frame). Dialogs are
    /// excluded from tiling.
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

    /// Executable file name (e.g. `explorer.exe`) of the owning process. Used
    /// by the filter to ignore specific apps.
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

    /// The window class name. Used by the filter to ignore system windows.
    pub fn class(self) -> anyhow::Result<String> {
        ensure_valid!(self);
        let mut class = vec![0u16; 256];

        let _ = wincall_into_result!(GetClassNameW(self.handle(), &mut class))?;
        let class = unsafe { windows_strings::PCWSTR::from_raw(class.as_ptr()).to_string() }?;

        Ok(class)
    }

    /// Whether the window is shown (per Win32 `IsWindowVisible`).
    pub fn is_visible(self) -> anyhow::Result<bool> {
        ensure_valid!(self);
        wincall_into_result!(IsWindowVisible(self.handle()).as_bool())
    }

    /// Whether the window is DWM-"cloaked": visible by Win32 rules yet hidden by
    /// the compositor (e.g. UWP apps on another virtual desktop). Such windows
    /// must be excluded from tiling even though `is_visible` reports `true`.
    pub fn is_cloaked(self) -> anyhow::Result<bool> {
        ensure_valid!(self);
        let mut is_cloaked = BOOL::default();
        self.get_dm_attribute(DWMWA_CLOAKED, &mut is_cloaked)?;
        Ok(is_cloaked.as_bool())
    }

    /// The root owner window in this window's ancestry.
    pub fn ancestor(self) -> anyhow::Result<Self> {
        ensure_valid!(self);
        let ancestor = wincall_into_result!(GetAncestor(self.handle(), GA_ROOT))?;
        Self::from_hwnd(ancestor)
    }

    /// Whether this window is its own root, i.e. a real top-level window rather
    /// than a child/owned one. Only roots are tiled.
    pub fn is_ancestor(self) -> anyhow::Result<bool> {
        Ok(self == self.ancestor()?)
    }

    /// Moves and resizes the window so its *visible frame* lands exactly at
    /// `pos`/`size`, compensating for the invisible DWM border (see
    /// [`Self::padding`]). Restores the window first if minimized.
    pub fn move_to(self, pos: Position, size: Size) -> anyhow::Result<()> {
        ensure_valid!(self);
        let [left, top, right, bottom] = self.padding()?;

        let x = pos.x() - left;
        let y = pos.y() - top;
        let w = size.width() + right + left;
        let h = size.height() + bottom + top;

        let _ = wincall_into_result!(ShowWindow(self.handle(), SW_RESTORE))?;
        wincall_result!(MoveWindow(
            self.handle(),
            x as i32,
            y as i32,
            w as i32,
            h as i32,
            true
        ))?;
        Ok(())
    }

    /// Marks the window as non-activating (`WS_EX_NOACTIVATE`) so clicking or
    /// showing it never steals focus. Used for the overview preview windows.
    pub fn set_no_activate(self) -> anyhow::Result<()> {
        ensure_valid!(self);
        #[allow(
            clippy::cast_possible_wrap,
            reason = "Will never run on 32-bit systems"
        )]
        wincall_into_result!(SetWindowLongPtrW(
            self.handle(),
            GWL_EXSTYLE,
            WS_EX_NOACTIVATE.0 as isize,
        ))?;
        Ok(())
    }

    /// Requests the window close (posts `WM_CLOSE`); the app may still prompt.
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

    /// Parks the window just past the left screen edge. Used to hide the real
    /// windows while the overview's thumbnails stand in for them, without
    /// minimizing (which would disrupt their DWM thumbnails).
    pub fn move_offscreen(self) -> anyhow::Result<()> {
        const ADDITIONAL_OFFSCREEN_OFFSET: f32 = 100.0;

        ensure_valid!(self);
        let width = self.desktop_manager_bounds()?.size().width();

        let offscreen_offset = width + ADDITIONAL_OFFSCREEN_OFFSET;

        wincall_result!(SetWindowPos(
            self.handle(),
            None,
            -offscreen_offset as i32,
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

    /// Raises the window to the top of the z-order without moving or resizing.
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

    /// The client-area rectangle (content only, no frame).
    pub fn inner_bounds(self) -> anyhow::Result<Bounds> {
        ensure_valid!(self);
        let mut rect = RECT::default();
        wincall_result!(GetClientRect(self.handle(), &raw mut rect))?;
        Ok(rect.into())
    }

    /// The *visible* frame as the compositor draws it (DWM extended frame).
    /// This is what the user perceives as the window's edges, and what winri
    /// tiles against — unlike [`Self::outer_bounds`], which includes the
    /// invisible resize border.
    pub fn desktop_manager_bounds(self) -> anyhow::Result<Bounds> {
        ensure_valid!(self);
        let mut rect = RECT::default();
        self.get_dm_attribute(DWMWA_EXTENDED_FRAME_BOUNDS, &mut rect)?;
        Ok(rect.into())
    }

    /// The full window rectangle including the invisible drop-shadow/resize
    /// border (Win32 `GetWindowRect`).
    pub fn outer_bounds(self) -> anyhow::Result<Bounds> {
        ensure_valid!(self);
        let mut rect = RECT::default();
        wincall_result!(GetWindowRect(self.handle(), &raw mut rect))?;
        Ok(rect.into())
    }

    /// The `[left, top, right, bottom]` gap between the outer rectangle and the
    /// visible frame, i.e. the invisible border thickness. [`Self::move_to`]
    /// adds this back so the *visible* frame lands where requested.
    pub fn padding(self) -> anyhow::Result<[f32; 4]> {
        ensure_valid!(self);
        let dm_rect = self.desktop_manager_bounds()?;
        let rect = self.outer_bounds()?;
        Ok([
            (rect.left - dm_rect.left).abs(),
            (rect.top - dm_rect.top).abs(),
            (rect.right - dm_rect.right).abs(),
            (rect.bottom - dm_rect.bottom).abs(),
        ])
    }

    pub fn is_focused(self) -> anyhow::Result<bool> {
        ensure_valid!(self);
        Ok(Self::focused()? == self)
    }

    /// Brings the window to the foreground, restoring it if minimized.
    ///
    /// Windows refuses `SetForegroundWindow` from a process that isn't the
    /// active one; simulating an Alt release first tricks the OS into allowing
    /// it (a widely-used workaround).
    pub fn focus(self) -> anyhow::Result<()> {
        ensure_valid!(self);

        if self.is_focused().unwrap_or(false) {
            return Ok(());
        }

        if wincall_into_result!(IsIconic(self.handle()))?.as_bool() {
            let _ = wincall_into_result!(ShowWindow(self.handle(), SW_RESTORE))?;
            thread::sleep(Duration::from_millis(500));
        }

        // HACK: Simulate an alt key release to bypass focus stealing restrictions:
        // https://stackoverflow.com/questions/10740346/setforegroundwindow-only-working-while-visual-studio-is-open
        rdev::simulate(&rdev::EventType::KeyRelease(rdev::Key::Alt))?;
        let _ = wincall_into_result!(SetForegroundWindow(self.handle()))?;
        Ok(())
    }

    /// A multi-line dump of every queryable property of the window. Purely a
    /// diagnostic aid, attached as context to errors and bug reports.
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
        let outer_bounds = self.outer_bounds();
        let inner_bounds = self.inner_bounds();
        let desktop_manager_bounds = self.desktop_manager_bounds();
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
        push!(outer_bounds);
        push!(inner_bounds);
        push!(desktop_manager_bounds);
        push!(padding);
        push!(is_focused);
        push!(is_dialog);

        res
    }
}
