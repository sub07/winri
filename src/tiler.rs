use std::{collections::HashSet, ops::Sub};

use log::{error, warn};

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
    scroll_offset: i32,
    screen_size: Size,
}

impl ScrollTiler {
    pub fn new(padding: u32, screen_size: Size) -> Self {
        Self {
            padding,
            screen_size,
            ..Default::default()
        }
    }

    fn focus_index(&self) -> Option<usize> {
        self.windows
            .iter()
            .position(|item| item.inner.is_focused().unwrap_or(false))
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
        if let Some(focus_index) = self.focus_index() {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_possible_wrap,
                reason = "to add a potential negative number to a usize"
            )]
            let other_swap_index =
                (focus_index as i32 + direction).clamp(0, self.windows.len() as i32 - 1) as usize;
            self.windows.swap(focus_index, other_swap_index);
        } else {
            warn!(
                "Could not find focused window in tiler. Focused window is {:?}",
                Window::focused()
            );
        }
    }

    pub fn focus_left(&self) {
        self.focus(-1);
    }

    pub fn focus_right(&self) {
        self.focus(1);
    }

    fn focus(&self, direction: i32) {
        if let Some(focus_index) = self.focus_index() {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_possible_wrap,
                reason = "to add a potential negative number to a usize"
            )]
            let new_focus_index =
                (focus_index as i32 + direction).clamp(0, self.windows.len() as i32 - 1) as usize;
            let window = self.windows[new_focus_index].inner;
            if let Err(err) = window.focus() {
                error!(
                    "Failed to focus window ({}): {}",
                    err,
                    window.get_formatted_extensive_info(),
                );
            }
        } else {
            warn!(
                "Could not find focused window in tiler. Focused window is {:?}",
                Window::focused()
            );
        }
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

        self.ajust_scroll(&windows_positions);
        self.layout_windows(&windows_positions);
    }

    fn append_new_windows(&mut self, windows_snapshot: &HashSet<Window>) {
        for window in windows_snapshot {
            if !self
                .windows
                .iter()
                .any(|window_item| window_item.inner == *window)
            {
                self.windows
                    .push(WindowItem::new(*window, self.screen_size.width() * 7 / 8));
            }
        }
    }

    fn layout_windows(&self, windows_positions: &[i32]) {
        for (window, x) in self.windows.iter().zip(windows_positions) {
            let y = self.padding.cast();
            let height = self.screen_size.height() - self.padding * 2;
            if let Err(err) = window.inner.move_to(
                [x - self.scroll_offset, y].into(),
                [window.width, height].into(),
            ) {
                warn!("Failed to move window {:?}: {err}", window.inner);
            }
        }
    }

    fn ajust_scroll(&mut self, windows_positions: &[i32]) -> bool {
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
                return false;
            }

            let window_left_to_screen_left = focused_window_left.abs();
            let window_right_to_screen_right = focused_window_right.sub(screen_width).abs();

            if window_left_to_screen_left < window_right_to_screen_right {
                self.scroll_offset -= window_left_to_screen_left;
                window_left_to_screen_left != 0
            } else {
                self.scroll_offset += window_right_to_screen_right;
                window_right_to_screen_right != 0
            }
        } else {
            false
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
