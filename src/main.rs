mod window;

use windows::Win32::{
    Foundation::HWND,
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook},
        WindowsAndMessaging::{
            DispatchMessageA, EVENT_OBJECT_CREATE, EVENT_SYSTEM_FOREGROUND, GetMessageA, MSG,
            TranslateMessage, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
        },
    },
};

use crate::window::Window;

unsafe extern "system" fn hook_callback(
    _hwineventhook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _idobject: i32,
    _idchild: i32,
    _ideventthread: u32,
    _dwmseventtime: u32,
) {
    if event == EVENT_OBJECT_CREATE || event == EVENT_SYSTEM_FOREGROUND {
        fn handle_window(hwnd: HWND) -> anyhow::Result<()> {
            let window = Window::from(hwnd)?;
            if let Some(title) = window.title()?
                && window.is_ancestor()?
                && (window.is_visible()? || window.is_uwp()?)
            {
                println!(
                    "{title} - {} - {:?} - ancestor: {:?}",
                    window.class()?,
                    window,
                    window.ancestor()?
                );
            }

            // if window.class()? == "MSTaskListWClass" {
            //     window.move_window(50, 50, 1000, 500)?;
            // }
            Ok(())
        }

        if let Err(err) = handle_window(hwnd) {
            eprintln!("{err}");
        }
    }
}

fn main() -> anyhow::Result<()> {
    unsafe {
        // let currently_focused_window =
        // windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        SetWinEventHook(
            EVENT_OBJECT_CREATE,
            EVENT_OBJECT_CREATE,
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
    }
    Ok(())
}
