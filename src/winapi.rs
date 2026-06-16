//! The lowest layer: raw Win32 calls and the error-handling plumbing every
//! other module relies on. The `wincall*` macros are the key idea — they clear
//! `GetLastError`, run the call, then turn a set last-error into an
//! `anyhow::Error` tagged with the calling function, so unsafe Win32 calls
//! become ordinary fallible Rust expressions.
use windows::Win32::Foundation::{GetLastError, SetLastError, WIN32_ERROR};
use windows::Win32::UI::WindowsAndMessaging::{MESSAGEBOX_RESULT, MESSAGEBOX_STYLE, MessageBoxW};
use windows_strings::PCWSTR;

/// Encodes a Rust string as a NUL-terminated UTF-16 buffer for Win32 `W` APIs.
pub fn str_to_wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    let os_str = std::ffi::OsStr::new(&s);
    let wide: Vec<u16> = os_str.encode_wide().chain(std::iter::once(0)).collect();
    wide
}

/// Lets a UTF-16 buffer be viewed as a `PCWSTR` pointer for Win32 calls.
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

/// Resets the thread's last-error to 0. Called before a Win32 call so a stale
/// error from an earlier call isn't misattributed to this one.
pub fn clear_last_error() {
    unsafe {
        SetLastError(WIN32_ERROR(0));
    };
}

/// The thread's current Win32 last-error as an `anyhow::Error`, or `None` if it
/// is 0 (no error).
pub fn last_error() -> Option<anyhow::Error> {
    unsafe { GetLastError().ok().err().map(Into::into) }
}

/// Runs a Win32 expression
///
/// Wrap inside the required `unsafe` block, clearing the
/// last-error first. The lowest-level building block; callers usually want
/// `wincall_result!` or `wincall_into_result!`, which also surface errors.
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

/// For Win32 calls that already return a `Result`: runs it and, on `Err`,
/// attaches the OS last-error and the calling function name as context.
#[macro_export]
macro_rules! wincall_result {
    ($fn:expr) => {
        anyhow::Context::context(
            anyhow::Context::context($crate::wincall!($fn), $crate::function!()),
            $crate::winapi::last_error().unwrap_or(anyhow::anyhow!("Unknown error")),
        )
    };
}

/// For Win32 calls that return a plain value (not a `Result`): runs the call,
/// then promotes it to `Err` if the OS last-error was set, otherwise `Ok(value)`.
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
