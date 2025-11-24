use std::collections::HashSet;

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
    "FindMyMouse",
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
    filter_out_if!(window.is_dialog()?);
    filter_out_if!(window.title()?.is_none());
    filter_out_if!(SYSTEM_CLASSES.contains(&window.class()?.as_str()));
    filter_out_if!(PROCESS_NAMES.contains(&window.process_name()?.as_str()));
    filter_out_if!(!window.is_valid()?);

    Ok(true)
}

pub fn opened_windows() -> anyhow::Result<HashSet<Window>> {
    let windows = Window::enumerate()?
        .into_iter()
        .filter(|window| is_managed_window(*window).unwrap_or(false))
        .collect::<HashSet<_>>();

    Ok(windows)
}
