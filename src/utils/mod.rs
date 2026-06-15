//! Cross-cutting helpers: geometry types, numeric-cast sugar, and the
//! assertion/diagnostic macros used throughout the crate.
pub mod cast;
pub mod math;

/// Expands to the fully-qualified name of the enclosing function as a `&str`.
/// Used to tag Win32 errors and log messages with their origin.
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

/// Assertion macros that fail loudly in debug but degrade to logging in
/// release, so a broken invariant crashes during development yet never takes
/// down a user's session in production.
// Code from near_o11y crate: https://github.com/near/nearcore and then adapted for my needs
pub mod invariants {
    /// Assert a condition: panics on debug, logs an error on release.
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

    /// Like [`assert_log`] but, on release, `return`s from the caller when the
    /// condition fails instead of continuing.
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

    /// Unconditional failure: panics on debug, logs the message on release.
    /// For "this should never happen" branches.
    #[macro_export]
    macro_rules! assert_log_fail {
        ($fmt:literal $($arg:tt)*) => {
            $crate::assert_log!(false, $fmt $($arg)*)
        };
    }
}
