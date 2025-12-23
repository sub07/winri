pub type ThumbnailId = isize;

use iced::{
    Task,
    window::{
        Settings,
        settings::{PlatformSpecific, platform::CornerPreference},
    },
};
use log::debug;
use windows::Win32::{
    Foundation::RECT,
    Graphics::Dwm::{
        DWM_THUMBNAIL_PROPERTIES, DWM_TNP_RECTDESTINATION, DWM_TNP_VISIBLE, DwmRegisterThumbnail,
        DwmUnregisterThumbnail, DwmUpdateThumbnailProperties,
    },
};

use crate::{
    app::service::overview::{self, ThumbnailWindowCreated},
    utils::math::{Position, Size},
    wincall_result,
    window::Window,
};

pub struct ThumbnailData {
    pub pos: Position,
    pub size: Size,
}

pub struct WindowData {
    pub inner: Window,
    pub width: f32,
}

pub fn compute_thumbnails_bounds_from_tiler_windows(
    windows: &[WindowData],
    screen_size: Size,
    padding: f32,
) -> Vec<ThumbnailData> {
    // Width of packed windows
    let total_tiler_width = windows.iter().map(|w| w.width + padding).sum::<f32>() - padding;

    let reduction_ratio = screen_size.width() / total_tiler_width;

    debug!("total tiler width including padding: {total_tiler_width}");
    debug!("Thumbnail reduction ratio for packing windows: {reduction_ratio}");

    let reduction_ratio = if reduction_ratio > 1.0 {
        reduction_ratio * 0.6
    } else if reduction_ratio < 1.0 {
        reduction_ratio * 0.9
    } else {
        reduction_ratio
    }
    .clamp(0.0, 0.6);

    debug!("Thumbnail reduction ratio after size adaptation: {reduction_ratio}");

    let mut current_x = 0.0;
    let mut thumbnails = Vec::new();

    let thumbnail_height = reduction_ratio * screen_size.height();
    let thumbnail_y = (screen_size.height() - thumbnail_height).abs() / 2.0;
    let thumbnail_x_center_offset =
        (screen_size.width() - reduction_ratio * total_tiler_width).abs() / 2.0;

    for window in windows {
        let width = reduction_ratio * window.width;
        thumbnails.push(ThumbnailData {
            pos: [current_x + thumbnail_x_center_offset, thumbnail_y].into(),
            size: [width, thumbnail_height].into(),
        });
        current_x += width + padding;
    }

    thumbnails
}

pub fn thumbnail_window_creation_task(
    source_window: Window,
    at: Position,
    size: Size,
) -> Task<overview::Message> {
    let (_, window_creation) = iced::window::open(Settings {
        decorations: false,
        size: size.into(),
        position: iced::window::Position::Specific(at.into()),
        resizable: false,
        visible: false,
        platform_specific: PlatformSpecific {
            skip_taskbar: true,
            corner_preference: CornerPreference::Round,
            ..Default::default()
        },
        ..Default::default()
    });

    window_creation
        .then(|id| {
            iced::window::raw_id::<overview::Message>(id)
                .then(move |raw_handle| Task::done((id, raw_handle)))
        })
        .then(move |(id, raw_handle)| {
            iced::window::enable_mouse_passthrough(id).chain(Task::done(
                overview::Message::ThumbnailWindowCreated(ThumbnailWindowCreated {
                    src: source_window,
                    dest_id: id,
                    dest_raw_handle: raw_handle,
                    size,
                }),
            ))
        })
}

pub fn bind_thumbnail(src: Window, dest: Window, size: Size) -> anyhow::Result<ThumbnailId> {
    let thumbnail_id = wincall_result!(DwmRegisterThumbnail(dest.handle(), src.handle()))?;

    let thumbnail_props = DWM_THUMBNAIL_PROPERTIES {
        dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE,
        rcDestination: RECT {
            left: 0,
            top: 0,
            right: size.width() as i32,
            bottom: size.height() as i32,
        },
        rcSource: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        opacity: 0,
        fVisible: true.into(),
        fSourceClientAreaOnly: true.into(),
    };

    wincall_result!(DwmUpdateThumbnailProperties(
        thumbnail_id,
        &raw const thumbnail_props
    ))?;

    Ok(thumbnail_id)
}

pub fn unbind_thumbnail(thumbnail_id: ThumbnailId) -> anyhow::Result<()> {
    wincall_result!(DwmUnregisterThumbnail(thumbnail_id))?;
    Ok(())
}
