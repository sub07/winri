use joy_vector::gen_vector;

use crate::utils::cast::FaillibleCastUtils;

pub mod cast;
pub mod color;
pub mod frac;
pub mod winapi;

pub const IS_DEBUG: bool = cfg!(debug_assertions);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn position(&self) -> Position {
        [self.left, self.top].into()
    }

    pub fn size(&self) -> Size {
        [
            (self.right - self.left).cast(),
            (self.bottom - self.top).cast(),
        ]
        .into()
    }
}

gen_vector!(Position<i32, 2> with two_dim);
gen_vector!(Size<u32, 2> with two_dim);
