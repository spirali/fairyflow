use std::sync::Arc;
use renderer::Color as RendererColor;
use serde::{de, Deserialize, Deserializer};
use crate::basictypes::{AvId, FrameId, NodeId};

// ──────────────────────────── Transition ───────────────────────────────────

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    Sharp,
    Linear,
}


#[derive(Debug, Clone)]
pub struct Color(RendererColor);

impl Color {
    pub fn into_inner(self) -> RendererColor {
        self.0
    }

    pub fn interpolate(a: &Color, b: &Color, t: f64) -> Color {
        Color(a.0.interpolate(&b.0, t))
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Helper { color: String }
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
}

impl Value {
    pub fn as_f64(&self) -> anyhow::Result<f64> {
        match  self {
            Value::Int(v) => Ok(*v as f64),
            Value::Float(v) => Ok(*v),
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
        struct Helper { av: u64 }
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
        struct Helper { id: u32 }
        let h = Helper::deserialize(d)?;
        Ok(NodeRef(NodeId::new(h.id)))
    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct CallParamsPair {
    pub a: Expr,
    pub b: Expr
}

#[derive(Debug, Clone, Deserialize)]
pub struct CallParamsNodeTransform  { pub source: NodeRef, pub target: NodeRef, pub x: Expr, pub y: Expr }


#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "fn", rename_all = "snake_case")]
pub enum CallExpr {
    NodeTransformX(Box<CallParamsNodeTransform>),
    NodeTransformY(Box<CallParamsNodeTransform>),
    #[serde(rename="+")]
    Add(Box<CallParamsPair>),
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
}

impl Expr {
    pub fn check_refs(&self) -> anyhow::Result<()> {
        todo!()
    }
}

// ──────────────────────────────── Mixins ───────────────────────────────────

/// Mirrors `PositionMixin` in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub x: Expr,
    pub y: Expr,
}

/// Mirrors `SizeMixin` in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Size {
    pub width: Expr,
    pub height: Expr,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Style {
    pub fill_color: Expr,
    pub stroke_color: Expr,
    pub stroke_width: Expr,
    pub alpha: Expr,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct TextStyle {
    #[serde(flatten)]
    pub style: Style,
    pub font: Expr,
    pub italic: Expr,
}

// ──────────────────────────── Path commands ─────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// Python: `Group(PositionMixin, SizeMixin, AlphaMixin)` + rotation + scale
    Group {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        alpha: Expr,
        rotation: Expr,
        scale_x: Expr,
        scale_y: Expr,
        #[serde(default)]
        children: Vec<NodeId>,
    },
    /// Python: `Rect(PositionMixin, SizeMixin, StyleMixin)`
    Rect {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Ellipse(PositionMixin, SizeMixin, StyleMixin)`
    Ellipse {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: `Path(StyleMixin)`
    Path {
        #[serde(flatten)]
        style: Style,
        #[serde(default)]
        children: Vec<NodeId>,
    },

    /// Python: `Text` — block of styled text with Line children
    Text {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        text_style: TextStyle,
        #[serde(default)]
        children: Vec<NodeId>,
    },
    /// A line of text within a Text node, containing Span children
    #[serde(rename="t_line")]
    TextLine {
        #[serde(flatten)]
        text_style: TextStyle,
        #[serde(default)]
        children: Vec<NodeId>,
    },
    /// A text run with a concrete string value
    #[serde(rename="t_span")]
    TextSpan {
        #[serde(flatten)]
        text_style: TextStyle,
        text: Expr,
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
        c1_x: Expr,
        c1_y: Expr,
        c2_x: Expr,
        c2_y: Expr,
    },
}


impl NodeKind {
    pub fn children(&self) -> &[NodeId] {
        match self {
            NodeKind::Group { children, .. } |
            NodeKind::Path { children, .. } |
            NodeKind::Text { children, .. } |
            NodeKind::TextLine { children, .. } => children,
            _ => &[],
        }
    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub id: NodeId,
    #[serde(skip)]
    pub parent: Option<NodeId>,
    #[serde(flatten)]
    pub kind: NodeKind,
}

/// Root of the scene definition. Mirrors `Scene(SizeMixin)` in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct SceneDef {
    pub id: NodeId,
    #[serde(flatten)]
    pub size: Size,
    pub fill_color: Expr,
    #[serde(default)]
    pub children: Vec<NodeId>,
}