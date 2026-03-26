use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub prologue: Option<String>,
    #[serde(default)]
    pub font_directories: Vec<String>,
}

impl ProjectConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
