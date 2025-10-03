use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GA_ROOT, GWL_STYLE, GetAncestor, GetWindowLongA, GetWindowTextA, WINDOW_STYLE,
        WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    },
};

use crate::window::Window;

const junkClasses: &[&str] = &[
    "IME",
    "Static",
    "WorkerW",
    "OleMainThreadWndClass",
    "DcppUserAdapterWindowClass",
    "CicMarshalWndClass",
    "SystemUserAdapterWindowClass",
    "MSCTFIME UI",
    "tooltips_class32",
    "CtrlNotifySink",
    "Button",
    "Shell Preview Extension Host",
    "InputSiteWindowClass",
    "Microsoft.UI.Content.DesktopChildSiteBridge",
    "MITMessageOnlyWindowClass",
    "InputNonClientPointerSource",
    "LiftedDMITCursorRecalculateClass",
    "WinUI_urlmonMessageWindow",
    "URL Moniker Notification Window",
    "DirectUIHWND",
];

// pub fn is_junk_window(window: &Window) -> bool {
//     for junkClass in junkClasses {
//         if *junkClass == className {
//             return true;
//         }
//     }
//     false
// }

pub fn is_top_level_window(hwnd: HWND) -> bool {
    unsafe {
        let style = GetWindowLongA(hwnd, GWL_STYLE);
        if style < 0 {
            return false;
        }
        let style = WINDOW_STYLE(style as u32);
        (style.contains(WS_VISIBLE) || style.contains(WS_OVERLAPPEDWINDOW))
            && GetAncestor(hwnd, GA_ROOT) == hwnd
    }
}

pub fn meaningful_title(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut title = [0u8; 256];
        GetWindowTextA(hwnd, &mut title);
        std::str::from_utf8(&title)
            .ok()
            .filter(|str| !str.is_empty())
            .map(|s| s.to_string())
    }
}
