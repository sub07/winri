mod action;
pub mod model;
mod service;
mod subscription;
mod view;

use anyhow::Context;
use iced::{Color, Task, theme::Palette, window::Settings};

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

pub struct State {
    pub tiler: ScrollTiler,
    pub mode: Mode,
    pub configuration: model::Configuration,
    overlay_window_id: iced::window::Id,
}

pub enum Mode {
    Tiler(tiler::State),
    Overview(overview::State),
    Exit,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Tiler(tiler::State::default())
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Action(action::Action),

    Overview(overview::Message),

    Global(subscription::global::GlobalMessage),

    CleanupAndExit,
}

fn create_overlay_window(screen_size: Size) -> (iced::window::Id, Task<Message>) {
    let (id, task) = iced::window::open(Settings {
        decorations: false,
        transparent: true,
        resizable: false,
        closeable: false,
        level: iced::window::Level::AlwaysOnTop,
        position: iced::window::Position::Specific(iced::Point::ORIGIN),
        size: screen_size.into(),
        ..Default::default()
    });

    (id, task.then(iced::window::enable_mouse_passthrough))
}

impl State {
    pub fn new() -> (Self, Task<Message>) {
        let screen_size = system::screen_size().expect("Screen size retrieval");
        let tiler = ScrollTiler::new(10.0, 20.0, screen_size);
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

    pub fn title(_: &Self, _window_id: iced::window::Id) -> String {
        window::filter::WINRI_IGNORED_WINDOW_TITLE_SUBSTRING.into()
    }

    pub fn handle_app_message(&mut self, message: Message) -> Task<Message> {
        let mut task = Task::none();
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

    pub fn view(&self, window_id: iced::window::Id) -> iced::Element<'_, Message> {
        if window_id == self.overlay_window_id {
            view::overlay::view(self)
        } else {
            view::empty()
        }
    }

    pub fn theme(&self, window_id: iced::window::Id) -> iced::Theme {
        if window_id == self.overlay_window_id {
            iced::Theme::custom(
                "Overlay transparent theme",
                Palette {
                    background: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
                    ..Palette::DARK
                },
            )
        } else {
            iced::Theme::Dark // TODO: Adapt to system theme
        }
    }

    pub fn subscription(_: &Self) -> iced::Subscription<Message> {
        iced::Subscription::run(subscription::global::subscription)
    }
}

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

    fn discard(self) {}
}
