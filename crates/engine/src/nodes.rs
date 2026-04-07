use crate::basictypes::{AvId, FrameId, NodeId};
use crate::eval::EvalCtx;
use renderer::Color as RendererColor;
use serde::{Deserialize, Deserializer, de};
use std::sync::Arc;
use serde::de::DeserializeOwned;
use crate::values::{Color, Expr, Value};
// ──────────────────────────── Transition ───────────────────────────────────

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    Step,
    Linear,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct TopLevelExpr<T: Value + DeserializeOwned>(Expr<T>);

impl<T: Value + DeserializeOwned> TopLevelExpr<T> {
    #[inline]
    pub fn get_expr(&self) -> &Expr<T> {
        &self.0
    }
}

// ──────────────────────────────── Mixins ───────────────────────────────────

/// Mirrors `PositionMixin` in Python.
#[derive(Debug, Deserialize)]
pub struct Position {
    pub x: TopLevelExpr<f64>,
    pub y: TopLevelExpr<f64>,
}

/// Mirrors `SizeMixin` in Python.
#[derive(Debug, Deserialize)]
pub struct Size {
    pub width: TopLevelExpr<f64>,
    pub height: TopLevelExpr<f64>,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Deserialize)]
pub struct Style {
    pub fill_color: TopLevelExpr<Option<Color>>,
    pub stroke_color: TopLevelExpr<Option<Color>>,
    pub stroke_width: TopLevelExpr<f64>,
    pub alpha: TopLevelExpr<f64>,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Deserialize)]
pub struct TextStyle {
    #[serde(flatten)]
    pub style: Style,
    pub font: TopLevelExpr<Arc<String>>,
    pub font_size: TopLevelExpr<f64>,
    pub italic: TopLevelExpr<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Layout {
    Center,
    Column {
        gap: TopLevelExpr<f64>,
        align: TopLevelExpr<f64>,
    },
    Row {
        gap: TopLevelExpr<f64>,
        align: TopLevelExpr<f64>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// Python: `Group(PositionMixin, SizeMixin, AlphaMixin)` + rotation + scale
    Group {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        alpha: TopLevelExpr<f64>,
        rotation: TopLevelExpr<f64>,
        pivot_x: TopLevelExpr<f64>,
        pivot_y: TopLevelExpr<f64>,
        scale_x: TopLevelExpr<f64>,
        scale_y: TopLevelExpr<f64>,
        z_level: TopLevelExpr<f64>,
        layout: Layout,
        #[serde(default)]
        children: Vec<NodeId>,
    },
    /// Python: `Rect(PositionMixin, SizeMixin, StyleMixin)`
    Rect {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: TopLevelExpr<f64>,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Ellipse(PositionMixin, SizeMixin, StyleMixin)`
    Ellipse {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: TopLevelExpr<f64>,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Path(StyleMixin)`
    Path {
        #[serde(flatten)]
        style: Style,
        z_level: TopLevelExpr<f64>,
        #[serde(default)]
        children: Vec<NodeId>,
    },

    /// Python: `Text` — block of styled text with Line children
    Text {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        text_style: TextStyle,
        z_level: TopLevelExpr<f64>,
        sh_language: Option<Arc<String>>,
        sh_theme: Option<Arc<String>>,
        #[serde(default)]
        children: Vec<NodeId>,
    },
    /// Group containing instance of other TextGroups or TextSpans.
    #[serde(rename = "t_group")]
    TextGroup {
        #[serde(flatten)]
        text_style: TextStyle,
        #[serde(default)]
        children: Vec<NodeId>,
    },
    /// A text run with a concrete string value
    #[serde(rename = "t_span")]
    TextSpan {
        #[serde(flatten)]
        text_style: TextStyle,
        text: TopLevelExpr<Arc<String>>,
    },

    /// Path commands; They always have Path as parent
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
        c1_x: TopLevelExpr<f64>,
        c1_y: TopLevelExpr<f64>,
        c2_x: TopLevelExpr<f64>,
        c2_y: TopLevelExpr<f64>,
    },
    Close,

    /// An image (SVG for now).  `path` is resolved relative to the project directory.
    Image {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: TopLevelExpr<f64>,
        alpha: TopLevelExpr<f64>,
        path: TopLevelExpr<Arc<String>>,
        /// When true, scale the image to fit inside the node box while
        /// preserving the SVG's intrinsic aspect ratio (letterbox/pillarbox).
        keep_aspect: TopLevelExpr<bool>,
        /// Optional child layer nodes (kind = "layer").
        #[serde(default)]
        children: Vec<NodeId>,
    },

    /// A layer within an SVG image.  Always a child of an Image node.
    Layer {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: TopLevelExpr<f64>,
        alpha: TopLevelExpr<f64>,
        /// Matches the SVG group `id` attribute.
        layer_name: Arc<String>,
        #[serde(default)]
        children: Vec<NodeId>,
    },
}

impl NodeKind {
    pub fn children(&self) -> &[NodeId] {
        match self {
            NodeKind::Group { children, .. }
            | NodeKind::Path { children, .. }
            | NodeKind::Text { children, .. }
            | NodeKind::TextGroup { children, .. }
            | NodeKind::Image { children, .. }
            | NodeKind::Layer { children, .. } => children,
            _ => &[],
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Node {
    pub id: NodeId,
    #[serde(skip)]
    pub parent: Option<NodeId>,
    #[serde(default)]
    pub start: FrameId,
    #[serde(default)]
    pub end: Option<FrameId>,
    #[serde(flatten)]
    pub kind: NodeKind,
}

impl Node {
    pub fn is_active(&self, frame: FrameId) -> bool {
        if frame < self.start {
            return false;
        }
        if let Some(e) = self.end
            && frame >= e
        {
            return false;
        }
        true
    }

    /// Walk parent links to find the nearest `Text` ancestor of `node_id`.
    pub fn text_ancestor<'a>(&self, ctx: &'a EvalCtx) -> anyhow::Result<&'a Node> {
        let mut current = self.parent;
        loop {
            let Some(node_id) = current else {
                anyhow::bail!("node {:?} has no Text ancestor", self.id)
            };
            let node = ctx.node(node_id)?;
            if matches!(node.kind, NodeKind::Text { .. }) {
                return Ok(node);
            }
            current = node.parent;
        }
    }
}

/// Root of the scene definition. Mirrors `Scene(SizeMixin)` in Python.
#[derive(Debug, Deserialize)]
pub struct SceneDef {
    pub id: NodeId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub size: Size,
    pub fill_color: TopLevelExpr<Option<Color>>,
    pub frames: u32,
    #[serde(default)]
    pub cues: Vec<u32>,
    #[serde(default)]
    pub children: Vec<NodeId>,
}
