use crate::Color;
use serde::{Serialize, Serializer};
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Inheritable<T: Debug + Clone> {
    Own(T),
    Inherited(T),
}

impl<T: Debug + Clone + Serialize> Serialize for Inheritable<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Inheritable::Own(v) => v.serialize(serializer),
            Inheritable::Inherited(_) => {
                unreachable!("Inherited fields must be skipped via skip_serializing_if")
            }
        }
    }
}

impl<T: Debug + Clone> Inheritable<T> {
    pub fn is_inherited(&self) -> bool {
        matches!(self, Inheritable::Inherited(_))
    }

    pub fn value(&self) -> &T {
        match self {
            Inheritable::Own(v) | Inheritable::Inherited(v) => v,
        }
    }

    pub fn map<S: Debug + Clone, E>(
        &self,
        f: impl FnOnce(&T) -> Result<S, E>,
    ) -> Result<Inheritable<S>, E> {
        Ok(match self {
            Inheritable::Own(v) => Inheritable::Own(f(v)?),
            Inheritable::Inherited(v) => Inheritable::Inherited(f(v)?),
        })
    }
}

/// Mirrors PositionMixin in Python.
#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Mirrors SizeMixin in Python.
#[derive(Debug, Clone, Serialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// Mirrors StyleMixin (which extends AlphaMixin) in Python.
#[derive(Debug, Clone, Serialize)]
pub struct Style {
    pub fill_color: Option<Color>,
    pub stroke_color: Option<Color>,
    pub stroke_width: f64,
    pub alpha: f64,
}

/// A single styled text run.
#[derive(Debug, Clone, Serialize)]
pub struct TextSpan {
    pub id: u64,
    pub text: Arc<String>,
    #[serde(flatten)]
    pub style: Style,
    pub font_family: Arc<String>,
    pub font_size: f64,
    pub italic: bool,
}

/// A node in the text tree — either a nested group or a leaf span.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum TextChild {
    #[serde(rename = "t_group")]
    Group(TextGroup),
    #[serde(rename = "t_span")]
    Span(TextSpan),
}

/// A group of text children (other groups or spans).
#[derive(Debug, Clone, Serialize)]
pub struct TextGroup {
    pub id: u64,
    pub children: Vec<TextChild>,
}

/// A single command in a path. Mirrors PathMove / PathLine / PathCubic in Python.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PathCommand {
    Move {
        id: u64,
        #[serde(flatten)]
        position: Position,
    },
    Line {
        id: u64,
        #[serde(flatten)]
        position: Position,
    },
    Cubic {
        id: u64,
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
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NodeKind {
    /// Python: Group(PositionMixin, SizeMixin, AlphaMixin) + explicit scale and rotation
    Group {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        alpha: f64,
        scale_x: f64,
        scale_y: f64,
        rotation: f64,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
        children: Vec<Node>,
    },
    /// Python: Rect(PositionMixin, SizeMixin, StyleMixin)
    Rect {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        #[serde(flatten)]
        style: Style,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
    },
    /// Python: Ellipse(PositionMixin, SizeMixin, StyleMixin)
    Ellipse {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        #[serde(flatten)]
        style: Style,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
    },
    /// Python: Path(StyleMixin)
    Path {
        #[serde(flatten)]
        style: Style,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
        children: Vec<PathCommand>,
    },
    /// Python: Text — positioned block of text lines.
    /// Each element of `lines` is one line (rendered top-to-bottom).
    Text {
        #[serde(flatten)]
        position: Position,
        #[serde(rename = "children")]
        lines: Vec<TextChild>,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
    },
    /// An image node (SVG for now).
    Image {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        size: Size,
        alpha: f64,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
        path: Arc<String>,
        keep_aspect: bool,
    },
}

/// A node in the scene tree. Only carries the id;
/// all other data lives in the kind-specific variant.
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: u64,
    #[serde(flatten)]
    pub kind: NodeKind,
}

/// Root of a single frame. Mirrors Scene(SizeMixin) in Python.
#[derive(Debug, Clone, Serialize)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub fill_color: Color,
    pub children: Vec<Node>,
}
