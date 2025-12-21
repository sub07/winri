#[derive(Debug, Clone)]
pub enum Action {
    Tiler(TilerAction),
    Overview(OverviewAction),
    Exit,
}

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
}

#[derive(Debug, Clone)]
pub enum OverviewAction {
    CloseOverview,
}
