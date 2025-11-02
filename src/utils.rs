use std::fmt::Display;

use anyhow::anyhow;

#[macro_export]
macro_rules! function {
    () => {{
        const fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        name.strip_suffix("::f").unwrap()
    }};
}

#[macro_export]
macro_rules! cast {
    ($src:expr => $t:ty as $dest:ident, $($rem:tt)*) => {
        let $dest: $t = $src.cast();
        cast!($($rem)*);
    };
    ($i:ident => $t:ty, $($rem:tt)*) => {
        let $i: $t = $i.cast();
        cast!($($rem)*);
    };
    () => {};
}

#[macro_export]
macro_rules! try_cast {
    ($src:expr => $t:ty as $dest:ident, $($rem:tt)*) => {
        let $dest: $t = $src.try_cast()?;
        try_cast!($($rem)*);
    };
    ($i:ident => $t:ty, $($rem:tt)*) => {
        let $i: $t = $i.try_cast()?;
        try_cast!($($rem)*);
    };
    () => {};
}

#[easy_ext::ext(CastUtils)]
pub impl<T, R> T
where
    T: TryInto<R> + Display + Clone + Copy,
{
    fn cast(self) -> R {
        self.try_cast().expect("Cast failed")
    }

    fn try_cast(self) -> anyhow::Result<R> {
        self.try_into().map_err(|_| {
            anyhow!(
                "Cast from {} with value {self} to {} failed",
                std::any::type_name::<T>(),
                std::any::type_name::<R>()
            )
        })
    }
}

pub mod winapi {
    use windows::Win32::Foundation::{GetLastError, SetLastError, WIN32_ERROR};
    use windows::Win32::UI::WindowsAndMessaging::{MB_OK, MessageBoxW};
    use windows_strings::PCWSTR;

    pub fn message_box(title: &str, message: &str) {
        use std::os::windows::ffi::OsStrExt;

        let title_os = std::ffi::OsStr::new(&title);
        let title = title_os.encode_wide().chain(std::iter::once(0));
        let title = title.collect::<Vec<_>>();

        let message_os = std::ffi::OsStr::new(&message);
        let message = message_os.encode_wide().chain(std::iter::once(0));
        let message = message.collect::<Vec<_>>();

        unsafe {
            MessageBoxW(
                None,
                PCWSTR(message.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK,
            );
        }
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
                    $crate::utils::winapi::clear_last_error();
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
                $crate::utils::winapi::last_error().unwrap_or(anyhow::anyhow!("Unknown error")),
            )
        };
    }

    #[macro_export]
    macro_rules! wincall_into_result {
        ($fn:expr) => {{
            let res = $crate::wincall!($fn);
            $crate::utils::winapi::last_error().map_or_else(
                || Ok(res),
                |err| anyhow::Context::context(Err(err), $crate::function!()),
            )
        }};
    }
}

pub mod color {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Color {
        pub r: u8,
        pub g: u8,
        pub b: u8,
        pub a: u8,
    }

    impl Color {
        pub const fn from_abgr_packed(abgr: u32) -> Self {
            Self {
                a: ((abgr >> 24) & 0xFF) as u8,
                b: ((abgr >> 16) & 0xFF) as u8,
                g: ((abgr >> 8) & 0xFF) as u8,
                r: (abgr & 0xFF) as u8,
            }
        }

        pub const fn into_argb_packed(self) -> u32 {
            ((self.a as u32) << 24)
                | ((self.r as u32) << 16)
                | ((self.g as u32) << 8)
                | (self.b as u32)
        }
    }
}
