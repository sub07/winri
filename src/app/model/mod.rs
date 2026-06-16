#[derive(Clone, Copy)]
pub struct BorderStyle {
    pub color: iced::Color,
    pub thickness: f32,
    pub radius: f32,
}

/// User-facing settings. Currently in-memory only (defaults set in
/// [`crate::app::State::new`]); a future home for persisted preferences.
pub struct Configuration {
    pub tiler_border_style: BorderStyle,
}
