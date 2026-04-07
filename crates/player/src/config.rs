use std::collections::HashMap;
use std::path::PathBuf;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct PackageConfig {
    pub scenes: Vec<String>,
    pub fps: u32,
    pub image_map: HashMap<String, String>,
}