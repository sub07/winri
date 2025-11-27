use std::path::PathBuf;

use log4rs::{
    Config,
    config::{Root, runtime::RootBuilder},
};

use crate::root_dir;

fn log_dir() -> anyhow::Result<PathBuf> {
    Ok(root_dir()?.join("logs"))
}

pub fn setup_logger() {
    log4rs::init_config(Config::builder().build(Root::builder().build(log::LevelFilter::Info)))
}
