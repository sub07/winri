/// Batch numeric `as` casts with the relevant clippy lints pre-silenced.
///
/// winri does a lot of `f32`/`i32`/`usize` juggling around screen coordinates
/// where the casts are known-safe; this keeps that noise in one place. Each
/// entry is either `expr => Type as name` (binds `name`) or `ident => Type`
/// (rebinds `ident` in place).
#[macro_export]
macro_rules! cast {
    ($src:expr => $t:ty as $dest:ident, $($rem:tt)*) => {
        #[allow(clippy::cast_possible_wrap)]
        #[allow(clippy::cast_precision_loss)]
        let $dest: $t = $src as $t;
        cast!($($rem)*);
    };
    ($i:ident => $t:ty, $($rem:tt)*) => {
        #[allow(clippy::cast_possible_wrap)]
        #[allow(clippy::cast_precision_loss)]
        let $i: $t = $i as $t;
        cast!($($rem)*);
    };
    () => {};
}
