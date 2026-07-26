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

/// A fill: solid color or linear gradient (proposal §9.11). Only used for
/// `fill_color` — `stroke_color` stays a plain `Color`.
#[derive(Debug, Clone)]
pub enum Paint {
    Solid(Color),
    LinearGradient {
        stops: Vec<(f64, Color)>,
        angle: f64,
    },
}

/// `Solid` serializes exactly like a bare `Color` (the same hex string) —
/// only consumed by the `/tree/{frame}` debug endpoint, but golden-image
/// tests snapshot that JSON, so a solid fill must stay byte-identical to
/// before this type existed. `LinearGradient` has no prior shape to match.
impl Serialize for Paint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Paint::Solid(c) => c.serialize(serializer),
            Paint::LinearGradient { stops, angle } => {
                use serde::ser::SerializeStruct;
                let mut s = serializer.serialize_struct("LinearGradient", 2)?;
                s.serialize_field("stops", stops)?;
                s.serialize_field("angle", angle)?;
                s.end()
            }
        }
    }
}

impl Paint {
    /// A gradient counts as transparent only when every stop is — a single
    /// visible stop still needs to draw.
    pub fn is_transparent(&self) -> bool {
        match self {
            Paint::Solid(c) => c.is_transparent(),
            Paint::LinearGradient { stops, .. } => stops.iter().all(|(_, c)| c.is_transparent()),
        }
    }

    /// The color to use where gradient fills aren't supported (text glyph
    /// fill) — degrades to the gradient's first stop as a solid color.
    pub fn solid_or_first_stop(&self) -> &Color {
        match self {
            Paint::Solid(c) => c,
            Paint::LinearGradient { stops, .. } => &stops[0].1,
        }
    }
}

/// Mirrors StyleMixin (which extends AlphaMixin) in Python.
#[derive(Debug, Clone, Serialize)]
pub struct Style {
    pub fill_color: Paint,
    pub stroke_color: Color,
    pub stroke_width: f64,
    pub alpha: f64,
    /// `(on, off)` pixel lengths. `None` means a solid (non-dashed) stroke.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash: Option<(f64, f64)>,
    pub dash_offset: f64,
}

/// Paragraph alignment for a `Text` block or a per-line `TextGroup` override.
/// Mirrors `TextAlignMode` in Python (api-v2-proposal.md §4.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Text styling that may be inherited from a parent node.
/// Fields are omitted from serialization when they are inherited (not overridden).
#[derive(Debug, Clone, Serialize)]
pub struct TextStyle {
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub fill_color: Inheritable<Paint>,
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
    /// `underline()`/`strike()` 0..1 progress (proposal §10.2) — inherited
    /// exactly like `italic`.
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub underline: Inheritable<f64>,
    #[serde(skip_serializing_if = "Inheritable::is_inherited")]
    pub strike: Inheritable<f64>,
}

/// A single styled text run.
#[derive(Debug, Clone, Serialize)]
pub struct TextSpan {
    pub id: u64,
    pub text: Arc<String>,
    #[serde(flatten)]
    pub text_style: TextStyle,
    /// Placeable-run position-override delta `(dx, dy)`, resolved against the
    /// nearest self-or-ancestor `TextGroup`/`TextSpan` with an explicit
    /// `x`/`y` — in the same final (fit-scaled) coordinate space `.at()`
    /// queries use. `None` for the common case (no position override
    /// anywhere in this span's ancestor chain); renderers convert to raw
    /// glyph-space via the block's own fit-scale before composing with
    /// `override_transform` below.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_offset: Option<(f32, f32)>,
    /// Placeable-run rotate/scale/pivot override, resolved against the
    /// nearest self-or-ancestor with any of `rotation`/`scale_*`/`pivot_*`
    /// explicitly set (independently of `override_offset` above — see
    /// `nearest_run_transform_component`, `layout.rs`). Already in raw,
    /// pre-fit-scale glyph space (rotation/scale are resolution-independent
    /// ratios, so — unlike `override_offset` — this needs no renderer-side
    /// conversion). `None` for the common case (no transform override
    /// anywhere in this span's ancestor chain).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_transform: Option<crate::AffineTransform>,
    /// `underline()`/`strike()` styling overrides (proposal §10.2) — sparse,
    /// per-node only (not inherited, unlike the progress fields on
    /// `TextStyle` above). Absent means "use the run's own resolved fill"
    /// (`color`) or "use the font's own underline/strikeout metrics at this
    /// span's resolved size" (`width`/`offset`), both computed by the
    /// renderer at draw time, not here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<Paint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike_color: Option<Paint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike_offset: Option<f64>,
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
    /// Per-line alignment override; `None` inherits the owning `Text` block's
    /// own `text_align`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<TextAlign>,
    /// See `TextSpan`'s identically-named fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<Paint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike_color: Option<Paint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike_offset: Option<f64>,
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

fn is_fully_revealed(reveal: &f64) -> bool {
    *reveal >= 1.0
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
        /// Corner radius in px, clamped to `min(w, h) / 2` at render time. `0.0` = square corners.
        radius: f64,
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
    /// Python: Text(PositionMixin, SizeMixin, RotAndScaleMixin) — positioned,
    /// rotatable/scalable block of text lines. Each element of `lines` is one
    /// line (rendered top-to-bottom). `node_box.size` is the resolved box the
    /// laid-out lines are scaled to fit (equals the natural measured extent
    /// when never explicitly set); `node_box.rotation`/`scale_*`/`pivot_*`
    /// rotate/scale/pivot the whole resolved box, same as `Rect`/`Image`.
    Text {
        #[serde(flatten)]
        node_box: NodeBox,
        keep_aspect: bool,
        /// Maximum line width, in unscaled layout units, before wrapping.
        /// `None` disables wrapping (a paragraph is exactly one visual row
        /// per logical line, as before this field existed).
        #[serde(skip_serializing_if = "Option::is_none")]
        wrap: Option<f64>,
        /// Resolved block-default alignment (never absent — `Left` when
        /// `Text.text_align()` was never called).
        text_align: TextAlign,
        #[serde(flatten)]
        text_style: TextStyle,
        #[serde(skip_serializing_if = "Option::is_none")]
        sh_language: Option<Arc<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sh_theme: Option<Arc<String>>,
        /// Typewriter-reveal fraction (proposal §9.6): `1.0` shows every
        /// glyph; renderers apply a per-glyph cutoff based on this value.
        /// Sparse like `wrap`/`sh_language` above — omitted at the default so
        /// existing golden-image snapshots of the `/tree/{frame}` debug JSON
        /// (predating this field) still match byte-for-byte.
        #[serde(skip_serializing_if = "is_fully_revealed")]
        reveal: f64,
        /// See `TextSpan`'s identically-named fields — the block-level
        /// default when no run overrides it.
        #[serde(skip_serializing_if = "Option::is_none")]
        underline_color: Option<Paint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        underline_width: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        underline_offset: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        strike_color: Option<Paint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        strike_width: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        strike_offset: Option<f64>,
        #[serde(rename = "children")]
        lines: Vec<TextChild>,
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
