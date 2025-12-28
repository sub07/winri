use windows::Win32::Foundation::{GetLastError, SetLastError, WIN32_ERROR};
use windows::Win32::UI::WindowsAndMessaging::{MESSAGEBOX_RESULT, MESSAGEBOX_STYLE, MessageBoxW};
use windows_strings::PCWSTR;

pub fn str_to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    let os_str = std::ffi::OsStr::new(&s);
    let wide: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0)).collect();
    wide
}

#[easy_ext::ext(WindowsStringsExt)]
pub impl Vec<u16> {
    fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR::from_raw(self.as_ptr())
    }
}

pub fn message_box(title: &str, message: &str, kind: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    let title = str_to_wide(title);
    let message = str_to_wide(message);

    unsafe { MessageBoxW(None, message.as_pcwstr(), title.as_pcwstr(), kind) }
}

pub fn clear_last_error() {
    unsafe {
        SetLastError(WIN32_ERROR(0));
    };
}

pub fn last_error() -> Option<anyhow::Error> {
    unsafe { GetLastError().ok().err().map(Into::into) }
}

#[macro_export]
macro_rules! wincall {
    ($fn:expr) => {
        {
            #[allow(
                clippy::macro_metavars_in_unsafe,
                reason = "This macro should always call a winapi function and thus is always unsafe. The caller should know that a unsafe block is automatically applied"
            )]
            unsafe {
                $crate::winapi::clear_last_error();
                $fn
            }
        }
    };
}

#[macro_export]
macro_rules! wincall_result {
    ($fn:expr) => {
        anyhow::Context::context(
            anyhow::Context::context($crate::wincall!($fn), $crate::function!()),
            $crate::winapi::last_error().unwrap_or(anyhow::anyhow!("Unknown error")),
        )
    };
}

#[macro_export]
macro_rules! wincall_into_result {
    ($fn:expr) => {{
        let res = $crate::wincall!($fn);
        $crate::winapi::last_error().map_or_else(
            || Ok(res),
            |err| anyhow::Context::context(Err(err), $crate::function!()),
        )
    }};
}
