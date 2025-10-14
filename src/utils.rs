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

pub mod winapi {
    use windows::Win32::Foundation::{GetLastError, SetLastError, WIN32_ERROR};

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
                    winapi::clear_last_error();
                    $fn
                }
            }
        };
    }

    #[macro_export]
    macro_rules! wincall_result {
        ($fn:expr) => {
            $crate::wincall!($fn)
                .context($crate::function!())
                .context(winapi::last_error().unwrap_or(anyhow::anyhow!("Unknown error")))
        };
    }

    #[macro_export]
    macro_rules! wincall_into_result {
        ($fn:expr) => {{
            let res = $crate::wincall!($fn);
            winapi::last_error()
                .map_or_else(|| Ok(res), |err| Err(err).context($crate::function!()))
        }};
    }
}
