use std::{thread, time::Duration};

use windows::Win32::UI::WindowsAndMessaging::{
    MoveWindow, SHOW_WINDOW_CMD, SW_RESTORE, ShowWindow,
};

fn main() {
    unsafe {
        thread::sleep(Duration::from_secs(3));

        let currently_focused_window =
            windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();

        // un-maximize the window
        ShowWindow(currently_focused_window, SW_RESTORE).unwrap();

        MoveWindow(currently_focused_window, 50, 50, 500, 500, false).unwrap();
    }
}
