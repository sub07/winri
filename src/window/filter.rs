use std::collections::HashSet;

use log::error;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::EnumWindows,
    },
    core::BOOL,
};

use crate::window::Window;

const SYSTEM_CLASSES: &[&str] = &[
    "Progman",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "Xaml_WindowedPopupClass",
    "Shell_TrayWnd",
];

const PROCESS_NAMES: &[&str] = &[
    "Microsoft.CmdPal.UI.exe",
    "PowerToys.MeasureToolUI.exe",
    "ShareX.exe",
    "SnippingTool.exe",
    "PowerToys.PowerLauncher.exe",
    "Ditto.exe",
];

macro_rules! filter_out_if {
    ($bool:expr) => {
        if $bool {
            return Ok(false);
        }
    };
}

pub fn is_managed_window(window: Window) -> anyhow::Result<bool> {
    filter_out_if!(!window.is_visible()?);
    filter_out_if!(window.is_cloaked()?);
    filter_out_if!(!window.is_ancestor()?);
    filter_out_if!(window.title()?.is_none());
    filter_out_if!(SYSTEM_CLASSES.contains(&window.class()?.as_str()));
    filter_out_if!(PROCESS_NAMES.contains(&window.process_name()?.as_str()));

    Ok(true)
}

pub fn opened_windows() -> anyhow::Result<HashSet<Window>> {
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
                    error!(
                        "Error filtering window ({err}): {}",
                        window.get_formatted_extensive_info()
                    );
                })
                .unwrap_or(false)
        })
        .collect::<HashSet<_>>();

    Ok(windows)
}
