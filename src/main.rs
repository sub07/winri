mod window;

use std::{thread, time::Duration};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook},
            WindowsAndMessaging::{
                DispatchMessageA, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY,
                EVENT_SYSTEM_FOREGROUND, EnumWindows, GetMessageA, MSG, TranslateMessage,
                WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
            },
        },
    },
    core::BOOL,
};

pub fn enum_windows() -> anyhow::Result<Vec<Window>> {
    unsafe extern "system" fn enum_callback(window: HWND, out_list: LPARAM) -> BOOL {
        let list = unsafe { &mut *(out_list.0 as *mut Vec<HWND>) };
        list.push(window);
        true.into() // Continue enumeration
    }

    let mut result = Vec::new();

    unsafe {
        EnumWindows(Some(enum_callback), LPARAM(&raw mut result as isize))?;
    }

    let windows = result
        .into_iter()
        .filter_map(|hwnd| Window::from(hwnd).ok())
        .filter(|window| {
            is_managed_window(*window)
                .inspect_err(|err| {
                    eprintln!(
                        "Error checking managed window {:?} ({:?}): {}",
                        window.title(),
                        window.class(),
                        err
                    );
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    Ok(windows)
}

use crate::window::{Window, filter::is_managed_window};

unsafe extern "system" fn hook_callback(
    _hwineventhook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _idobject: i32,
    _idchild: i32,
    _ideventthread: u32,
    _dwmseventtime: u32,
) {
    fn handle_window() -> anyhow::Result<()> {
        for window in enum_windows()? {
            println!(
                "{} - {}",
                window.title()?.as_deref().unwrap_or("No title"),
                window.class()?,
            );
        }
        Ok(())
    }

    if let Err(err) = handle_window() {
        eprintln!("{err}");
    }
}

fn main() -> anyhow::Result<()> {
    unsafe {
        // let currently_focused_window =
        // windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        SetWinEventHook(
            EVENT_OBJECT_CREATE,
            EVENT_OBJECT_DESTROY,
            None,
            Some(hook_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        let mut message = MSG::default();
        while GetMessageA(&raw mut message, None, 0, 0).as_bool() {
            TranslateMessage(&raw const message).ok()?;
            DispatchMessageA(&raw const message);
        }

        // loop {
        //     println!("--------------------------");
        //     for window in enum_windows()? {
        //         println!(
        //             "{} - {}",
        //             window.title()?.as_deref().unwrap_or("No title"),
        //             window.class()?,
        //         );
        //     }
        //     thread::sleep(Duration::from_secs(2));
        // }
    }
    Ok(())
}
