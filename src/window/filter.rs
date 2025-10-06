use crate::window::Window;

const SYSTEM_CLASSES: &[&str] = &[
    "Progman",
    "TopLevelWindowForOverflowXamlIsland",
    "XamlExplorerHostIslandWindow",
    "Xaml_WindowedPopupClass",
];

pub fn is_managed_window(window: Window) -> anyhow::Result<bool> {
    let class = window.class()?;
    let title = window.title()?;

    let is_system_window = SYSTEM_CLASSES.contains(&class.as_str());

    Ok(!is_system_window && title.is_some() && window.is_ancestor()? && window.is_visible()?)
}
