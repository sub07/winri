use std::fmt::Display;

use crate::infaillible_cast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frac {
    pub numerator: u32,
    pub denominator: u32,
}

impl Display for Frac {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_f32())
    }
}

pub enum Category {
    /// val > 1
    Upside,
    /// val < 1
    Downside,
    /// val == 1
    Equal,
}

pub const fn f(numerator: u32, denominator: u32) -> Frac {
    Frac::new(numerator, denominator)
}

impl Frac {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn as_f32(self) -> f32 {
        self.numerator as f32 / self.denominator as f32
    }

    pub fn category(self) -> Category {
        match self.numerator.cmp(&self.denominator) {
            std::cmp::Ordering::Less => Category::Downside,
            std::cmp::Ordering::Equal => Category::Equal,
            std::cmp::Ordering::Greater => Category::Upside,
        }
    }

    pub const fn is_unit(self) -> bool {
        self.numerator == self.denominator
    }
}

impl std::ops::Mul<u32> for Frac {
    type Output = u32;

    fn mul(self, rhs: u32) -> Self::Output {
        if self.is_unit() {
            return rhs;
        }
        infaillible_cast! {
            rhs => u64,
            (self.numerator) => u64 as numerator,
            (self.denominator) => u64 as denominator,
        }
        (rhs * numerator / denominator)
            .try_into()
            .expect("Multiplication overflowed u32")
    }
}

impl std::ops::Mul<Frac> for u32 {
    type Output = Self;

    fn mul(self, rhs: Frac) -> Self::Output {
        rhs * self
    }
}

impl std::ops::Mul<Self> for Frac {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        if self.is_unit() {
            return rhs;
        }
        if rhs.is_unit() {
            return self;
        }
        Self::new(
            self.numerator * rhs.numerator,
            self.denominator * rhs.denominator,
        )
    }
}

#[macro_export]
macro_rules! f {
    ($n:literal / $d:literal) => {
        $crate::utils::frac::Frac::new($n, $d)
    };
}

#[cfg(test)]
mod test {
    #[test]
    fn test_frac() {
        let f = f!(1920 / 1080);
        assert_eq!(f.numerator, 16);
        assert_eq!(f.denominator, 9);
        assert_eq!(f * 1080, 1920);

        let f = f!(1280 / 720);
        assert_eq!(f.numerator, 16);
        assert_eq!(f.denominator, 9);

        let f = f!(1024 / 768);
        assert_eq!(f.numerator, 4);
        assert_eq!(f.denominator, 3);

        let f = f!(500 / 1000);
        assert_eq!(f.numerator, 1);
        assert_eq!(f.denominator, 2);
        assert_eq!(20, f * 41);
    }
}
