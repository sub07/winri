mod manager;
mod subscription;

use std::collections::HashMap;

use anyhow::Context;
use iced::{Color, Task, theme::Palette, window::Settings};

use crate::{
    app::{
        manager::{overview::OverviewState, tiler::TilerState},
        subscription::global::GlobalMessage,
    },
    assert_log_fail,
    scroll_tiler::ScrollTiler,
    system,
    utils::math::{Position, Size},
    window::{self},
};

pub struct State {
    pub tiler: ScrollTiler,
    pub mode: Mode,
    overlay_window_id: iced::window::Id,
}

pub enum Mode {
    Tiler(TilerState),
    Overview(OverviewState),
    Exit,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Tiler(TilerState::default())
    }
}

#[derive(Debug, Clone)]
pub enum Message {
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
        }
        if matches!(self.mode, Mode::Exit) {
            task = task.chain(Task::done(Message::CleanupAndExit));
        }
        task
    }

    pub fn handle_global_message(&mut self, message: GlobalMessage) -> Task<Message> {
        match message {
            GlobalMessage::Key(modifiers, key) => handle_faillible_process(
                self.handle_global_key_event(modifiers, key)
                    .context("global key event handling"),
            ),
            GlobalMessage::Window => {
                handle_faillible_process(self.update_tiler().context("global window event"));
            }
        }
        Task::none()
    }

    pub fn view(&self, _window_id: iced::window::Id) -> iced::Element<'_, Message> {
        use iced::widget::Text;

        Text::new("Hello, Winri!").into()
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

fn handle_faillible_process<E: std::fmt::Debug>(result: Result<(), E>) {
    match result {
        Ok(()) => {}
        Err(e) => {
            assert_log_fail!("{:?}", e);
        }
    }
}
