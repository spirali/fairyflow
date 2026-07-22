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

    pub fn map<S: Debug + Clone>(&self, f: impl FnOnce(&T) -> S) -> Inheritable<S> {
        match self {
            Inheritable::Own(v) => Inheritable::Own(f(v)),
            Inheritable::Inherited(v) => Inheritable::Inherited(f(v)),
        }
    }
}

/// Mirrors PositionMixin in Python.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Mirrors SizeMixin in Python.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeBox {
    #[serde(flatten)]
    pub position: Position,
    #[serde(flatten)]
    pub size: Size,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub z_level: Inheritable<f64>,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation: f64,
    pub pivot_x: f64,
    pub pivot_y: f64,
}

/// Mirrors StyleMixin (which extends AlphaMixin) in Python.
#[derive(Debug, Clone, Serialize)]
pub struct Style {
    pub fill_color: Color,
    pub stroke_color: Color,
    pub stroke_width: f64,
    pub alpha: f64,
}

/// Text styling that may be inherited from a parent node.
/// Fields are omitted from serialization when they are inherited (not overridden).
#[derive(Debug, Clone, Serialize)]
pub struct TextStyle {
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub fill_color: Inheritable<Color>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub stroke_color: Inheritable<Color>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub stroke_width: Inheritable<f64>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub alpha: Inheritable<f64>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub font_family: Inheritable<Arc<String>>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub font_size: Inheritable<f64>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub font_weight: Inheritable<f64>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub italic: Inheritable<bool>,
}

/// A single styled text run.
#[derive(Debug, Clone, Serialize)]
pub struct TextSpan {
    pub id: u64,
    pub text: Arc<String>,
    #[serde(flatten)]
    pub text_style: TextStyle,
}

/// A node in the text tree — either a nested group or a leaf span.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum TextChild {
    #[serde(rename = "tline")]
    Group(TextGroup),
    #[serde(rename = "tspan")]
    Span(TextSpan),
}

/// A group of text children (other groups or spans).
#[derive(Debug, Clone, Serialize)]
pub struct TextGroup {
    pub id: u64,
    #[serde(flatten)]
    pub text_style: TextStyle,
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
    Close {
        id: u64,
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
        node_box: NodeBox,
        alpha: f64,
        /// Relative clip region. Values outside [0,1] mean no clipping on that axis.
        clip_x: f64,
        clip_y: f64,
        clip_w: f64,
        clip_h: f64,
        /// True iff `.clip()` was explicitly called (any axis, even at the
        /// literal default 0,0,1,1) - distinguishes "explicitly clipped to
        /// the full box" from "never called `.clip()`", which the numeric
        /// fields alone can't (both evaluate to the same 0,0,1,1). Drives the
        /// `needs_clip` fast-path in both renderers.
        clip_enabled: bool,
        #[serde(flatten)]
        camera: Camera,
        children: Vec<Node>,
    },
    /// Python: Rect(PositionMixin, SizeMixin, StyleMixin) + rotation + scale
    Rect {
        #[serde(flatten)]
        node_box: NodeBox,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: Ellipse(PositionMixin, SizeMixin, StyleMixin) + rotation + scale
    Ellipse {
        #[serde(flatten)]
        node_box: NodeBox,
        #[serde(flatten)]
        style: Style,
    },
    /// Python: Path(StyleMixin) + rotation + scale.
    /// No position/size: a path has no bounds of its own (points are absolute
    /// path-command coordinates), so pivot_x/pivot_y are always inert (rotation
    /// happens around the local origin) until path bounds computation lands.
    Path {
        #[serde(flatten)]
        style: Style,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
        crop_start: f64,
        crop_end: f64,
        children: Vec<PathCommand>,
    },
    /// Python: Text — positioned block of text lines.
    /// Each element of `lines` is one line (rendered top-to-bottom).
    Text {
        #[serde(flatten)]
        position: Position,
        #[serde(flatten)]
        text_style: TextStyle,
        #[serde(skip_serializing_if = "Option::is_none")]
        sh_language: Option<Arc<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sh_theme: Option<Arc<String>>,
        #[serde(rename = "children")]
        lines: Vec<TextChild>,
        #[serde(skip_serializing_if = "Inheritable::is_inherited")]
        z_level: Inheritable<f64>,
    },
    /// An image node (SVG for now).
    Image {
        #[serde(flatten)]
        node_box: NodeBox,
        alpha: f64,
        path: Arc<String>,
        keep_aspect: bool,
        /// Active per-layer overrides.  Empty when no layers are specified.
        #[serde(default)]
        layers: Vec<ImageLayer>,
        /// Layer names that have a Layer node but are inactive this frame.
        /// These SVG layers must be hidden even though they have no active override.
        #[serde(default)]
        hidden_layers: Vec<Arc<String>>,
        /// All `inkscape:label` layer names present in the SVG, in document order.
        /// Used by the web UI to display implicit (unnamed) layers in the tree.
        #[serde(default)]
        all_svg_layers: Arc<Vec<String>>,
    },
}

/// A layer override for an SVG image node.
#[derive(Debug, Clone, Serialize)]
pub struct ImageLayer {
    pub id: u64,
    /// Identifies the SVG group by its `id` attribute.
    pub layer_name: Arc<String>,
    #[serde(flatten)]
    pub node_box: NodeBox,
    pub alpha: f64,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub z_level: Inheritable<f64>,
}

/// A node in the scene tree. Only carries the id;
/// all other data lives in the kind-specific variant.
#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: u64,
    #[serde(flatten)]
    pub kind: NodeKind,
}

/// Content-space viewpoint (api-v2-proposal §4.7/§4.8): `camera_zoom = 1.0`
/// with `camera_x/y` at the box's own center is the identity camera.
/// Applied only when recursing into a container's children — never folded
/// into `NodeBox`'s own transform, so it never affects layout, world-
/// bounds, or hit-testing. See `transform::camera_transform`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Camera {
    pub camera_zoom: f64,
    pub camera_x: f64,
    pub camera_y: f64,
}

/// Root of a single frame. Mirrors Scene(SizeMixin) in Python.
#[derive(Debug, Clone, Serialize)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub fill_color: Color,
    #[serde(flatten)]
    pub camera: Camera,
    pub children: Vec<Node>,
}
