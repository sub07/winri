mod hooks;
mod screen;
mod tiler;
mod window;

use std::{thread, time::Duration};

use crate::{
    tiler::ScrollTiler,
    window::{Window, filter::opened_windows},
};

fn main() -> anyhow::Result<()> {
    let window_event_notifier = hooks::launch_window_hook().unwrap();
    let mut tiler = ScrollTiler::new();

    for () in window_event_notifier {
        // Extremely weird bug: some windows api calls doesn't work if this sleep is not executed.
        // This is likely due to the threading context (the window event hook running in another thread for example, or not lol)
        thread::sleep(Duration::from_nanos(1));

        let windows_snapshot = opened_windows()?;
        for window in &windows_snapshot {
            Window::print_extensive_info(*window);
        }
        tiler.handle_window_snapshot(&windows_snapshot);
        tiler.layout_windows()?;
    }

    Ok(())
}
