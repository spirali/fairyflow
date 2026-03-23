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

    pub fn into_str(self) -> anyhow::Result<String> {
        match self {
            Value::Str(s) => Ok((*s).clone()),
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
    Hold { av: AvRef },
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
    pub x: AvId,
    pub y: AvId,
}

impl Position {
    pub fn check_attributes<F>(&self, f: &mut F) -> anyhow::Result<()> where F: FnMut(AvId) -> anyhow::Result<()> {
        let Position { x, y } = self;
        f(*x)?;
        f(*x)?;
        Ok(())
    }
}

/// Mirrors `SizeMixin` in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Size {
    pub width: AvId,
    pub height: AvId,
}

impl Size {
    pub fn check_attributes<F>(&self, f: &mut F) -> anyhow::Result<()> where F: FnMut(AvId) -> anyhow::Result<()> {
        let Size { width, height } = self;
        f(*width)?;
        f(*height)?;
        Ok(())
    }
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct Style {
    pub fill_color: AvId,
    pub stroke_color: AvId,
    pub stroke_width: AvId,
    pub alpha: AvId,
}

impl Style {
    pub fn check_attributes<F>(&self, f: &mut F) -> anyhow::Result<()> where F: FnMut(AvId) -> anyhow::Result<()> {
        let Style { fill_color, stroke_color, stroke_width, alpha } = self;
        f(*fill_color)?;
        f(*stroke_color)?;
        f(*stroke_width)?;
        f(*alpha)?;
        Ok(())
    }
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python.
#[derive(Debug, Clone, Deserialize)]
pub struct TextStyle {
    #[serde(flatten)]
    pub style: Style,
    font: AvId,
    italic: AvId,
}

impl TextStyle {
    pub fn font(&self) -> AvId { self.font }
    pub fn italic(&self) -> AvId { self.italic }

    pub fn check_attributes<F>(&self, f: &mut F) -> anyhow::Result<()> where F: FnMut(AvId) -> anyhow::Result<()> {
        self.style.check_attributes(f)?;
        f(self.font)?;
        f(self.italic)?;
        Ok(())
    }
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
        alpha: AvId,
        rotation: AvId,
        scale_x: AvId,
        scale_y: AvId,
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
        text: AvId,
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
        c1_x: AvId,
        c1_y: AvId,
        c2_x: AvId,
        c2_y: AvId,
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

    pub fn check_attributes<F>(&self, f: &mut F) -> anyhow::Result<()> where F: FnMut(AvId) -> anyhow::Result<()> {
        match self {
            NodeKind::Group { position, size, alpha, rotation, scale_x, scale_y, children: _ } => {
                position.check_attributes(f)?;
                size.check_attributes(f)?;
                f(*alpha)?;
                f(*rotation)?;
                f(*scale_x)?;
                f(*scale_y)?;
            }
            NodeKind::Rect { position, size, style } | NodeKind::Ellipse { position, size, style } => {
                position.check_attributes(f)?;
                size.check_attributes(f)?;
                style.check_attributes(f)?;
            }
            NodeKind::Path { style, children: _ } => {
                style.check_attributes(f)?;
            }
            NodeKind::Text { position, text_style, children: _ } => {
                position.check_attributes(f)?;
                text_style.check_attributes(f)?;
            }
            NodeKind::TextLine { text_style, children: _ } => {
                text_style.check_attributes(f)?;
            }
            NodeKind::TextSpan { text_style, text } => {
                text_style.check_attributes(f)?;
                f(*text)?;
            }
            NodeKind::Move { position } => position.check_attributes(f)?,
            NodeKind::Line { position } => position.check_attributes(f)?,
            NodeKind::Cubic { position, c1_x, c1_y, c2_x, c2_y } => {
                position.check_attributes(f)?;
                f(*c1_x)?;
                f(*c1_y)?;
                f(*c2_x)?;
                f(*c2_y)?;
            }
        }
        Ok(())
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
    pub fill_color: AvId,
    #[serde(default)]
    pub children: Vec<NodeId>,
}