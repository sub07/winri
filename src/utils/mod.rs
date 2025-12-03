use joy_vector::gen_vector;

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
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

gen_vector!(Position<i32, 2> with two_dim);
gen_vector!(Size<u32, 2> with two_dim);
