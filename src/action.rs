#[derive(Debug)]
pub enum Action {
    Tiler(TilerAction),
    Overview(OverviewAction),
    Exit,
}

#[derive(Debug)]
pub enum TilerAction {
    CloseCurrent,
    MoveFocusNext,
    MoveFocusPrevious,
    SwapWithNext,
    SwapWithPrevious,
    ResizeToFullscreen,
    ResizeToHalfScreen,
    OpenOverview,
}

#[derive(Debug)]
pub enum OverviewAction {
    CloseOverview,
}
