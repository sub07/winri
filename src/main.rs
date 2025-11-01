mod hook;
mod system;
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
        thumbnail::{OutgoingEvent, launch_thumbnail_manager},
    },
    system::screen_size,
    tiler::ScrollTiler,
    window::{Window, filter::opened_windows},
};

pub enum Event {
    Key(key::Event),
    Thumbnail(OutgoingEvent),
    Window,
}

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

    let (event_tx, event_rx) = std::sync::mpsc::channel();

    launch_hooks(event_tx.clone())?;
    let thumbnail_manager = launch_thumbnail_manager(event_tx);

    let mut thumbnail_mode = false;

    for event in event_rx {
        match event {
            Event::Key(key::Event(modifiers, key)) => match key {
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
                Key::UpArrow if modifiers.contains(Modifiers::WIN) => {
                    let src = Window::focused().unwrap();
                    let rect = src.client_rect()?;
                    let width = rect.width / 2;
                    let height = rect.height / 2;
                    thumbnail_manager.create_thumbnail(src, 300, 300, width, height);
                    thumbnail_mode = true;
                }
                Key::Escape => {
                    thumbnail_manager.close_all_thumbnails();
                    thumbnail_mode = false;
                }
                _ => {}
            },
            Event::Window => {
                if thumbnail_mode {
                    continue;
                }
                update_tiler!();
            }
            Event::Thumbnail(outgoing_event) => match outgoing_event {
                OutgoingEvent::CursorEnteredThumbnail(thumbnail_descriptor) => {
                    thumbnail_manager.display_border(
                        thumbnail_descriptor,
                        4,
                        system::highlight_color(),
                    );
                }
                OutgoingEvent::CursorLeftThumbnail(thumbnail_descriptor) => {
                    thumbnail_manager.hide_border(thumbnail_descriptor);
                }
            },
        }
    }

    Ok(())
}
