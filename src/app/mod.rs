//! The root app module: it ties together every winri subsystem behind the
//! iced daemon. The flow is `subscription` (global keyboard + window hooks)
//! -> [`Message`] -> [`State::handle_app_message`] -> [`Task`]s, with the
//! current [`Mode`] deciding how input is interpreted and what is rendered.
mod action;
pub mod model;
mod service;
mod subscription;
pub mod task;
mod view;

use anyhow::Context;
use iced::{
    Color, Task,
    theme::palette::Seed,
    window::{Settings, settings::PlatformSpecific},
};
use joy_error::ResultUtilityExt;

use crate::{
    app::{
        service::{
            overview::{self},
            tiler::{self},
        },
        subscription::global::GlobalMessage,
    },
    assert_log_fail,
    scroll_tiler::ScrollTiler,
    system,
    utils::math::Size,
    window::{self},
};

/// The whole application state. A single instance lives for the program's
/// lifetime and is mutated in place by [`State::handle_app_message`].
pub struct State {
    pub tiler: ScrollTiler,
    /// What winri is currently doing; gates input handling and rendering.
    pub mode: Mode,
    pub configuration: model::Configuration,
    /// The always-on-top, click-through window we draw the overlay onto.
    overlay_window_id: iced::window::Id,
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
    /// A resolved, mode-aware user action (keybinding already mapped).
    Action(action::Action),

    Overview(overview::Message),

    Global(subscription::global::GlobalMessage),

    /// Restore managed windows and quit. Sent after [`Mode::Exit`] is set.
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
        let screen_size = system::screen_size().expect("Screen size retrieval");
        let work_area = system::work_area().expect("Work area retrieval");
        let tiler = ScrollTiler::new(10.0, 20.0, work_area);
        let (overlay_window_id, overlay_window_creation_task) = create_overlay_window(screen_size);
        (
            Self {
                tiler,
                mode: Mode::default(),
                configuration: model::Configuration {
                    tiler_border_style: model::BorderStyle {
                        color: system::highlight_color().unwrap(),
                        thickness: 4.0,
                        radius: 8.0,
                    },
                },
                overlay_window_id,
            },
            overlay_window_creation_task,
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
            Message::Global(global_message) => {
                task = task.chain(self.handle_global_message(global_message));
            }
            Message::CleanupAndExit => {
                system::restore_windows();
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
    pub fn subscription(_: &Self) -> iced::Subscription<Message> {
        iced::Subscription::run(subscription::global::subscription)
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
