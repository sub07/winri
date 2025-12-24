/// This module contains asynchronous `task`s for the iced runtime.
use anyhow::Context;
use iced::Task;
use joy_error::log::ResultLogExt;

use crate::{
    app::{self, HandleFaillibleProcessResultExt},
    system,
    window::Window,
};

pub fn ensure_overlay_not_focused(overlay_window_id: iced::window::Id) -> Task<app::Message> {
    iced::window::raw_id::<app::Message>(overlay_window_id).then(|raw_id| {
        unfocus_window(raw_id)
            .context("unfocusing overlay window")
            .error()
            .log_err()
            .discard();
        Task::none()
    })
}

fn unfocus_window(raw_id: u64) -> anyhow::Result<()> {
    let focused_window = Window::focused().context("getting focused window")?;

    let overlay_window = Window::from_safe_hwnd(raw_id).context(format!(
        "given raw id ({raw_id}) is invalid: expected overlay raw id (aka. HWND)"
    ))?;

    let desktop_window = system::get_desktop_window().context("getting desktop window")?;
    if focused_window == overlay_window {
        desktop_window.focus()?;
    }

    Ok(())
}
