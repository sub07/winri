//! Asynchronous `Task`s for the iced runtime.
use anyhow::Context;
use iced::Task;
use joy_error::{ResultUtilityExt, log::ResultLogExt};

use crate::{
    app::{self},
    system,
    window::Window,
};

/// Refocuses the desktop if the overlay currently holds focus.
///
/// The overlay steals focus when first shown, which breaks keystroke capture
/// until another window is clicked. Run after every message as a cheap guard
/// against that state.
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
