use std::fmt::Display;

#[easy_ext::ext(FaillibleCastUtils)]
pub impl<T, R> T
where
    T: TryInto<R> + Display + Clone + Copy,
{
    fn cast(self) -> R {
        self.try_cast().expect("Cast failed")
    }

    fn try_cast(self) -> anyhow::Result<R> {
        self.try_into().map_err(|_| {
            anyhow::anyhow!(
                "Cast from {} with value {self} to {} failed",
                std::any::type_name::<T>(),
                std::any::type_name::<R>()
            )
        })
    }
}

#[easy_ext::ext(InfaillibleCastUtils)]
pub impl<T, R> T
where
    T: Into<R> + Display + Clone + Copy,
{
    fn infaillible_cast(self) -> R {
        self.into()
    }
}

#[macro_export]
macro_rules! cast {
    ($src:expr => $t:ty as $dest:ident, $($rem:tt)*) => {
        let $dest: $t = $crate::utils::cast::FaillibleCastUtils::cast($src);
        cast!($($rem)*);
    };
    ($i:ident => $t:ty, $($rem:tt)*) => {
        let $i: $t = $crate::utils::cast::FaillibleCastUtils::cast($i);
        cast!($($rem)*);
    };
    () => {};
}

#[macro_export]
macro_rules! try_cast {
    ($src:expr => $t:ty as $dest:ident, $($rem:tt)*) => {
        let $dest: $t = $crate::utils::cast::FaillibleCastUtils::try_cast($src)?;
        try_cast!($($rem)*);
    };
    ($i:ident => $t:ty, $($rem:tt)*) => {
        let $i: $t = $crate::utils::cast::FaillibleCastUtils::try_cast($i)?;
        try_cast!($($rem)*);
    };
    () => {};
}

#[macro_export]
macro_rules! infaillible_cast {
    ($src:expr => $t:ty as $dest:ident, $($rem:tt)*) => {
        let $dest: $t = $crate::utils::cast::InfaillibleCastUtils::infaillible_cast($src);
        infaillible_cast!($($rem)*);
    };
    ($i:ident => $t:ty, $($rem:tt)*) => {
        let $i: $t = $crate::utils::cast::InfaillibleCastUtils::infaillible_cast($i);
        infaillible_cast!($($rem)*);
    };
    () => {};
}
