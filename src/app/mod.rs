//! The root app module: it ties together every winri subsystem behind the
//! iced daemon. The flow is `subscription` (global keyboard + window hooks)
//! -> [`Message`] -> [`State::handle_app_message`] -> [`Task`]s, with the
//! current [`Mode`] deciding how input is interpreted and what is rendered.
mod action;
mod service;
mod subscription;
pub mod task;
mod view;

use std::{panic, path::PathBuf, sync::Arc};

use anyhow::Context;
use arc_swap::ArcSwap;
use iced::{
    Color, Task,
    theme::palette::Seed,
    window::{Settings, settings::PlatformSpecific},
};
use joy_error::ResultUtilityExt;

use crate::{
    Config,
    app::{
        service::{
            overview::{self},
            tiler::{self},
        },
        subscription::global::GlobalMessage,
    },
    assert_log_fail, bug_report, config,
    scroll_tiler::ScrollTiler,
    system::{self, message_box_query},
    utils::math::Size,
    window::{self},
};

/// The whole application state. A single instance lives for the program's
/// lifetime and is mutated in place by [`State::handle_app_message`].
pub struct State {
    pub tiler: ScrollTiler,
    pub mode: Mode,
    pub config: Config,
    pub config_source: Option<PathBuf>,
    overlay_window_id: iced::window::Id,
    /// `Ok(true)` if screen locking feature was enabled when vim mode was enabled.
    /// `Ok(false)` if screen locking feature was disabled when vim mode was enabled.
    /// `None` if the lock state detection failed.
    /// If `None`, the lock state restoration is skipped.
    pub initial_lock: Option<bool>,
}

/// The mutually exclusive states winri can be in. The active mode decides which
/// key bindings are live (see [`State::resolve_action`]) and what is rendered.
pub enum Mode {
    Tiler(tiler::State),
    Overview(overview::State),
    /// Transient state requesting a clean shutdown on the next message pump.
    Exit,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Tiler(tiler::State::default())
    }
}

/// Every event the iced runtime can deliver to [`State::handle_app_message`].
#[derive(Debug, Clone)]
pub enum Message {
    Init,
    Action(action::Action),
    Overview(overview::Message),
    Global(subscription::global::GlobalMessage),
    CleanupAndExit,
}

/// Creates the full-screen, transparent, click-through overlay window used to
/// draw winri's UI (currently just the focused-window border) on top of every
/// other window without intercepting any input.
fn create_overlay_window(screen_size: Size) -> (iced::window::Id, Task<Message>) {
    let (id, task) = iced::window::open(Settings {
        decorations: false,
        transparent: true,
        resizable: false,
        closeable: false,
        level: iced::window::Level::AlwaysOnTop,
        position: iced::window::Position::Specific(iced::Point::ORIGIN),
        size: screen_size.into(),
        platform_specific: PlatformSpecific {
            skip_taskbar: true,
            ..Default::default()
        },
        ..Default::default()
    });

    (id, task.then(iced::window::enable_mouse_passthrough))
}

