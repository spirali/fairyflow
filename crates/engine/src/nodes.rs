use std::collections::HashSet;
use crate::basictypes::{AvId, FrameId, NodeId};
use crate::eval::EvalCtx;
use renderer_core::Color as RendererColor;
use serde::{Deserialize, Deserializer, de};
use std::sync::Arc;
use serde::de::DeserializeOwned;
use crate::values::{Color, Expr, Value};
// ──────────────────────────── Transition ───────────────────────────────────

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Transition {
    #[serde(rename="S")]
    Step,
    #[serde(rename="L")]
    Linear,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct AttrExpr<T: Value + DeserializeOwned>(Expr<T>);

impl<T: Value + DeserializeOwned> AttrExpr<T> {
    #[inline]
    pub fn get_expr(&self) -> &Expr<T> {
        &self.0
    }
}

// ──────────────────────────────── Mixins ───────────────────────────────────

/// Mirrors `PositionMixin` in Python.
#[derive(Debug, Deserialize)]
pub struct Position {
    pub x: AttrExpr<f64>,
    pub y: AttrExpr<f64>,
}

/// Mirrors `SizeMixin` in Python.
#[derive(Debug, Deserialize)]
pub struct Size {
    pub width: AttrExpr<f64>,
    pub height: AttrExpr<f64>,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Deserialize)]
pub struct Style {
    pub fill_color: AttrExpr<Option<Color>>,
    pub stroke_color: AttrExpr<Option<Color>>,
    pub stroke_width: AttrExpr<f64>,
    pub alpha: AttrExpr<f64>,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Deserialize)]
pub struct TextStyle {
    #[serde(flatten)]
    pub style: Style,
    pub font: AttrExpr<Arc<String>>,
    pub font_size: AttrExpr<f64>,
    pub font_weight: AttrExpr<f64>,
    pub italic: AttrExpr<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Layout {
    Center,
    Column {
        gap: AttrExpr<f64>,
        align: AttrExpr<f64>,
    },
    Row {
        gap: AttrExpr<f64>,
        align: AttrExpr<f64>,
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
        alpha: AttrExpr<f64>,
        rotation: AttrExpr<f64>,
        pivot_x: AttrExpr<f64>,
        pivot_y: AttrExpr<f64>,
        scale_x: AttrExpr<f64>,
        scale_y: AttrExpr<f64>,
        z_level: AttrExpr<f64>,
        clip_x: AttrExpr<f64>,
        clip_y: AttrExpr<f64>,
        clip_w: AttrExpr<f64>,
        clip_h: AttrExpr<f64>,
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
        z_level: AttrExpr<f64>,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Ellipse(PositionMixin, SizeMixin, StyleMixin)`
    Ellipse {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: AttrExpr<f64>,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Path(StyleMixin)`
    Path {
        #[serde(flatten)]
        style: Style,
        z_level: AttrExpr<f64>,
        crop_start: AttrExpr<f64>,
        crop_end: AttrExpr<f64>,
        #[serde(default)]
        children: Vec<NodeId>,
    },

    /// Python: `Text` — block of styled text with Line children
    Text {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        text_style: TextStyle,
        z_level: AttrExpr<f64>,
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
        text: AttrExpr<Arc<String>>,
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
        c1_x: AttrExpr<f64>,
        c1_y: AttrExpr<f64>,
        c2_x: AttrExpr<f64>,
        c2_y: AttrExpr<f64>,
    },
    Close,

    /// An image (SVG for now).  `path` is resolved relative to the project directory.
    Image {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: AttrExpr<f64>,
        alpha: AttrExpr<f64>,
        path: AttrExpr<Arc<String>>,
        /// When true, scale the image to fit inside the node box while
        /// preserving the SVG's intrinsic aspect ratio (letterbox/pillarbox).
        keep_aspect: AttrExpr<bool>,
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
        z_level: AttrExpr<f64>,
        alpha: AttrExpr<f64>,
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

    pub fn collect_images(&self, image_paths: &mut HashSet<Arc<String>>) {
        match &self.kind {
            NodeKind::Image {
                path, ..
            } => {
                todo!()
            }
            _ => {}
        }
    }
}

/// Root of the scene definition. Mirrors `Scene(SizeMixin)` in Python.
#[derive(Debug, Deserialize)]
pub(crate) struct SceneDef {
    pub id: NodeId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub size: Size,
    pub fill_color: AttrExpr<Option<Color>>,
    pub frames: u32,
    #[serde(default)]
    pub cues: Vec<u32>,
    #[serde(default)]
    pub children: Vec<NodeId>,
}