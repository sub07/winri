mod hook;
mod system;
mod tiler;
mod utils;
mod window;

use std::collections::HashSet;

use anyhow::bail;
use joy_vector::Vector;
use log::{error, info};
use rdev::Key;

use crate::{
    hook::{
        key::{self, Modifiers},
        launch_hooks,
    },
    system::screen_size,
    tiler::ScrollTiler,
    window::{
        Window,
        filter::opened_windows,
        manager::{BorderStyle, HandleOutputProtocol, ThumbnailId},
    },
};

pub type Position = Vector<i32, 2>;
pub type Size = Vector<u32, 2>;

pub enum Event {
    Key(key::Event),
    WindowManager(window::manager::OutputProtocolMessage),
    Window,
}

enum Mode {
    Tiler,
    Overview,
    ExitingWithError(anyhow::Error),
}

pub struct Winri {
    mode: Mode,
    window_manager_client: window::manager::InputProtocolClient,
    tiler: ScrollTiler,
    event_rx: std::sync::mpsc::Receiver<Event>,
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

impl window::manager::HandleOutputProtocol for Winri {
    fn cursor_entered_thumbnail(&mut self, (): (), id: ThumbnailId) {
        if let Err(e) = self.window_manager_client.border_thumbnail(id) {
            self.mode = Mode::ExitingWithError(e);
        }
    }

    fn cursor_exited_thumbnail(&mut self, (): (), _id: ThumbnailId) {
        if let Err(e) = self.window_manager_client.unborder_all_thumbnails() {
            self.mode = Mode::ExitingWithError(e);
        }
    }

    fn unrecoverable_error(&mut self, (): (), err: anyhow::Error) {
        self.mode = Mode::ExitingWithError(err);
    }
}

impl Winri {
    fn update_tiler(&mut self) -> anyhow::Result<()> {
        let windows_snapshot = opened_windows()?;
        info!(
            "Opened windows: {:#?}",
            get_process_names(&windows_snapshot)
        );
        self.tiler.handle_window_snapshot(&windows_snapshot);
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        match event {
            Event::Key(key::Event(modifiers, key)) => match key {
                Key::LeftArrow if modifiers.contains(Modifiers::CTRL.union(Modifiers::WIN)) => {
                    self.tiler.swap_current_left();
                    self.update_tiler()?;
                }
                Key::RightArrow if modifiers.contains(Modifiers::CTRL.union(Modifiers::WIN)) => {
                    self.tiler.swap_current_right();
                    self.update_tiler()?;
                }
                Key::LeftArrow if modifiers.contains(Modifiers::WIN) => {
                    self.tiler.focus_left();
                }
                Key::RightArrow if modifiers.contains(Modifiers::WIN) => {
                    self.tiler.focus_right();
                }
                Key::UpArrow if modifiers.contains(Modifiers::WIN) => {
                    let src = Window::focused().unwrap();
                    let rect = src.client_rect()?;
                    let width = rect.width / 2;
                    let height = rect.height / 2;
                    self.window_manager_client.create_thumbnail(
                        src,
                        [300, 300].into(),
                        [width, height].into(),
                    )?;
                    self.mode = Mode::Overview;
                }
                Key::Escape => {
                    self.window_manager_client.close_all_thumbnails()?;
                    self.mode = Mode::Tiler;
                }
                _ => {}
            },
            Event::Window => {
                if matches!(self.mode, Mode::Overview) {
                    return Ok(());
                }
                self.update_tiler()?;
            }
            Event::WindowManager(msg) => self.dispatch((), msg),
        }
        if let Mode::ExitingWithError(err) = &self.mode {
            bail!("Exiting due to unrecoverable error: {err:#}");
        }
        Ok(())
    }

    fn run(mut self) -> anyhow::Result<()> {
        self.update_tiler()?;

        while let Ok(event) = self.event_rx.recv() {
            self.handle_event(event)?;
        }

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    let (screen_width, screen_height) = screen_size()?;
    let (event_tx, event_rx) = std::sync::mpsc::channel();

    launch_hooks(event_tx.clone())?;

    let window_manager_client = window::manager::launch(
        event_tx,
        BorderStyle {
            color: system::highlight_color(),
            thickness: 4,
        },
    )?;

    let app = Winri {
        mode: Mode::Tiler,
        window_manager_client,
        tiler: ScrollTiler::new(10, screen_width, screen_height),
        event_rx,
    };

    if let Err(e) = app.run() {
        error!("Fatal error: {e:?}");
        utils::winapi::message_box(
            "Fatal error",
            &format!("{e:#}.\nThe application will now exit."),
        );
    }
    Ok(())
}
