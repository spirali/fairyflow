use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct PackageConfig {
    pub scenes: Vec<String>,
    pub fps: u32,
    pub image_map: HashMap<String, String>,
}