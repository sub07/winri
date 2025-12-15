use std::{collections::HashSet, ops::Sub};

use anyhow::Context;
use joy_error::log::ResultLogExt;
use log::{debug, info, warn};

use crate::{
    cast,
    utils::{Size, cast::FaillibleCastUtils},
    window::Window,
};

#[derive(PartialEq, Eq)]
pub struct WindowItem {
    pub inner: Window,
    pub width: u32,
}

impl WindowItem {
    pub const fn new(inner: Window, width: u32) -> Self {
        Self { inner, width }
    }
}

#[derive(Default)]
pub struct ScrollTiler {
    windows: Vec<WindowItem>,
    padding: u32,
    resize_increment: u32,
    scroll_offset: i32,
    screen_size: Size,
    previously_focused_window_index: Option<usize>,
}

impl ScrollTiler {
    pub fn new(padding: u32, resize_increment: u32, screen_size: Size) -> Self {
        Self {
            padding,
            resize_increment,
            screen_size,
            ..Default::default()
        }
    }

    fn focus_index(&self) -> Option<usize> {
        self.windows
            .iter()
            .position(|item| item.inner.is_focused().unwrap_or(false))
    }

    fn logged_focus_index(&self) -> Option<usize> {
        self.focus_index()
            .context("Focused window is not tiled")
            .info()
            .log_err()
            .ok()
    }

    fn focus_index_with_fallback_and_log(&self) -> Option<usize> {
        self.focus_index()
            .context("Focused window is not tiled, using previously focused window index")
            .info()
            .log_err()
            .ok()
            .or(self.previously_focused_window_index)
            .context("No previously focused window index available")
            .info()
            .log_err()
            .ok()
            .or_else(|| {
                if self.windows.is_empty() {
                    None
                } else {
                    info!("Defaulting to first window");
                    Some(0)
                }
            })
    }

    pub fn windows(&self) -> impl Iterator<Item = &WindowItem> {
        self.windows.iter()
    }

    pub fn swap_current_left(&mut self) {
        self.swap_current(-1);
    }

    pub fn swap_current_right(&mut self) {
        self.swap_current(1);
    }

