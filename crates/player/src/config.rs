use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub(crate) struct PackageConfig {
    pub scenes: Vec<String>,
    pub fps: u32,
    pub image_map: HashMap<String, String>,
}
