//! The core layout engine.
//!
//! Windows are packed left to right at their individual requested widths into a
//! strip that may be wider than the screen. A horizontal scroll offset defines
//! which slice of that strip is visible, and is auto-adjusted to keep the
//! focused window on screen. This is what makes winri a *scrolling* tiler
//! rather than a fixed-grid one. The engine owns only the model; applying it to
//! real windows happens in [`ScrollTiler::handle_window_snapshot`].

use std::{collections::HashSet, ops::Sub};

use anyhow::Context;
use joy_error::log::ResultLogExt;
use log::{debug, info, warn};

use crate::{Config, cast, utils::math::Bounds, window::Window};

#[derive(PartialEq)]
pub struct WindowItem {
    pub inner: Window,
    /// The width that has been requested for the window.
    /// Should be handled and cleared during the next `handle_window_snapshot` call.
    /// If `None` during that call, the current window width will be used.
    pub requested_width: Option<f32>,
    /// Keep track of the current width of the window.
    /// Should be updated to always reflect the actual window width.
    pub width: f32,
}

impl WindowItem {
    /// Wraps a window, requesting `width` on the next layout pass.
    pub const fn new(inner: Window, width: f32) -> Self {
        Self {
            inner,
            requested_width: Some(width),
            width,
        }
    }

    fn request_width(&mut self, width: f32) {
        self.requested_width = Some(width);
    }

    fn requested_width(&mut self) -> Option<f32> {
        self.requested_width.take()
    }
}

#[derive(Default)]
pub struct ScrollTiler {
    windows: Vec<WindowItem>,
    config: Config,
    scroll_offset: f32,
    /// The work area of the screen the tiler is applied to: the screen
    /// rectangle minus the taskbar and any other docked `AppBars`. Tiling stays
    /// inside this rectangle so windows do not overlap the taskbar.
    work_area: Bounds,
    /// The index of the previously focused window. Used as a fallback when the focused window is not tiled.
    previously_focused_window_index: Option<usize>,
}

impl ScrollTiler {
    pub fn new(config: Config, work_area: Bounds) -> Self {
        Self {
            config,
            work_area,
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

    #[allow(
        clippy::cast_sign_loss,
        reason = "return value is guaranteed to be positive by the clamp call"
    )]
    fn compute_index_for_direction(&self, focus_index: usize, direction: i32) -> usize {
        cast! {
            focus_index => i32,
            self.windows.len() => i32 as windows_len,
        }
        (focus_index + direction).clamp(0, windows_len - 1) as usize
    }

    fn swap_current(&mut self, direction: i32) {
        if let Some(focus_index) = self.logged_focus_index() {
            let other_swap_index = self.compute_index_for_direction(focus_index, direction);
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
            let new_focus_index = self.compute_index_for_direction(focus_index, direction);
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
            let width = self.max_screen_width();
            // -1 to avoid occupying the whole screen and causing scroll issues
            // TODO: find a better solution for this, the problem is that the scroll system
            // doesn't handle windows that are equals or bigger than the screen size well.
            // Fix for now: prevent windows width from being equal or bigger than screen size.
            self.windows[focus_index].request_width(width);
        }
    }

    pub fn set_current_window_halfscreen(&mut self) {
        if let Some(focus_index) = self.focus_index() {
            let work_width = self.work_area.size().width();
            self.windows[focus_index].request_width(
                self.config
                    .load()
                    .default_window
                    .padding
                    .mul_add(-2.0, work_width / 2.0),
            );
        }
    }

    pub fn increment_current_window_width(&mut self) {
        self.resize_current_window_width_by_resize_increment(1);
    }

    pub fn decrement_current_window_width(&mut self) {
        self.resize_current_window_width_by_resize_increment(-1);
    }

    /// Largest width a window may take: the work area minus padding, less 1px.
    /// The -1 keeps windows strictly narrower than the screen, which the scroll
    /// math relies on (see [`Self::set_current_window_fullscreen`]).
    pub fn max_screen_width(&self) -> f32 {
        self.config
            .load()
            .default_window
            .padding
            .mul_add(-2.0, self.work_area.size().width())
            - 1.0
    }

