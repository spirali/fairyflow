use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ProjectConfig {
    pub prologue: String,
    #[serde(default)]
    pub font_dirs: Vec<String>,
}