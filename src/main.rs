mod hook;
mod screen;
mod tiler;
mod utils;
mod window;

use std::collections::HashSet;

use log::info;

use crate::{
    hook::window::launch_window_hook,
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

    let mut tiler = ScrollTiler::with_padding(10);

    macro_rules! update_tiler {
        () => {
            let windows_snapshot = opened_windows()?;
            info!(
                "Opened windows: {:#?}",
                get_process_names(&windows_snapshot)
            );
            tiler.handle_window_snapshot(&windows_snapshot);
        };
    }

    update_tiler!();

    let window_event_notifier = launch_window_hook().unwrap();

    for () in window_event_notifier {
        update_tiler!();
    }

    Ok(())
}