    /// Resize the current window width by the resize increment in the given direction.
    /// Direction should be 1 for increasing width and -1 for decreasing width.
    fn resize_current_window_width_by_resize_increment(&mut self, direction: i32) {
        if let Some(focus_index) = self.focus_index() {
            // TODO: check explanation in `set_current_window_fullscreen` about -1
            cast! {
                direction.signum() => f32 as direction,
            }
            let new_width = self
                .config
                .load()
                .default_window
                .resize_increment
                .mul_add(direction, self.windows[focus_index].width)
                .clamp(
                    0.0,
                    self.config
                        .load()
                        .default_window
                        .padding
                        .mul_add(-2.0, self.work_area.size().width())
                        - 1.0,
                );
            self.windows[focus_index].request_width(new_width);
        }
    }

    pub fn focused_window(&self) -> Option<Window> {
        self.focus_index().map(|index| self.windows[index].inner)
    }

    /// Reconciles the model against the live set of tileable windows and lays
    /// them out on screen.
    ///
    /// This is the heart of the engine: it drops closed windows, inserts new
    /// ones (next to the focused window so they appear where attention is),
    /// refreshes widths, scrolls so the focused window stays visible, and moves
    /// every window to its computed position.
    pub fn handle_window_snapshot(&mut self, windows_snapshot: &HashSet<Window>) {
        if windows_snapshot.is_empty() {
            self.windows.clear();
            return;
        }

        self.windows
            .retain(|item| windows_snapshot.contains(&item.inner));

        self.update_widths();

        self.append_new_windows(windows_snapshot);

        let windows_positions = self.windows_positions();

        let previous_scroll_offset = self.scroll_offset;
        self.ajust_scroll(&windows_positions);
        if (previous_scroll_offset - self.scroll_offset).abs() > 1.0 {
            debug!(
                "Adjusted scroll offset from {} to {}",
                previous_scroll_offset, self.scroll_offset
            );
        }
        self.layout_windows(&windows_positions);

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
                    self.windows
                        .push(WindowItem::new(*window, self.default_size()));
                }
            }
        }
    }

    fn default_size(&self) -> f32 {
        self.config
            .load()
            .default_window
            .padding
            .mul_add(-2.0, self.work_area.size().width() / 2.0)
    }

    fn layout_windows(&mut self, windows_positions: &[f32]) {
        let origin = self.work_area.position();
        let config = self.config.load();
        let height = config
            .default_window
            .padding
            .mul_add(-2.0, self.work_area.size().height());
        for (window, x) in self.windows.iter_mut().zip(windows_positions) {
            let y = origin.y() + config.default_window.padding;
            if let Err(e) = window.inner.move_to(
                [origin.x() + x - self.scroll_offset, y].into(),
                [window.width, height].into(),
            ) {
                warn!(
                    "Error while layouting window, skipping to next one (window might have been closed just after enumeration): {e}"
                );
            }
        }
    }

    fn update_widths(&mut self) {
        let max_screen_width = self.max_screen_width();
        for window in &mut self.windows {
            if let Some(requested_width) = window.requested_width() {
                window.width = requested_width;
            } else if let Ok(bounds) = window
                .inner
                .desktop_manager_bounds()
                .context("Updating widths")
                .error()
                .log_err()
            {
                window.width = bounds.size().width().min(max_screen_width);
            }
        }
    }

    fn ajust_scroll(&mut self, windows_positions: &[f32]) {
        if let Some((index, focused_window)) = self
            .windows
            .iter()
            .enumerate()
            .find(|(_, window_item)| window_item.inner.is_focused().unwrap_or(false))
        {
            let config = self.config.load();
            let focused_window_left =
                windows_positions[index] - config.default_window.padding - self.scroll_offset;
            let focused_window_right = config
                .default_window
                .padding
                .mul_add(2.0, focused_window_left + focused_window.width);

            let work_width = self.work_area.size().width();
            if focused_window_left >= 0.0 && focused_window_right <= work_width {
                return;
            }

            let window_left_to_screen_left = focused_window_left.abs();
            let window_right_to_screen_right = focused_window_right.sub(work_width).abs();

            if window_left_to_screen_left < window_right_to_screen_right {
                self.scroll_offset -= window_left_to_screen_left;
            } else {
                self.scroll_offset += window_right_to_screen_right;
            }
        }
    }

    /// The unscrolled left x of each window in the strip, in tiling order.
    /// Subtracting the scroll offset gives the on-screen position.
    pub fn windows_positions(&self) -> Vec<f32> {
        let config = self.config.load();
        let mut positions = Vec::new();
        let mut current_position = 0.0;

        for window in &self.windows {
            current_position += config.default_window.padding;
            positions.push(current_position);
            current_position += window.width + config.default_window.padding;
        }

        positions
    }

    pub const fn work_area(&self) -> Bounds {
        self.work_area
    }
}
