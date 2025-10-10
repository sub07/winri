use crate::window::Window;

const SYSTEM_CLASSES: &[&str] = &[
    "Progman",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "Xaml_WindowedPopupClass",
];

const PROCESS_NAMES: &[&str] = &[
    "Microsoft.CmdPal.UI.exe",
    "PowerToys.MeasureToolUI.exe",
    "ShareX.exe",
    "SnippingTool.exe",
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
    filter_out_if!(!window.is_ancestor()?);
    filter_out_if!(window.title()?.is_none());
    filter_out_if!(SYSTEM_CLASSES.contains(&window.class()?.as_str()));
    filter_out_if!(PROCESS_NAMES.contains(&window.process_name()?.as_str()));

    Ok(true)
}
