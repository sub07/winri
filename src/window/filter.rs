use std::collections::HashSet;

use crate::window::{Window, manager::utils::WINRI_WINDOW_MANAGER_CLASS_NAME};

const IGNORED_CLASSES: &[&str] = &[
    "Progman",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "Xaml_WindowedPopupClass",
    "Shell_TrayWnd",
    "FindMyMouse",
    WINRI_WINDOW_MANAGER_CLASS_NAME,
];

const IGNORED_PROCESS_NAMES: &[&str] = &[
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
    filter_out_if!(IGNORED_CLASSES.contains(&window.class()?.as_str()));
    filter_out_if!(IGNORED_PROCESS_NAMES.contains(&window.process_name()?.as_str()));
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
