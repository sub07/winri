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
