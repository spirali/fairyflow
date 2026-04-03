use crate::basictypes::{AvId, FrameId, NodeId};
use crate::eval::EvalCtx;
use renderer::Color as RendererColor;
use serde::{Deserialize, Deserializer, de};
use std::sync::Arc;
// ──────────────────────────── Transition ───────────────────────────────────

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    Step,
    Linear,
}

#[derive(Debug, Clone)]
pub struct Color(RendererColor);

impl Color {
    pub fn into_inner(self) -> RendererColor {
        self.0
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.0.set_alpha(alpha);
    }

    pub fn interpolate(a: &Color, b: &Color, t: f64) -> Color {
        Color(a.0.interpolate(&b.0, t))
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Helper {
            color: String,
        }
        let h = Helper::deserialize(d)?;
        RendererColor::from_html(&h.color)
            .map(Color)
            .ok_or_else(|| de::Error::custom(format!("invalid color '{}'", h.color)))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Arc<String>),
    Color(Color),
    None,
    Recursive,
}

impl Value {
    pub fn is_number(&self) -> bool {
        match self {
            Value::Int(_) | Value::Float(_) => true,
            _ => false,
        }
    }

    pub fn as_f64(&self) -> anyhow::Result<f64> {
        match self {
            Value::Int(v) => Ok(*v as f64),
            Value::Float(v) => Ok(*v),
            Value::Recursive => Ok(0.0),
            _ => Err(anyhow::Error::msg("Value is not a number")),
        }
    }

    pub fn into_color(self) -> Option<renderer::Color> {
        match self {
            Value::Color(c) => Some(c.into_inner()),
            _ => None,
        }
    }

    pub fn as_str(&self) -> anyhow::Result<&str> {
        match self {
            Value::Str(s) => Ok(s.as_str()),
            _ => Err(anyhow::Error::msg("Value is not a string")),
        }
    }

    pub fn as_string_ref(&self) -> anyhow::Result<Arc<String>> {
        match self {
            Value::Str(s) => Ok(s.clone()),
            _ => Err(anyhow::Error::msg("Value is not a string")),
        }
    }

