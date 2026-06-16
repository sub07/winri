//! Decides which windows winri manages. Tiling everything would be chaos
//! (system UI, popups, tooltips, winri's own overlay), so the rules here keep
//! the set down to real, user-facing application windows.
use std::collections::HashSet;

use crate::window::Window;

/// Window class an app can set to opt out of tiling.
pub const WINRI_IGNORED_CLASS_NAME: &str = "Winri_IgnoreWindowClass";
/// Title substring that opts a window out of tiling. winri's own windows use it
/// (see [`crate::app::State::title`]) so it never manages itself.
pub const WINRI_IGNORED_WINDOW_TITLE_SUBSTRING: &str = "[Winri Ignore Window]";

const IGNORED_CLASSES: &[&str] = &[
    "Progman",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "Xaml_WindowedPopupClass",
    "Shell_TrayWnd",
    "FindMyMouse",
    WINRI_IGNORED_CLASS_NAME,
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

/// Whether a single window qualifies for tiling, applying every exclusion rule
/// (visibility, cloaking, dialogs, ignored classes/processes, …).
pub fn should_be_tiled(window: Window) -> anyhow::Result<bool> {
    filter_out_if!(!window.is_visible()?);
    filter_out_if!(window.is_cloaked()?);
    filter_out_if!(!window.is_ancestor()?);
    filter_out_if!(window.is_dialog()?);
    let title = window.title()?;
    filter_out_if!(title.is_none());
    filter_out_if!(title.is_some_and(|title| title.contains(WINRI_IGNORED_WINDOW_TITLE_SUBSTRING)));
    filter_out_if!(IGNORED_CLASSES.contains(&window.class()?.as_str()));
    filter_out_if!(IGNORED_PROCESS_NAMES.contains(&window.process_name()?.as_str()));
    filter_out_if!(!window.is_valid()?);

    Ok(true)
}

/// The current set of tileable windows: every top-level window passed through
/// [`should_be_tiled`]. This is the snapshot fed to the tiler.
pub fn opened_windows() -> anyhow::Result<HashSet<Window>> {
    let windows = Window::enumerate()?
        .into_iter()
        .filter(|window| should_be_tiled(*window).unwrap_or(false))
        .collect::<HashSet<_>>();

    Ok(windows)
}
