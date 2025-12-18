use crate::utils;

impl From<utils::math::Size> for iced::Size {
    fn from(value: utils::math::Size) -> Self {
        iced::Size {
            width: value.width(),
            height: value.height(),
        }
    }
}

impl From<iced::Size> for utils::math::Size {
    fn from(value: iced::Size) -> Self {
        utils::math::Size([value.width, value.height])
    }
}

impl From<utils::math::Position> for iced::Point {
    fn from(value: utils::math::Position) -> Self {
        iced::Point {
            x: value.x(),
            y: value.y(),
        }
    }
}

impl From<iced::Point> for utils::math::Position {
    fn from(value: iced::Point) -> Self {
        utils::math::Position([value.x, value.y])
    }
}
