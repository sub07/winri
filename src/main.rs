#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release builds
#![warn(clippy::pedantic, clippy::nursery, clippy::dbg_macro)]
#![allow(
    clippy::missing_errors_doc,
    clippy::cast_possible_truncation,
    clippy::missing_const_for_fn,
    clippy::needless_pass_by_value
)]

use std::{panic, path::PathBuf};

use anyhow::anyhow;

mod action;
mod adapter;
mod app;
mod logger;
mod scroll_tiler;
mod system;
mod utils;
mod winapi;
mod window;

pub const DEBUG_MODE: bool = cfg!(debug_assertions);

pub fn root_dir() -> anyhow::Result<PathBuf> {
    const PROJECT_DIR_NAME: &str = if DEBUG_MODE { "winri-dev" } else { "winri" };
    Ok(dirs::config_dir()
        .ok_or_else(|| anyhow!("Could not determine config directory"))?
        .join(PROJECT_DIR_NAME))
}

fn main() {
    log::info!("Winri starting up");
    if let Err(e) = logger::setup() {
        winapi::message_box(
            "Log initialization error",
            &format!("Could not initialize log system. No log will be written.\n\n{e}"),
        );
    }

    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        log::error!("Winri panicked: {info}");
        system::restore_windows();
        if let Some(error_cause) = info.payload_as_str() {
            winapi::message_box(
                "Fatal error",
                &format!("{error_cause}.\nThe application will now exit."),
            );
        }
        default_hook(info);
    }));

    if let Err(e) = iced::daemon(
        app::State::new,
        app::State::handle_app_message,
        app::State::view,
    )
    .subscription(app::State::subscription)
    .title(app::State::title)
    .theme(app::State::theme)
    .run()
    {
        log::error!("Winri exited with error: {e}");
    }

    log::info!("Winri exited successfully");
}