    pub fn as_bool(&self) -> anyhow::Result<bool> {
        match self {
            Value::Bool(b) => Ok(*b),
            _ => Err(anyhow::Error::msg("Value is not a bool")),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct AvRef(AvId);

impl AvRef {
    #[inline]
    pub fn get_id(&self) -> AvId {
        self.0
    }
}

impl<'de> Deserialize<'de> for AvRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Helper {
            av: u64,
        }
        let h = Helper::deserialize(d)?;
        Ok(AvRef(AvId::new(h.av)))
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct NodeRef(NodeId);

impl NodeRef {
    #[inline]
    pub fn get_id(&self) -> NodeId {
        self.0
    }
}

impl<'de> Deserialize<'de> for NodeRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Helper {
            id: u32,
        }
        let h = Helper::deserialize(d)?;
        Ok(NodeRef(NodeId::new(h.id)))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CallParamsPair {
    pub a: Expr,
    pub b: Expr,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CallParamsNodeTransform {
    pub source: NodeRef,
    pub target: NodeRef,
    pub x: Expr,
    pub y: Expr,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "fn", rename_all = "snake_case")]
pub enum CallExpr {
    NodeTransformX(Box<CallParamsNodeTransform>),
    NodeTransformY(Box<CallParamsNodeTransform>),
    #[serde(rename = "+")]
    Add(Box<CallParamsPair>),
    #[serde(rename = "-")]
    Sub(Box<CallParamsPair>),
    #[serde(rename = "*")]
    Mul(Box<CallParamsPair>),
    DefaultWidth {
        node: NodeRef,
    },
    DefaultHeight {
        node: NodeRef,
    },
    DefaultX {
        node: NodeRef,
    },
    DefaultY {
        node: NodeRef,
    },
}

// ─────────────────────────── KeyframeValue ─────────────────────────────────

/// The `value` field inside an animated keyframe: a constant value or a call
/// expression.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Expr {
    Const(Value),
    Call(CallExpr),
    Av(AvRef),
    Inherited { expr: Box<Expr> },
}

impl Expr {
    /// Returns `true` if this expression is a `default_width` call on `node_id`.
    pub fn is_default_width_of(&self, node_id: NodeId) -> bool {
        match self {
            Expr::Call(CallExpr::DefaultWidth { node }) => node.get_id() == node_id,
            Expr::Inherited { expr } => expr.is_default_width_of(node_id),
            _ => false,
        }
    }

    /// Returns `true` if this expression is a `default_height` call on `node_id`.
    pub fn is_default_height_of(&self, node_id: NodeId) -> bool {
        match self {
            Expr::Call(CallExpr::DefaultHeight { node }) => node.get_id() == node_id,
            Expr::Inherited { expr } => expr.is_default_height_of(node_id),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct TopLevelExpr(Expr);

impl TopLevelExpr {
    pub fn new(expr: Expr) -> Self {
        TopLevelExpr(expr)
    }

    #[inline]
    pub fn get_expr(&self) -> &Expr {
        &self.0
    }
}

// ──────────────────────────────── Mixins ───────────────────────────────────

/// Mirrors `PositionMixin` in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub x: TopLevelExpr,
    pub y: TopLevelExpr,
}

/// Mirrors `SizeMixin` in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Size {
    pub width: TopLevelExpr,
    pub height: TopLevelExpr,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Style {
    pub fill_color: TopLevelExpr,
    pub stroke_color: TopLevelExpr,
    pub stroke_width: TopLevelExpr,
    pub alpha: TopLevelExpr,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct TextStyle {
    #[serde(flatten)]
    pub style: Style,
    pub font: TopLevelExpr,
    pub font_size: TopLevelExpr,
    pub italic: TopLevelExpr,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Layout {
    Center,
    Column {
        gap: TopLevelExpr,
        align: TopLevelExpr,
    },
    Row {
        gap: TopLevelExpr,
        align: TopLevelExpr,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// Python: `Group(PositionMixin, SizeMixin, AlphaMixin)` + rotation + scale
    Group {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        alpha: TopLevelExpr,
        rotation: TopLevelExpr,
        pivot_x: TopLevelExpr,
        pivot_y: TopLevelExpr,
        scale_x: TopLevelExpr,
        scale_y: TopLevelExpr,
        z_level: TopLevelExpr,
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
        z_level: TopLevelExpr,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Ellipse(PositionMixin, SizeMixin, StyleMixin)`
    Ellipse {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: TopLevelExpr,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Path(StyleMixin)`
    Path {
        #[serde(flatten)]
        style: Style,
        z_level: TopLevelExpr,
        #[serde(default)]
        children: Vec<NodeId>,
    },

    /// Python: `Text` — block of styled text with Line children
    Text {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        text_style: TextStyle,
        z_level: TopLevelExpr,
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
        text: TopLevelExpr,
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
        c1_x: TopLevelExpr,
        c1_y: TopLevelExpr,
        c2_x: TopLevelExpr,
        c2_y: TopLevelExpr,
    },

    /// An image (SVG for now).  `path` is resolved relative to the project directory.
    Image {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        z_level: TopLevelExpr,
        alpha: TopLevelExpr,
        path: TopLevelExpr,
        /// When true, scale the image to fit inside the node box while
        /// preserving the SVG's intrinsic aspect ratio (letterbox/pillarbox).
        keep_aspect: TopLevelExpr,
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
        z_level: TopLevelExpr,
        alpha: TopLevelExpr,
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

#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct SceneDef {
    pub id: NodeId,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(flatten)]
    pub size: Size,
    pub fill_color: TopLevelExpr,
    pub frames: u32,
    #[serde(default)]
    pub children: Vec<NodeId>,
}
