mod action;
mod hook;
mod system;
mod thumbnail;
mod tiler;
mod utils;
mod window;

use std::{
    collections::{HashMap, HashSet},
    panic,
    sync::mpsc::Sender,
};

use anyhow::bail;
use itertools::Itertools;
use keyboard_types::Modifiers;
use log::{error, info};
use rdev::Key;

use crate::{
    action::{Action, TilerAction},
    hook::{
        key::{self},
        launch_hooks,
    },
    system::{restore_windows, screen_size},
    tiler::ScrollTiler,
    window::{
        Window,
        filter::opened_windows,
        manager::{BorderStyle, HandleOutputProtocol, ThumbnailId},
    },
};

pub enum Event {
    Key(key::Event),
    WindowManager(window::manager::OutputProtocolMessage),
    Window,
}

enum Mode {
    Tiler,
    Overview {
        thumbnails: HashMap<ThumbnailId, Window>,
    },
    ExitingSuccessfully,
    ExitingWithError(anyhow::Error),
}

pub struct Winri {
    mode: Mode,
    window_manager_client: window::manager::InputProtocolClient,
    tiler: ScrollTiler,
    event_rx: std::sync::mpsc::Receiver<Event>,
    event_tx: Sender<Event>,
}

fn get_process_names(windows: &HashSet<Window>) -> Vec<String> {
    windows
        .iter()
        .map(|w| {
            let is_focused = w.is_focused().unwrap_or(false);
            format!(
                "{}{}(class: {})",
                if is_focused { "[FOCUSED] " } else { "" },
                w.process_name()
                    .ok()
                    .unwrap_or_else(|| "[ERROR] Could not get process name".to_string()),
                w.class()
                    .unwrap_or_else(|_| "[ERROR] Could not get class name".to_string())
            )
        })
        .collect::<Vec<_>>()
}

impl window::manager::HandleOutputProtocol for Winri {
    fn cursor_entered_thumbnail(&mut self, id: ThumbnailId) {
        if let Err(e) = self.window_manager_client.border_thumbnail(id) {
            self.mode = Mode::ExitingWithError(e);
        }
    }

    fn cursor_exited_thumbnail(&mut self, _id: ThumbnailId) {
        if let Err(e) = self.window_manager_client.unborder_all_thumbnails() {
            self.mode = Mode::ExitingWithError(e);
        }
    }

    fn thumbnail_clicked(&mut self, id: ThumbnailId) {
        let Mode::Overview { thumbnails } = std::mem::replace(&mut self.mode, Mode::Tiler) else {
            return;
        };
        let Some(window) = thumbnails.get(&id) else {
            return;
        };
        self.window_manager_client.close_all_thumbnails().unwrap();
        window.focus().unwrap();
        self.update_tiler().unwrap();
    }

