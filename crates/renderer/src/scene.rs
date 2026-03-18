use serde::Deserialize;

fn default_stroke_width() -> f64 { 1.0 }

/// Visual style fields (mirrors StyledItem in Python).
#[derive(Debug, Clone, Deserialize)]
pub struct Style {
    pub fill_color: Option<String>,
    #[serde(default)]
    pub stroke_color: Option<String>,
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f64,
}

/// Kind-specific data for a scene node.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeKind {
    Node {
        children: Vec<SceneNode>,
    },
    Rect {
        #[serde(flatten)]
        style: Style,
    },
    Ellipse {
        #[serde(flatten)]
        style: Style,
    },
}

/// A node in the scene tree with common item fields and kind-specific data.
#[derive(Debug, Clone, Deserialize)]
pub struct SceneNode {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub rotation: f64,
    pub alpha: f64,

    #[serde(flatten)]
    pub kind: NodeKind,
}

/// Root of a single frame. Mirrors Scene in Python (which extends Node).
#[derive(Debug, Clone, Deserialize)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub fill_color: String,
    pub children: Vec<SceneNode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Animation {
    pub key_frames: Vec<u64>,
    pub frames: Vec<Scene>,
}

pub fn parse_scene(json: &str) -> Result<Animation, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample() {
        let json = r#"{"key_frames":[0,10],"frames":[{"kind":"scene","id":0,"x":0,"y":0,"width":300,"height":300,"scale":1,"rotation":0,"fill_color":"white","children":[{"kind":"node","id":1,"x":100,"y":0,"width":5,"height":5,"scale":1,"rotation":0,"alpha":1,"children":[{"kind":"rect","id":2,"x":5,"y":0,"width":5,"height":5,"scale":1,"rotation":0,"alpha":1,"fill_color":"red"}]},{"kind":"node","id":3,"x":100,"y":0,"width":0,"height":0,"scale":1,"rotation":0,"alpha":1,"children":[{"kind":"rect","id":4,"x":0,"y":0,"width":5,"height":5,"scale":1,"rotation":0,"alpha":1,"fill_color":"yellow"}]}]}]}"#;
        let anim = parse_scene(json).expect("parse failed");
        assert_eq!(anim.key_frames, vec![0, 10]);
        assert_eq!(anim.frames.len(), 1);
        let scene = &anim.frames[0];
        assert_eq!(scene.width, 300.0);
        assert_eq!(scene.height, 300.0);
        assert_eq!(scene.children.len(), 2);
        let first = &scene.children[0];
        assert_eq!(first.x, 100.0);
        match &first.kind {
            NodeKind::Node { children } => {
                match &children[0].kind {
                    NodeKind::Rect { style } => assert_eq!(style.fill_color.as_deref(), Some("red")),
                    _ => panic!("expected rect"),
                }
            }
            _ => panic!("expected node"),
        }
    }
}
