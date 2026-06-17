use iced::{
    Renderer, Theme, mouse,
    widget::{
        self,
        canvas::{self, Path, Stroke},
    },
};

use crate::{
    adapter::iced::CssColorExt,
    app::{self, service::tiler::State, view},
    config,
    utils::math::Bounds,
};

/// Renders the overlay's contents for the current mode. Only tiler mode draws
/// anything (the focused-window border); every other mode renders empty.
pub fn view(app: &app::State) -> iced::Element<'_, app::Message> {
    match &app.mode {
        app::Mode::Tiler(tiler_state) => tiler_view(app, tiler_state),
        _ => view::empty(),
    }
}

fn tiler_view<'a>(app: &'a app::State, tiler_state: &'a State) -> iced::Element<'a, app::Message> {
    if let Some(border_bounds) = tiler_state.current_border_bounds {
        widget::canvas(TilerBorder {
            border_bounds,
            border_thickness: app.config.default_window.border_thickness,
            border_color: app.config.default_window.border_color.to_iced(),
            border_radius: app.config.default_window.border_radius,
        })
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
    } else {
        view::empty()
    }
}

struct TilerBorder {
    border_bounds: Bounds,
    border_thickness: f32,
    border_color: iced::Color,
    border_radius: f32,
}

impl canvas::Program<app::Message> for TilerBorder {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let path = Path::rounded_rectangle(
            self.border_bounds.position().into(),
            self.border_bounds.size().into(),
            self.border_radius.into(),
        );

        frame.stroke(
            &path,
            Stroke::default()
                .with_color(self.border_color)
                .with_width(self.border_thickness),
        );

        vec![frame.into_geometry()]
    }
}
