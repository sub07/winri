use keyboard_types::Modifiers;
use rdev::Key;

use crate::{
    action::{Action, OverviewAction, TilerAction},
    app::{self, Mode},
};

pub mod overview;
pub mod thumbnail;
pub mod tiler;

impl app::State {
    pub fn handle_global_key_event(
        &mut self,
        modifiers: Modifiers,
        key: Key,
    ) -> anyhow::Result<()> {
        if let Some(action) = self.resolve_action(modifiers, key) {
            self.handle_action(action)?;
        }
        Ok(())
    }

    fn resolve_action(&self, modifiers: Modifiers, key: Key) -> Option<Action> {
        match (&self.mode, modifiers, key) {
            (Mode::Tiler { .. }, Modifiers::META, Key::LeftArrow) => {
                Some(Action::Tiler(TilerAction::MoveFocusPrevious))
            }
            (Mode::Tiler { .. }, Modifiers::META, Key::RightArrow) => {
                Some(Action::Tiler(TilerAction::MoveFocusNext))
            }
            (Mode::Tiler { .. }, _, Key::LeftArrow)
                if modifiers == Modifiers::META.union(Modifiers::CONTROL) =>
            {
                Some(Action::Tiler(TilerAction::SwapWithPrevious))
            }
            (Mode::Tiler { .. }, _, Key::RightArrow)
                if modifiers == Modifiers::META.union(Modifiers::CONTROL) =>
            {
                Some(Action::Tiler(TilerAction::SwapWithNext))
            }
            (Mode::Tiler { .. }, Modifiers::META, Key::KeyQ) => {
                Some(Action::Tiler(TilerAction::CloseCurrent))
            }
            (Mode::Tiler { .. }, Modifiers::META, Key::KeyF) => {
                Some(Action::Tiler(TilerAction::ResizeToFullscreen))
            }
            (Mode::Tiler { .. }, Modifiers::META, Key::KeyC) => {
                Some(Action::Tiler(TilerAction::ResizeToHalfScreen))
            }
            (Mode::Tiler { .. }, _, Key::LeftArrow)
                if modifiers == Modifiers::META.union(Modifiers::SHIFT) =>
            {
                Some(Action::Tiler(TilerAction::DecrementWidth))
            }
            (Mode::Tiler { .. }, _, Key::RightArrow)
                if modifiers == Modifiers::META.union(Modifiers::SHIFT) =>
            {
                Some(Action::Tiler(TilerAction::IncrementWidth))
            }
            (Mode::Tiler { .. }, Modifiers::META, Key::UpArrow) => {
                Some(Action::Tiler(TilerAction::OpenOverview))
            }
            (Mode::Overview { .. }, Modifiers::META, Key::DownArrow)
            | (Mode::Overview { .. }, Modifiers::META, Key::Escape) => {
                Some(Action::Overview(OverviewAction::CloseOverview))
            }
            (_, Modifiers::META, Key::Escape) => Some(Action::Exit),
            _ => None,
        }
    }

    fn handle_action(&mut self, action: Action) -> anyhow::Result<()> {
        log::info!("Executing action: {action:?}");
        match action {
            Action::Tiler(tiler_action) => match tiler_action {
                TilerAction::CloseCurrent => {
                    if let Some(window) = self.tiler.focused_window() {
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
                    self.update_tiler()?;
                }
                TilerAction::ResizeToHalfScreen => {
                    self.tiler.set_current_window_halfscreen();
                    self.update_tiler()?;
                }
                TilerAction::OpenOverview => {}
                TilerAction::IncrementWidth => {
                    self.tiler.increment_current_window_width();
                    self.update_tiler()?;
                }
                TilerAction::DecrementWidth => {
                    self.tiler.decrement_current_window_width();
                    self.update_tiler()?;
                }
            },
            Action::Overview(_) => {}
            Action::Exit => self.mode = Mode::Exit,
        }

        Ok(())
    }
}
