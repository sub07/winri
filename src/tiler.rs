use std::{collections::HashSet, ops::Sub};

use log::warn;

use crate::{screen::screen_size, window::Window};

#[derive(PartialEq, Eq)]
pub struct WindowItem {
    inner: Window,
    width: i32,
}

impl WindowItem {
    pub const fn new(inner: Window, width: i32) -> Self {
        Self { inner, width }
    }
}

#[derive(Default)]
pub struct ScrollTiler {
    windows: Vec<WindowItem>,
    padding: i32,
    scroll_offset: i32,
}

impl ScrollTiler {
    pub fn with_padding(padding: i32) -> Self {
        Self {
            padding,
            ..Default::default()
        }
    }

    pub fn handle_window_snapshot(&mut self, windows_snapshot: &HashSet<Window>) {
        if windows_snapshot.is_empty() {
            self.windows.clear();
            return;
        }

        let len_before_deletion = self.windows.len();

        self.windows
            .retain(|item| windows_snapshot.contains(&item.inner));

        // Early return optimization
        if windows_snapshot.len() == self.windows.len() && len_before_deletion == self.windows.len()
        {
            let windows_positions = self.windows_positions();

            if self.ajust_scroll(&windows_positions) {
                self.layout_windows(&windows_positions);
            }
            return;
        }
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
                self.windows.push(WindowItem::new(
                    *window,
                    (screen_size().0 as f32 / 1.5).round() as i32,
                ));
            }
        }
    }

    fn layout_windows(&self, windows_positions: &[i32]) {
        for (window, x) in self.windows.iter().zip(windows_positions) {
            let y = self.padding;
            let height = screen_size().1 - self.padding * 2;
            if let Err(err) =
                window
                    .inner
                    .move_window(x - self.scroll_offset, y, window.width, height)
            {
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
            let screen_width = screen_size().0;

            let focused_window_left = windows_positions[index] - self.padding - self.scroll_offset;
            let focused_window_right =
                focused_window_left + focused_window.width + self.padding * 2;

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

        for window in &self.windows {
            current_position += self.padding;
            positions.push(current_position);
            current_position += window.width + self.padding;
        }

        positions
    }
}
