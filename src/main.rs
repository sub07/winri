#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release builds
#![warn(clippy::pedantic, clippy::nursery, clippy::dbg_macro)]
#![allow(
    clippy::missing_errors_doc,
    clippy::cast_possible_truncation,
    clippy::missing_const_for_fn,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::default_trait_access
)]

use std::{panic, path::PathBuf, sync::Arc};

use anyhow::{Context, anyhow};
use arc_swap::ArcSwap;
use windows::Win32::Foundation::HANDLE;

use crate::system::aquire_winri_running_lock;

mod adapter;
mod app;
mod bug_report;
mod config;
mod logger;
mod scroll_tiler;
mod system;
mod utils;
mod winapi;
mod window;

pub const DEBUG_MODE: bool = cfg!(debug_assertions);
/// The crate version, baked in at compile time. Surfaced in bug report URLs.
pub const WINRI_VERSION: &str = env!("CARGO_PKG_VERSION");

pub type Config = Arc<ArcSwap<config::Root>>;

/// winri's per-user data directory (`%APPDATA%/winri`, or `winri-dev` in debug
/// so a development build never clobbers a real install's logs/config).
pub fn root_dir() -> anyhow::Result<PathBuf> {
    const PROJECT_DIR_NAME: &str = if DEBUG_MODE { "winri-dev" } else { "winri" };
    Ok(dirs::config_dir()
        .ok_or_else(|| anyhow!("Could not determine config directory"))?
        .join(PROJECT_DIR_NAME))
}

fn main() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        log::error!("Winri panicked: {info}");
        bug_report::display_and_exit(info);
        system::restore_windows();
        default_hook(info);
        std::process::exit(1);
    }));

    if let Err(e) = logger::setup()
        .context("Could not initialize log system, no log will be written for this session")
    {
        bug_report::display_and_continue(e);
    }

    let _running_lock = match aquire_winri_running_lock() {
        Ok(Some(handle)) => handle,
        Ok(None) => {
            log::warn!("Another instance of Winri is already running, exiting now.");
            return;
        }
        Err(e) => {
            // In case of mutex claim error, we continue. It's better to risk two Winri instance than not being able to start the first instance.
            log::warn!("Error while getting running lock. Still continuing: {e:?}");
            HANDLE::default()
        }
    };

    log::info!("Winri starting up");

    // Needs to be behind config as not all users want to disable the lockscreen
    log::info!("Disabling lockscreen");
    if let Err(e) = system::disable_lock() {
        log::warn!("Could not disable lock: {e}");
    }

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
        bug_report::display_and_exit(anyhow!(e));
    }

    log::info!("Winri exited successfully");
}
