//! Semantic user intents, decoupled from the key combinations that trigger
//! them. Keybindings are mapped to these in [`crate::app::State::resolve_action`]
//! and then executed in [`crate::app::State::handle_action`], so rebinding keys
//! never touches behaviour.

/// A top-level user action, namespaced by the mode it applies to.
#[derive(Debug, Clone)]
pub enum Action {
    Tiler(TilerAction),
    Overview(OverviewAction),
    Exit,
}

/// Actions available while tiling (the normal mode).
#[derive(Debug, Clone)]
pub enum TilerAction {
    CloseCurrent,
    MoveFocusNext,
    MoveFocusPrevious,
    SwapWithNext,
    SwapWithPrevious,
    ResizeToFullscreen,
    ResizeToHalfScreen,
    IncrementWidth,
    DecrementWidth,
    OpenOverview,
    ForceRefresh,
}

/// Actions available while the overview is open.
#[derive(Debug, Clone)]
pub enum OverviewAction {
    CloseOverview,
}
