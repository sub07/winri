use std::collections::HashMap;

use crate::{
    app::{self, manager::thumbnail::ThumbnailId},
    window::Window,
};

pub struct OverviewState {
    thumbnails: HashMap<ThumbnailId, Window>,
}

impl app::State {}
