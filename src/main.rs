mod hooks;
mod screen;
mod tiler;
mod utils;
mod window;

use std::collections::HashSet;

use log::info;

use crate::{
    tiler::ScrollTiler,
    window::{Window, filter::opened_windows},
};

fn get_process_names(windows: &HashSet<Window>) -> Vec<String> {
    windows
        .iter()
        .map(|w| {
            w.process_name()
                .ok()
                .unwrap_or_else(|| "[ERROR] Could not get process name".to_string())
        })
        .collect::<Vec<_>>()
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let mut tiler = ScrollTiler::new();
    let windows_snapshot = opened_windows()?;
    info!(
        "Opened windows: {:#?}",
        get_process_names(&windows_snapshot)
    );
    tiler.handle_window_snapshot(&windows_snapshot);

    let window_event_notifier = hooks::launch_window_hook().unwrap();

    for () in window_event_notifier {
        let windows_snapshot = opened_windows()?;
        info!(
            "Opened windows: {:#?}",
            get_process_names(&windows_snapshot)
        );
        tiler.handle_window_snapshot(&windows_snapshot);
    }

    Ok(())
}
