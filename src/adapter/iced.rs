//! `From` conversions between winri's [`utils::math`] geometry and the
//! equivalent `iced` types, so the two can be passed back and forth freely.
use easy_ext::ext;

use crate::utils;

impl From<utils::math::Size> for iced::Size {
    fn from(value: utils::math::Size) -> Self {
        Self {
            width: value.width(),
            height: value.height(),
        }
    }
}

impl From<iced::Size> for utils::math::Size {
    fn from(value: iced::Size) -> Self {
        Self([value.width, value.height])
    }
}

impl From<utils::math::Position> for iced::Point {
    fn from(value: utils::math::Position) -> Self {
        Self {
            x: value.x(),
            y: value.y(),
        }
    }
}

impl From<iced::Point> for utils::math::Position {
    fn from(value: iced::Point) -> Self {
        Self([value.x, value.y])
    }
}

#[ext(CssColorExt)]
pub impl csscolorparser::Color {
    fn to_iced(&self) -> iced::Color {
        iced::Color::from_linear_rgba(self.r, self.g, self.b, self.a)
    }
}
