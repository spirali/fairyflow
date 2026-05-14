use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    #[serde(default = "default_fps")]
    pub fps: u32,
    pub prologue: Option<String>,
    #[serde(default)]
    pub font_directories: Vec<String>,
    #[serde(default, rename = "font-aliases")]
    pub font_aliases: HashMap<String, String>,
}

impl ProjectConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

fn default_fps() -> u32 {
    24
}
