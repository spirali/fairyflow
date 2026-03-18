use serde::Deserialize;

fn default_stroke_width() -> f64 { 1.0 }
fn default_scale() -> f64 { 1.0 }
fn default_alpha() -> f64 { 1.0 }

/// Mirrors PositionMixin in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

/// Mirrors SizeMixin in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Size {
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
}

/// Mirrors StyleMixin (which extends AlphaMixin) in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Style {
    pub fill_color: Option<String>,
    #[serde(default)]
    pub stroke_color: Option<String>,
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f64,
    #[serde(default = "default_alpha")]
    pub alpha: f64,
}

/// A single command in a path. Mirrors PathMove / PathLine / PathCubic in Python.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PathCommand {
    Move {
        #[serde(flatten)]
        position: Position,
    },
    Line {
        #[serde(flatten)]
        position: Position,
    },
    Cubic {
        #[serde(flatten)]
        position: Position,
        c1_x: f64,
        c1_y: f64,
        c2_x: f64,
        c2_y: f64,
    },
}

/// Kind-specific data for a scene node.
/// Each variant carries exactly the mixins its Python counterpart inherits.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeKind {
    /// Python: Node(PositionMixin, SizeMixin, AlphaMixin) + explicit scale and rotation
    Node {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        #[serde(default = "default_alpha")]
        alpha: f64,
        #[serde(default = "default_scale")]
        scale_x: f64,
        #[serde(default = "default_scale")]
        scale_y: f64,
        #[serde(default)]
        rotation: f64,
        children: Vec<SceneNode>,
    },
    /// Python: Rect(PositionMixin, SizeMixin, StyleMixin)
    Rect {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: Ellipse(PositionMixin, SizeMixin, StyleMixin)
    Ellipse {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: Path(StyleMixin)
    Path {
        #[serde(flatten)]
        style: Style,
        children: Vec<PathCommand>,
    },
}

/// A node in the scene tree. Only carries the id;
/// all other data lives in the kind-specific variant.
#[derive(Debug, Clone, Deserialize)]
pub struct SceneNode {
    pub id: u64,

    #[serde(flatten)]
    pub kind: NodeKind,
}

/// Root of a single frame. Mirrors Scene(SizeMixin) in Python.
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
        match &first.kind {
            NodeKind::Node { position, children, .. } => {
                assert_eq!(position.x, 100.0);
                match &children[0].kind {
                    NodeKind::Rect { style, .. } => assert_eq!(style.fill_color.as_deref(), Some("red")),
                    _ => panic!("expected rect"),
                }
            }
            _ => panic!("expected node"),
        }
    }

    #[test]
    fn parse_path() {
        let json = r##"{"key_frames":[0],"frames":[{"kind":"scene","id":0,"width":200,"height":200,"scale":1,"fill_color":"#ffffff","children":[{"kind":"path","id":1,"fill_color":null,"stroke_color":null,"stroke_width":1,"alpha":1,"children":[{"kind":"move","id":2,"x":20,"y":20},{"kind":"line","id":3,"x":30,"y":40},{"kind":"cubic","id":4,"x":40,"y":50,"c1_x":30,"c1_y":0,"c2_x":30,"c2_y":40}]}]}]}"##;
        let anim = parse_scene(json).expect("parse failed");
        let path_node = &anim.frames[0].children[0];
        match &path_node.kind {
            NodeKind::Path { children, .. } => assert_eq!(children.len(), 3),
            _ => panic!("expected path"),
        }
    }
}