    fn swap_current(&mut self, direction: i32) {
        if let Some(focus_index) = self.logged_focus_index() {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_possible_wrap,
                reason = "to add a potential negative number to a usize"
            )]
            let other_swap_index =
                (focus_index as i32 + direction).clamp(0, self.windows.len() as i32 - 1) as usize;
            self.windows.swap(focus_index, other_swap_index);
        }
    }

    pub fn focus_left(&self) {
        self.focus(-1);
    }

    pub fn focus_right(&self) {
        self.focus(1);
    }

    fn focus(&self, direction: i32) {
        if let Some(focus_index) = self.focus_index_with_fallback_and_log() {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_possible_wrap,
                reason = "to add a potential negative number to a usize"
            )]
            let new_focus_index =
                (focus_index as i32 + direction).clamp(0, self.windows.len() as i32 - 1) as usize;
            let window = self.windows[new_focus_index].inner;

            let _ = window
                .focus()
                .context(window.get_formatted_extensive_info())
                .context("Changing tiler focused window")
                .error()
                .log_err();
        }
    }

    pub fn set_current_window_fullscreen(&mut self) {
        if let Some(focus_index) = self.focus_index() {
            let screen_width = self.screen_size.width();
            // -1 to avoid occupying the whole screen and causing scroll issues
            // TODO: find a better solution for this, the problem is that the scroll system
            // doesn't handle windows that are equals or bigger than the screen size well.
            // Fix for now: prevent windows width from being equal or bigger than screen size.
            self.windows[focus_index].width = screen_width - self.padding * 2 - 1;
        }
    }

    pub fn set_current_window_halfscreen(&mut self) {
        if let Some(focus_index) = self.focus_index() {
            let screen_width = self.screen_size.width();
            self.windows[focus_index].width = (screen_width / 2) - self.padding * 2;
        }
    }

    pub fn increment_current_window_width(&mut self) {
        self.resize_current_window_width_by_resize_increment(1);
    }

    pub fn decrement_current_window_width(&mut self) {
        self.resize_current_window_width_by_resize_increment(-1);
    }

    /// Resize the current window width by the resize increment in the given direction.
    /// Direction should be 1 for increasing width and -1 for decreasing width.
    fn resize_current_window_width_by_resize_increment(&mut self, direction: i32) {
        if let Some(focus_index) = self.focus_index() {
            cast! {
                self.windows[focus_index].width => i32 as current_width,
                self.resize_increment => i32 as resize_increment,
                self.screen_size.width() => i32 as screen_width,
                self.padding => i32 as padding,
            }
            // TODO: check explanation in `set_current_window_fullscreen` about -1
            let new_width = (current_width + resize_increment * direction.signum())
                .max(0)
                .min(screen_width - padding * 2 - 1);
            self.windows[focus_index].width = new_width.cast();
        }
    }

    pub fn current_window(&self) -> Option<Window> {
        self.focus_index().map(|index| self.windows[index].inner)
    }

    pub fn handle_window_snapshot(&mut self, windows_snapshot: &HashSet<Window>) {
        if windows_snapshot.is_empty() {
            self.windows.clear();
            return;
        }

        self.windows
            .retain(|item| windows_snapshot.contains(&item.inner));

        self.append_new_windows(windows_snapshot);

        let windows_positions = self.windows_positions();

        let previous_scroll_offset = self.scroll_offset;
        self.ajust_scroll(&windows_positions);
        if previous_scroll_offset != self.scroll_offset {
            debug!(
                "Adjusted scroll offset from {} to {}",
                previous_scroll_offset, self.scroll_offset
            );
        }
        self.layout_windows(&windows_positions);
        self.reconciliate_window_widths();

        if let Some(new_focused_window_index) = self
            .focus_index()
            .filter(|i| Some(*i) != self.previously_focused_window_index)
        {
            self.previously_focused_window_index = Some(new_focused_window_index);
        }
    }

    /// Append new windows from the snapshot that are not already in the tiler.
    /// If the focused window is tiled, new windows are appended after it.
    /// Otherwise, they are appended at the end.
    fn append_new_windows(&mut self, windows_snapshot: &HashSet<Window>) {
        if !self.windows.is_empty()
            && let Some(focus_index) = self.focus_index().or(self.previously_focused_window_index)
            && focus_index < self.windows.len()
        {
            for window in windows_snapshot {
                if !self
                    .windows
                    .iter()
                    .any(|window_item| window_item.inner == *window)
                {
                    log::info!("Adding after focused {focus_index}");
                    self.windows.insert(
                        focus_index + 1,
                        WindowItem::new(*window, self.default_size()),
                    );
                }
            }
        } else {
            for window in windows_snapshot {
                if !self
                    .windows
                    .iter()
                    .any(|window_item| window_item.inner == *window)
                {
                    log::info!("Appending at end");
                    self.windows
                        .push(WindowItem::new(*window, self.default_size()));
                }
            }
        }
    }

    fn default_size(&self) -> u32 {
        self.screen_size.width() / 2 - self.padding * 2
    }

    fn layout_windows(&mut self, windows_positions: &[i32]) {
        for (window, x) in self.windows.iter_mut().zip(windows_positions) {
            let y = self.padding.cast();
            let height = self.screen_size.height() - self.padding * 2;
            if let Err(e) = window.inner.move_to(
                [x - self.scroll_offset, y].into(),
                [window.width, height].into(),
            ) {
                warn!(
                    "Error while layouting window, skipping to next one (window might have been closed just after enumeration): {e}"
                );
            }
        }
    }

    fn reconciliate_window_widths(&mut self) {
        for window in &mut self.windows {
            let window_rect = match window.inner.desktop_manager_bounds() {
                Ok(rect) => rect,
                Err(e) => {
                    warn!(
                        "Error while reconciliating window, skipping to next one (window might have been closed just after enumeration): {e}"
                    );
                    continue;
                }
            };

            let actual_size = window_rect.right - window_rect.left;

            cast! {
                actual_size => u32,
            }

            let expected_size = window.width;
            if expected_size < actual_size {
                window.width = actual_size + self.padding * 2;
            }
        }
    }

    fn ajust_scroll(&mut self, windows_positions: &[i32]) {
        if let Some((index, focused_window)) = self
            .windows
            .iter()
            .enumerate()
            .find(|(_, window_item)| window_item.inner.is_focused().unwrap_or(false))
        {
            cast! {
                self.padding => i32 as padding,
                focused_window.width => i32 as focused_window_width,
                self.screen_size.width() => i32 as screen_width,
            }

            let focused_window_left = windows_positions[index] - padding - self.scroll_offset;
            let focused_window_right = focused_window_left + focused_window_width + padding * 2;

            if focused_window_left >= 0 && focused_window_right <= screen_width {
                return;
            }

            let window_left_to_screen_left = focused_window_left.abs();
            let window_right_to_screen_right = focused_window_right.sub(screen_width).abs();

            if window_left_to_screen_left < window_right_to_screen_right {
                self.scroll_offset -= window_left_to_screen_left;
            } else {
                self.scroll_offset += window_right_to_screen_right;
            }
        }
    }

    pub fn windows_positions(&self) -> Vec<i32> {
        let mut positions = Vec::new();
        let mut current_position = 0;

        cast! {
            self.padding => i32 as padding,
        }

        for window in &self.windows {
            cast! {
                window.width => i32 as window_width,
            }

            current_position += padding;
            positions.push(current_position);
            current_position += window_width + padding;
        }

        positions
    }

    pub const fn screen_size(&self) -> Size {
        self.screen_size
    }
}
