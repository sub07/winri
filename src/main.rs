mod hook;
mod screen;
mod tiler;
mod utils;
mod window;

use std::collections::HashSet;

use log::info;
use rdev::Key;

use crate::{
    hook::{
        key::{self, Modifiers},
        launch_hooks,
    },
    screen::screen_size,
    tiler::ScrollTiler,
    window::{Window, filter::opened_windows},
};

fn get_process_names(windows: &HashSet<Window>) -> Vec<String> {
    windows
        .iter()
        .map(|w| {
            let is_focused = w.is_focused().unwrap_or(false);
            format!(
                "{}{}",
                if is_focused { "[FOCUSED] " } else { "" },
                w.process_name()
                    .ok()
                    .unwrap_or_else(|| "[ERROR] Could not get process name".to_string())
            )
        })
        .collect::<Vec<_>>()
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let (screen_width, screen_height) = screen_size()?;

    let mut tiler = ScrollTiler::new(10, screen_width, screen_height);

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

    let events = launch_hooks()?;

    for event in events {
        match event {
            hook::Event::Key(key::Event(modifiers, key)) => match key {
                Key::LeftArrow if modifiers.contains(Modifiers::CTRL.union(Modifiers::WIN)) => {
                    tiler.swap_current_left();
                    update_tiler!();
                }
                Key::RightArrow if modifiers.contains(Modifiers::CTRL.union(Modifiers::WIN)) => {
                    tiler.swap_current_right();
                    update_tiler!();
                }
                Key::LeftArrow if modifiers.contains(Modifiers::WIN) => {
                    tiler.focus_left();
                }
                Key::RightArrow if modifiers.contains(Modifiers::WIN) => {
                    tiler.focus_right();
                }
                _ => {}
            },
            hook::Event::Window => {
                update_tiler!();
            }
        }
    }

    Ok(())
}
