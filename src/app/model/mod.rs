#[derive(Clone, Copy)]
pub struct BorderStyle {
    pub color: iced::Color,
    pub thickness: f32,
    pub radius: f32,
}

pub struct Configuration {
    pub tiler_border_style: BorderStyle,
}
