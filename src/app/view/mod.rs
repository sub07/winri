//! Rendering of winri's UI onto the overlay window.
use crate::app;

pub mod overlay;

/// An empty element, rendered by windows that have no visible content.
pub fn empty<'a>() -> iced::Element<'a, app::Message> {
    iced::widget::Row::new().into()
}
