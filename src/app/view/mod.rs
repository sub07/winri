use crate::app;

pub mod overlay;

pub fn empty<'a>() -> iced::Element<'a, app::Message> {
    iced::widget::Row::new().into()
}
