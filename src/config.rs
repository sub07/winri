use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::{root_dir, system};

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct Root {
    pub default_window: Window,
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Window {
    pub padding: f32,
    pub resize_increment: f32,
    pub border_thickness: f32,
    pub border_color: csscolorparser::Color,
    pub border_radius: f32,
}

fn config_files() -> anyhow::Result<Vec<PathBuf>> {
    let mut pathes = Vec::new();
    let config_dir = root_dir()?.join("config");

    pathes.push(config_dir.join("winri.yaml"));
    pathes.push(config_dir.join("winri.yml"));

    Ok(pathes)
}

fn elect_config_file(pathes: Vec<PathBuf>) -> Option<PathBuf> {
    for path in pathes {
        log::info!("Looking for conf at {}", path.display());
        if fs::exists(&path).unwrap_or(false) {
            return Some(path);
        }
    }
    None
}

pub fn load() -> anyhow::Result<Root> {
    let config_files = config_files()?;
    if let Some(config_file) = elect_config_file(config_files) {
        let content = fs::read_to_string(config_file)?;
        log::info!("conf content loaded:\n {content}");
        let config = yaml_serde::from_str::<Root>(&content)?;
        log::info!("Config successfully parsed:\n {config:#?}");
        return Ok(config);
    }
    let config = Root::default();
    log::info!("No config file found, loading defaults:\n {config:#?}");
    Ok(config)
}

impl Default for Window {
    fn default() -> Self {
        Self {
            padding: default_window_padding(),
            resize_increment: default_window_resize_increment(),
            border_thickness: default_window_border_thickness(),
            border_color: default_window_border_color(),
            border_radius: default_window_border_radius(),
        }
    }
}

const fn default_window_padding() -> f32 {
    10.0
}

fn default_window_border_color() -> csscolorparser::Color {
    system::highlight_color().map_or_else(
        |_| csscolorparser::Color::from_linear_rgba(0.1, 0.2, 0.8, 1.0),
        |color| csscolorparser::Color::from_linear_rgba(color.r, color.g, color.b, color.a),
    )
}

const fn default_window_border_thickness() -> f32 {
    4.0
}

const fn default_window_border_radius() -> f32 {
    4.0
}

const fn default_window_resize_increment() -> f32 {
    20.0
}
