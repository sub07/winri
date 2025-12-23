use windows::Win32::Foundation::{GetLastError, SetLastError, WIN32_ERROR};
use windows::Win32::UI::WindowsAndMessaging::{MESSAGEBOX_RESULT, MESSAGEBOX_STYLE, MessageBoxW};
use windows_strings::PCWSTR;

pub fn message_box(title: &str, message: &str, kind: MESSAGEBOX_STYLE) -> MESSAGEBOX_RESULT {
    use std::os::windows::ffi::OsStrExt;

    let title_os = std::ffi::OsStr::new(&title);
    let title = title_os.encode_wide().chain(std::iter::once(0));
    let title = title.collect::<Vec<_>>();

    let message_os = std::ffi::OsStr::new(&message);
    let message = message_os.encode_wide().chain(std::iter::once(0));
    let message = message.collect::<Vec<_>>();

    unsafe { MessageBoxW(None, PCWSTR(message.as_ptr()), PCWSTR(title.as_ptr()), kind) }
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
