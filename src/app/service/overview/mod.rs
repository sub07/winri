mod thumbnail;

use anyhow::Context;
use iced::Task;
use itertools::Itertools;

use crate::{
    app::{
        self, Mode,
        service::overview::{self, thumbnail::ThumbnailId},
    },
    window::Window,
};

pub struct State {
    opened_thumbnails: Vec<(ThumbnailId, iced::window::Id)>,
}

#[derive(Debug, Clone)]
pub struct ThumbnailWindowCreated {
    pub src: Window,
    pub dest_id: iced::window::Id,
    pub dest_raw_handle: u64,
    pub size: crate::utils::math::Size,
}

#[derive(Debug, Clone)]
pub enum Message {
    ThumbnailWindowCreated(ThumbnailWindowCreated),
}

impl app::State {
    pub fn prepare_open_overview(&self) -> Task<app::Message> {
        if matches!(self.mode, Mode::Overview(_)) {
            log::warn!(
                "Overview operation requested in {} while already in Overview mode",
                crate::function!()
            );
            return Task::none();
        }

        let windows = self.tiler.windows();

        let windows_data = windows
            .map(|item| thumbnail::WindowData {
                inner: item.inner,
                width: item.width,
            })
            .collect_vec();

        let thumbnails_data = thumbnail::compute_thumbnails_bounds_from_tiler_windows(
            &windows_data,
            self.tiler.screen_size(),
            10.0,
        );

        Task::batch(thumbnails_data.into_iter().zip(windows_data).map(
            |(thumbnail_data, window)| {
                thumbnail::thumbnail_window_creation_task(
                    window.inner,
                    thumbnail_data.pos,
                    thumbnail_data.size,
                )
                .then(|overview_message| Task::done(app::Message::Overview(overview_message)))
            },
        ))
    }

    pub fn handle_overview_message(&mut self, message: overview::Message) -> anyhow::Result<()> {
        match message {
            overview::Message::ThumbnailWindowCreated(thumbnail_window_created) => {
                self.finalize_open_overview(thumbnail_window_created)?;
            }
        }

        Ok(())
    }

    fn finalize_open_overview(
        &mut self,
        ThumbnailWindowCreated {
            src,
            dest_id,
            dest_raw_handle,
            size,
        }: ThumbnailWindowCreated,
    ) -> anyhow::Result<()> {
        // Switch to overview mode if not already in it
        if !matches!(self.mode, Mode::Overview(_)) {
            log::info!("switching to Overview mode");
            self.mode = Mode::Overview(State {
                opened_thumbnails: Vec::new(),
            });
        }

        let Mode::Overview(State {
            opened_thumbnails: thumbnails,
        }) = &mut self.mode
        else {
            unreachable!("checked above that we are in Overview mode")
        };

        for tiled_window in self.tiler.windows() {
            tiled_window.inner.move_offscreen()?;
        }

        let dest_window = Window::from_safe_hwnd(dest_raw_handle)
            .context(dest_raw_handle)
            .context("invalid hwnd from thumbnail window")?;

        dest_window.set_no_activate()?;

        let thumbnail_id =
            thumbnail::bind_thumbnail(src, dest_window, size).context("thumbnail binding")?;

        dest_window.show()?;
        dest_window.set_max_zindex()?;

        thumbnails.push((thumbnail_id, dest_id));

        Ok(())
    }

    pub fn close_overview(&mut self) -> anyhow::Result<Task<app::Message>> {
        let Mode::Overview(State {
            opened_thumbnails: thumbnails,
        }) = &self.mode
        else {
            log::warn!(
                "Close overview requested in {} while not in Overview mode",
                crate::function!()
            );
            return Ok(Task::none());
        };

        let tasks = thumbnails
            .iter()
            .map(|(thumbnail_id, window_id)| {
                thumbnail::unbind_thumbnail(*thumbnail_id).map(|()| window_id)
            })
            .map(|window_id| window_id.map(|id| iced::window::close(*id)))
            .collect::<anyhow::Result<Vec<_>>>()?;

        self.switch_to_tiler_mode()?;

        Ok(Task::batch(tasks))
    }
}
