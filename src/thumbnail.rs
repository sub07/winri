use log::debug;

use crate::{
    Position, Size, cast, f,
    utils::frac::{self, f},
    window::Window,
};

pub struct ThumbnailData {
    pub pos: Position,
    pub size: Size,
}

pub struct WindowData {
    pub inner: Window,
    pub width: u32,
}

pub fn create_thumbnails_from_tiler_windows(
    windows: &[WindowData],
    screen_size: Size,
    padding: u32,
) -> Vec<ThumbnailData> {
    // Width of packed windows
    let total_tiler_width = windows.iter().map(|w| w.width + padding).sum::<u32>() - padding;

    let reduction_ratio = f(screen_size.w(), total_tiler_width);

    debug!("Thumbnail reduction ratio: {reduction_ratio}");

    let reduction_ratio = match reduction_ratio.category() {
        frac::Category::Upside => reduction_ratio * f!(6 / 10),
        frac::Category::Downside => reduction_ratio * f!(9 / 10),
        frac::Category::Equal => reduction_ratio,
    };

    let mut current_x = 0;
    let mut thumbnails = Vec::new();

    let thumbnail_height = reduction_ratio * screen_size.h();
    let thumbnail_y = screen_size.h().abs_diff(thumbnail_height) / 2;
    let thumbnail_x_center_offset = screen_size
        .w()
        .abs_diff(reduction_ratio * total_tiler_width)
        / 2;

    cast! {
        thumbnail_y => i32,
        thumbnail_x_center_offset => i32,
        padding => i32,
    }

    for window in windows {
        let width = reduction_ratio * window.width;
        thumbnails.push(ThumbnailData {
            pos: [current_x + thumbnail_x_center_offset, thumbnail_y].into(),
            size: [width, thumbnail_height].into(),
        });
        cast! {
            width => i32,
        }
        current_x += width + padding;
    }

    thumbnails
}
