pub mod cast;
pub mod math;

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

// Code from near_o11y crate: https://github.com/near/nearcore and then adapted for my needs
pub mod invariants {
    ///
    /// If assert fails, panic on debug, and log error on release
    ///
    #[macro_export]
    macro_rules! assert_log {
        ($cond:expr) => {
            $crate::assert_log!($cond, "assertion failed: {}", stringify!($cond))
        };

        ($cond:expr, $fmt:literal $($arg:tt)*) => {
            if cfg!(debug_assertions) {
                assert!($cond, $fmt $($arg)*);
            } else {
                if !$cond {
                    log::error!($fmt $($arg)*);
                }
            }
        };
    }

    #[macro_export]
    macro_rules! assert_log_bail {
        ($cond:expr) => {
            $crate::assert_log_bail!($cond, "assertion failed: {}", stringify!($cond))
        };

        ($cond:expr, $fmt:literal $($arg:tt)*) => {
            if cfg!(debug_assertions) {
                assert!($cond, $fmt $($arg)*);
            } else {
                if !$cond {
                    log::error!($fmt $($arg)*);
                    return;
                }
            }
        };
    }

    #[macro_export]
    macro_rules! assert_log_fail {
        ($fmt:literal $($arg:tt)*) => {
            $crate::assert_log!(false, $fmt $($arg)*)
        };
    }
}
