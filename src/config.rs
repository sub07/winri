use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::{root_dir, system};

#[derive(Deserialize, Debug)]
pub struct Root {
    pub tiler: Tiler,
    pub default_window: Window,
}

#[derive(Deserialize, Debug)]
pub struct Tiler {
    pub padding: f32,
}

#[derive(Deserialize, Debug)]
pub struct Window {
    pub resize_increment: f32,
    pub border_style: BorderStyle,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BorderStyle {
    pub thickness: f32,
    pub color: csscolorparser::Color,
    pub radius: f32,
}

fn config_file() -> anyhow::Result<PathBuf> {
    let mut dir = root_dir()?;
    dir.push("config");
    dir.push("winri.yaml");
    Ok(dir)
}

pub fn load() -> anyhow::Result<Root> {
    let config_file = config_file()?;
    log::info!("Looking for config at {}", config_file.display());
    if !fs::exists(&config_file).unwrap_or(false) {
        let config = Root::default();
        log::info!("No config file found, loading defaults:\n {config:#?}");
        return Ok(config);
    }
    let content = fs::read_to_string(config_file)?;
    log::info!("conf content loaded:\n {content}");
    let config = yaml_serde::from_str::<Root>(&content)?;
    log::info!("Config successfully parsed:\n {config:#?}");
    Ok(config)
}

// A yaml merge would be better than an all hardcoded default. Avoid defaults boilerplate

impl Default for Root {
    fn default() -> Self {
        Self {
            tiler: Tiler {
                padding: default_tiler_padding(),
            },
            default_window: Window {
                resize_increment: default_window_resize_increment(),
                border_style: BorderStyle::default(),
            },
        }
    }
}

impl Default for Tiler {
    fn default() -> Self {
        Self {
            padding: default_tiler_padding(),
        }
    }
}

impl Default for BorderStyle {
    fn default() -> Self {
        Self {
            thickness: default_window_border_thickness(),
            color: default_color(),
            radius: default_window_border_radius(),
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            resize_increment: Default::default(),
            border_style: Default::default(),
        }
    }
}

fn default_color() -> csscolorparser::Color {
    system::highlight_color().map_or_else(
        |_| csscolorparser::Color::from_linear_rgba(0.1, 0.2, 0.8, 1.0),
        |color| csscolorparser::Color::from_linear_rgba(color.r, color.g, color.b, color.a),
    )
}

const fn default_tiler_padding() -> f32 {
    4.0
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