    fn unrecoverable_error(&mut self, err: anyhow::Error) {
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
            Event::Key(key_event) => {
                if let Some(action) = self.resolve_action(key_event) {
                    info!("Executing action: {action:?}");
                    match action {
                        Action::Tiler(tiler_action) => match tiler_action {
                            TilerAction::CloseCurrent => {
                                if let Some(window) = self.tiler.current_window() {
                                    window.close()?;
                                    self.update_tiler()?;
                                }
                            }
                            TilerAction::MoveFocusNext => {
                                self.tiler.focus_right();
                            }
                            TilerAction::MoveFocusPrevious => {
                                self.tiler.focus_left();
                            }
                            TilerAction::SwapWithNext => {
                                self.tiler.swap_current_right();
                                self.update_tiler()?;
                            }
                            TilerAction::SwapWithPrevious => {
                                self.tiler.swap_current_left();
                                self.update_tiler()?;
                            }
                            TilerAction::ResizeToFullscreen => {
                                self.tiler.set_current_window_fullscreen();
                                self.update_tiler()?;
                            }
                            TilerAction::ResizeToHalfScreen => {
                                self.tiler.set_current_window_halfscreen();
                                self.update_tiler()?;
                            }
                            TilerAction::OpenOverview => {
                                if matches!(self.mode, Mode::Overview { .. }) {
                                    return Ok(());
                                }
                                let windows = self.tiler.windows();

                                let windows_data = windows
                                    .map(|item| thumbnail::WindowData {
                                        inner: item.inner,
                                        width: item.width,
                                    })
                                    .collect_vec();

                                let thumbnails = thumbnail::create_thumbnails_from_tiler_windows(
                                    &windows_data,
                                    self.tiler.screen_size(),
                                    10,
                                );

                                for window in &windows_data {
                                    window.inner.move_offscreen()?;
                                }

                                let thumbnails = thumbnails
                                    .into_iter()
                                    .zip(windows_data)
                                    .map(|(thumbnail, window)| {
                                        self.window_manager_client
                                            .create_thumbnail(
                                                window.inner,
                                                thumbnail.pos,
                                                thumbnail.size,
                                            )
                                            .map(|id| (id, window.inner))
                                    })
                                    .collect::<anyhow::Result<HashMap<ThumbnailId, Window>>>()?;

                                self.mode = Mode::Overview { thumbnails };
                            }
                        },
                        Action::Overview(overview_action) => match overview_action {
                            action::OverviewAction::CloseOverview => {
                                if matches!(self.mode, Mode::Tiler) {
                                    return Ok(());
                                }
                                self.window_manager_client.close_all_thumbnails()?;
                                self.mode = Mode::Tiler;
                                self.event_tx.send(Event::Window)?;
                            }
                        },
                        Action::Exit => self.mode = Mode::ExitingSuccessfully,
                    }
                }
            }
            Event::Window => {
                if matches!(self.mode, Mode::Overview { .. }) {
                    return Ok(());
                }
                self.update_tiler()?;
            }
            Event::WindowManager(msg) => self.dispatch(msg),
        }
        Ok(())
    }

    fn resolve_action(&self, key::Event(modifiers, key): key::Event) -> Option<Action> {
        match (&self.mode, modifiers, key) {
            (Mode::Tiler, Modifiers::META, Key::LeftArrow) => {
                Some(Action::Tiler(TilerAction::MoveFocusPrevious))
            }
            (Mode::Tiler, Modifiers::META, Key::RightArrow) => {
                Some(Action::Tiler(TilerAction::MoveFocusNext))
            }
            (Mode::Tiler, _, Key::LeftArrow)
                if modifiers == Modifiers::META.union(Modifiers::CONTROL) =>
            {
                Some(Action::Tiler(TilerAction::SwapWithPrevious))
            }
            (Mode::Tiler, _, Key::RightArrow)
                if modifiers == Modifiers::META.union(Modifiers::CONTROL) =>
            {
                Some(Action::Tiler(TilerAction::SwapWithNext))
            }
            (Mode::Tiler, Modifiers::META, Key::KeyQ) => {
                Some(Action::Tiler(TilerAction::CloseCurrent))
            }
            (Mode::Tiler, Modifiers::META, Key::KeyF) => {
                Some(Action::Tiler(TilerAction::ResizeToFullscreen))
            }
            (Mode::Tiler, Modifiers::META, Key::KeyJ) => {
                Some(Action::Tiler(TilerAction::ResizeToHalfScreen))
            }
            (Mode::Tiler, Modifiers::META, Key::UpArrow) => {
                Some(Action::Tiler(TilerAction::OpenOverview))
            }
            (Mode::Overview { .. }, Modifiers::META, Key::DownArrow)
            | (Mode::Overview { .. }, Modifiers::META, Key::Escape) => {
                Some(Action::Overview(action::OverviewAction::CloseOverview))
            }
            (_, Modifiers::META, Key::Escape) => Some(Action::Exit),
            _ => None,
        }
    }

    fn run(mut self) -> anyhow::Result<()> {
        self.update_tiler()?;

        while let Ok(event) = self.event_rx.recv() {
            self.handle_event(event)?;

            match self.mode {
                Mode::ExitingSuccessfully => {
                    info!("Exiting successfully.");
                    break;
                }
                Mode::ExitingWithError(ref err) => {
                    bail!("Exiting due to unrecoverable error: {err:#}");
                }
                _ => {}
            }
        }

        Ok(())
    }
}

pub fn launch_winri() -> anyhow::Result<()> {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        error!("Winri panicked: {info}");
        system::restore_windows();
        if let Some(error_cause) = info.payload_as_str() {
            utils::winapi::message_box(
                "Fatal error",
                &format!("{error_cause}.\nThe application will now exit."),
            );
        }
        default_hook(info);
    }));

    pretty_env_logger::init();
    let screen_size = screen_size()?;
    let (event_tx, event_rx) = std::sync::mpsc::channel();

    launch_hooks(event_tx.clone())?;

    let window_manager_client = window::manager::launch(
        event_tx.clone(),
        BorderStyle {
            color: system::highlight_color(),
            thickness: 4,
        },
    )?;

    let app = Winri {
        mode: Mode::Tiler,
        window_manager_client,
        tiler: ScrollTiler::new(10, screen_size),
        event_rx,
        event_tx,
    };

    if let Err(e) = app.run() {
        error!("Fatal error: {e:?}");
        restore_windows();
        utils::winapi::message_box(
            "Fatal error",
            &format!("{e:#}.\nThe application will now exit."),
        );
    }

    restore_windows();
    Ok(())
}
