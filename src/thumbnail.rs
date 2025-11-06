use crate::{
    Position, Size, cast,
    utils::{CastUtils, ratio},
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
) -> Vec<ThumbnailData> {
    // Width of packed windows
    let total_tiler_width = windows.iter().map(|w| w.width).sum::<u32>();

    let reduction_ratio = ratio(screen_size.w(), total_tiler_width);

    let mut current_x = 0;
    let mut thumbnails = Vec::new();

    for window in windows {
        let width = reduction_ratio * window.width;
        thumbnails.push(ThumbnailData {
            pos: [current_x, 0].into(),
            size: [width, reduction_ratio * screen_size.h()].into(),
        });
        cast! {
            width => i32,
        }
        current_x += width;
    }

    thumbnails
}