impl State {
    /// Builds the initial state and the task that opens the overlay window.
    /// This is the `new` callback handed to `iced::daemon`.
    pub fn new() -> (Self, Task<Message>) {
        const MESSAGE: &str = r"
The config file could not be loaded.
Do you want to continue with default values ?
";
        let mut init_tasks = Vec::new();

        let (config, config_source) = match config::load() {
            Ok(config) => config,
            Err(err) => {
                log::error!("could not load config: {err:?}");
                let should_continue = message_box_query("configuration loading error", MESSAGE);
                if !should_continue {
                    log::info!(
                        "user chose to close Winri instead of continuing with default config"
                    );
                    init_tasks.push(Task::done(Message::CleanupAndExit));
                }
                (config::Root::default(), None)
            }
        };

        let config = Arc::new(ArcSwap::from_pointee(config));
        let panic_config = config.clone();

        let initial_lock_state = system::is_lock_enabled();
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            log::error!("Winri panicked: {info}");
            bug_report::display_and_exit(info);
            system::restore_windows();
            if panic_config.load().vim_mode
                && let Ok(initial_lock_state) = initial_lock_state
            {
                if initial_lock_state {
                    system::enable_lock().discard();
                } else {
                    system::disable_lock().discard();
                }
            }
            default_hook(info);
            std::process::exit(1);
        }));

        let screen_size = system::screen_size().expect("Screen size retrieval");
        let work_area = system::work_area().expect("Work area retrieval");
        let tiler = ScrollTiler::new(config.clone(), work_area);
        let (overlay_window_id, overlay_window_creation_task) = create_overlay_window(screen_size);
        init_tasks.push(overlay_window_creation_task);

        (
            Self {
                tiler,
                mode: Mode::default(),
                config,
                config_source,
                overlay_window_id,
                initial_lock: system::is_lock_enabled().ok(),
            },
            Task::done(Message::Init).chain(Task::batch(init_tasks)),
        )
    }

    /// Title for every winri-owned window. Deliberately the "ignore" marker so
    /// winri never tries to tile its own windows (see [`window::filter`]).
    pub fn title(_: &Self, _window_id: iced::window::Id) -> String {
        window::filter::WINRI_IGNORED_WINDOW_TITLE_SUBSTRING.into()
    }

    /// Central message dispatch (the daemon's `update` callback): routes each
    /// [`Message`] to its handler and chains the resulting tasks. Also re-asserts
    /// that the overlay never holds focus, and triggers shutdown once
    /// [`Mode::Exit`] has been set.
    pub fn handle_app_message(&mut self, message: Message) -> Task<Message> {
        let mut task = Task::none();

        // HACK: By default the overlay window steals focus when created, but should not be able to be focused.
        // It causes weird behavior like keystroke not recorded until another window is focused.
        // So we refocus the desktop window after creation.
        task = task.chain(task::ensure_overlay_not_focused(self.overlay_window_id));

        match message {
            Message::Init => {
                self.reconcile_lock_state();
            }
            Message::Global(global_message) => {
                task = task.chain(self.handle_global_message(global_message));
            }
            Message::CleanupAndExit => {
                system::restore_windows();
                self.restore_lock_state();
                return iced::exit();
            }
            Message::Overview(message) => self
                .handle_overview_message(message)
                .handle_faillible_process()
                .discard(),
            Message::Action(action) => {
                if let Ok(action_task) = self
                    .handle_action(action)
                    .context("action handling")
                    .handle_faillible_process()
                {
                    task = task.chain(action_task);
                }
            }
        }
        if matches!(self.mode, Mode::Exit) {
            task = task.chain(Task::done(Message::CleanupAndExit));
        }
        task
    }

    /// Handles a raw hook event: maps a keystroke to an [`action::Action`] for
    /// the current mode, or refreshes the tiler when the window set changes.
    pub fn handle_global_message(&mut self, message: GlobalMessage) -> Task<Message> {
        match message {
            GlobalMessage::Key(modifiers, key) => {
                if let Some(action) = self.resolve_action(modifiers, key) {
                    return Task::done(Message::Action(action));
                }
            }
            GlobalMessage::Window => {
                self.update_tiler()
                    .context("global window event")
                    .handle_faillible_process()
                    .discard();
            }
            GlobalMessage::ConfigChanged(path) => {
                let config = config::load_from(path).unwrap_or_default();
                self.config.store(Arc::new(config));
                self.reconcile_lock_state();
            }
        }
        Task::none()
    }

    /// The daemon's `view` callback. Only the overlay window has content; every
    /// other window (e.g. thumbnail windows) renders empty.
    pub fn view(&self, window_id: iced::window::Id) -> iced::Element<'_, Message> {
        if window_id == self.overlay_window_id {
            view::overlay::view(self)
        } else {
            view::empty()
        }
    }

    /// The daemon's `theme` callback. The overlay uses a fully transparent
    /// background so only what we draw is visible over the desktop.
    pub fn theme(&self, window_id: iced::window::Id) -> iced::Theme {
        if window_id == self.overlay_window_id {
            iced::Theme::custom(
                "Overlay transparent theme",
                Seed {
                    background: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
                    ..Seed::DARK
                },
            )
        } else {
            iced::Theme::Dark // TODO: Adapt to system theme
        }
    }

    /// The daemon's `subscription` callback: starts the global keyboard and
    /// window-event hooks and streams their events back as [`Message`]s.
    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::run_with(
            self.config_source.clone(),
            subscription::global::subscription,
        )
    }
}

/// Extension that logs (and in debug, panics on) an `Err` while passing the
/// `Result` through unchanged, so fallible side-tasks can be fire-and-forget
/// without silently swallowing failures.
#[easy_ext::ext(HandleFaillibleProcessResultExt)]
impl<T, E: std::fmt::Debug> Result<T, E> {
    fn handle_faillible_process(self) -> Self {
        match &self {
            Ok(_) => {}
            Err(e) => {
                assert_log_fail!("{:?}", e);
            }
        }
        self
    }
}
