mod scene;
mod render;

pub use scene::{Animation, NodeKind, Scene, SceneNode, Style, parse_scene};
pub use render::render_scene;