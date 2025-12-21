use std::collections::HashSet;

use anyhow::Context;

use crate::{
    app::{self, Mode},
    utils::math::Bounds,
    window::{Window, filter::opened_windows},
};

#[derive(Default)]
pub struct State {
    pub current_border_bounds: Option<Bounds>,
}

macro_rules! bind_tiler_mode_result {
    ($mode:expr => TilerState { $($bindings:tt),+ }) => {
        let Mode::Tiler(State { $($bindings),+ ,.. }) = &mut $mode else {
            log::warn!(
                "Tiler operation requested in {} while not in Tiler mode",
                crate::function!()
            );
            return Ok(());
        };
    };
}

macro_rules! ensure_tiler_mode_result {
    ($mode:expr) => {
        match &$mode {
            Mode::Tiler(_) => {}
            _ => {
                log::warn!(
                    "Tiler operation requested in {} while not in Tiler mode",
                    crate::function!()
                );
                return Ok(());
            }
        }
    };
}

fn get_process_names(windows: &HashSet<Window>) -> Vec<String> {
    windows
        .iter()
        .map(|w| {
            let is_focused = w.is_focused().unwrap_or(false);
            format!(
                "{}{}[class: {}][hwnd: {:?}][title: {}]",
                if is_focused { "[FOCUSED] " } else { "" },
                w.process_name()
                    .ok()
                    .unwrap_or_else(|| "[ERROR] Could not get process name".to_string()),
                w.class()
                    .unwrap_or_else(|_| "[ERROR] Could not get class name".to_string()),
                w.handle(),
                w.title()
                    .unwrap_or_else(|_| Some("[ERROR] Could not get window title".to_string()))
                    .unwrap_or_else(|| "[UNNAMED]".to_string()),
            )
        })
        .collect::<Vec<_>>()
}

impl app::State {
    pub fn update_tiler(&mut self) -> anyhow::Result<()> {
        ensure_tiler_mode_result!(self.mode);

        let windows_snapshot = opened_windows().context("Window enumeration for tiler update")?;

        log::info!("Opened windows: {:?}", get_process_names(&windows_snapshot));

        self.tiler.handle_window_snapshot(&windows_snapshot);

        self.update_tiler_border()?;

        Ok(())
    }

    pub fn update_tiler_border(&mut self) -> anyhow::Result<()> {
        bind_tiler_mode_result!(self.mode => TilerState { current_border_bounds });
        if let Some(focused_window) = self.tiler.focused_window() {
            let bounds = focused_window
                .desktop_manager_bounds()
                .context("Desktop manager bounds querying for tiler border update")?;
            if current_border_bounds != &Some(bounds) {
                *current_border_bounds = Some(bounds);
            }
        } else {
            *current_border_bounds = None;
        }

        Ok(())
    }

    pub fn switch_to_tiler_mode(&mut self) -> anyhow::Result<()> {
        if !matches!(self.mode, Mode::Tiler(_)) {
            log::info!("switching to Tiler mode");
            self.mode = Mode::Tiler(State::default());
            self.update_tiler()
                .context("initial tiler update on mode switch")?;
        }
        Ok(())
    }
}
