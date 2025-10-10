use std::{collections::HashSet, hash::Hash, iter};

use bimap::BiMap;

use crate::{screen::screen_size, window::Window};

#[derive(PartialEq, Eq)]
pub struct WindowItem {
    inner: Window,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl From<Window> for WindowItem {
    fn from(value: Window) -> Self {
        Self {
            inner: value,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
}

impl Hash for WindowItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

pub struct HorizontalTiler {
    windows: BiMap<WindowItem, usize>,
}

impl HorizontalTiler {
    pub fn new() -> Self {
        Self {
            windows: BiMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.windows.clear();
    }

    pub fn windows(&self) -> impl Iterator<Item = &WindowItem> {
        self.windows.left_values()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn remove_window(&mut self, window: Window) {
        self.windows.remove_by_left(&window.into());
    }

    pub fn greatest_used_index(&self) -> Option<usize> {
        self.windows.right_values().max().copied()
    }

    pub fn has_window_at_index(&self, index: usize) -> bool {
        self.windows.contains_right(&index)
    }

    pub fn remove_window_at_index(&mut self, index: usize) -> Option<WindowItem> {
        self.windows.remove_by_right(&index).map(|(w, _)| w)
    }

    pub fn put_window_at_index(&mut self, index: usize, window: WindowItem) {
        self.windows.insert(window, index);
    }

    pub fn has_window(&self, window: Window) -> bool {
        self.windows.contains_left(&window.into())
    }

    pub fn get_window_at_index(&self, index: usize) -> Option<&WindowItem> {
        self.windows.get_by_right(&index)
    }
}

pub struct ScrollTiler {
    inner: HorizontalTiler,
}

impl ScrollTiler {
    pub fn new() -> Self {
        Self {
            inner: HorizontalTiler::new(),
        }
    }

    pub fn handle_window_snapshot(&mut self, windows_snapshot: &HashSet<Window>) {
        if windows_snapshot.is_empty() {
            self.inner.clear();
            return;
        }

        let managed_windows_deleted = self.remove_unmanaged_windows(windows_snapshot);

        // if we have the same number of windows as the snapshot after deletion, we don't need to do anything
        if windows_snapshot.len() == self.inner.window_count() && managed_windows_deleted.is_empty()
        {
            return;
        }

        let next_available_index = self.pack();
        self.append_new_windows(windows_snapshot, next_available_index);
    }

    fn remove_unmanaged_windows(&mut self, windows_snapshot: &HashSet<Window>) -> Vec<Window> {
        let to_delete_window = self
            .inner
            .windows()
            .map(|item| item.inner)
            .filter(|window| !windows_snapshot.contains(window))
            .collect::<Vec<_>>();

        for managed_window in &to_delete_window {
            self.inner.remove_window(*managed_window);
        }

        to_delete_window
    }

    fn pack(&mut self) -> usize {
        let Some(greatest_used_index) = self.inner.greatest_used_index() else {
            return 0;
        };

        // Lowest empty slot index during packing
        let mut empty_index = 0;

        for index in 0..=greatest_used_index {
            if index == empty_index {
                if self.inner.has_window_at_index(index) {
                    empty_index += 1;
                }
            } else if let Some(window) = self.inner.remove_window_at_index(index) {
                self.inner.put_window_at_index(empty_index, window);
                empty_index += 1;
            }
        }

        empty_index
    }

    fn append_new_windows(
        &mut self,
        windows_snapshot: &HashSet<Window>,
        mut next_available_index: usize,
    ) {
        for window in windows_snapshot {
            if !self.inner.has_window(*window) {
                self.inner
                    .put_window_at_index(next_available_index, (*window).into());
                next_available_index += 1;
            }
        }
    }

    pub fn layout_windows(&self) -> anyhow::Result<()> {
        let (screen_width, screen_height) = screen_size();

        let height = screen_height;
        let width = screen_width / self.inner.window_count() as i32;

        for (index, window) in
            (0usize..).map_while(|index| self.inner.get_window_at_index(index).map(|w| (index, w)))
        {
            let x = index as i32 * width;
            let y = 0;
            let w = width;
            let h = height;
            window.inner.move_window(x, y, w, h)?;
        }
        Ok(())
    }
}
