use crate::basictypes::{FrameId, NodeId};
use crate::eval::EvalCtx;
use crate::values::{Color, Expr, Value};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::Arc;

// ──────────────────────────── Attribute wrapper ────────────────────────────

/// A possibly-absent animatable attribute. Absence means "use the kind/field's
/// default" (auto-layout position/size, inherited-from-parent style/z, or a
/// literal constant) — resolved at eval time (`eval.rs`), not here. Plain
/// internal type: `NodeDef` (below) is the only thing that's actually
/// deserialized from JSON; `AttrExpr`/`NodeKind`/the mixins are built from it
/// by `Node::from_def`.
#[derive(Debug)]
pub struct AttrExpr<T: Value + DeserializeOwned>(pub Option<Expr<T>>);

impl<T: Value + DeserializeOwned> AttrExpr<T> {
    #[inline]
    pub fn get_expr(&self) -> Option<&Expr<T>> {
        self.0.as_ref()
    }
}

impl<T: Value + DeserializeOwned> From<Option<Expr<T>>> for AttrExpr<T> {
    fn from(v: Option<Expr<T>>) -> Self {
        AttrExpr(v)
    }
}

// ──────────────────────────────── Mixins ───────────────────────────────────

/// Mirrors `PositionMixin` in Python.
#[derive(Debug)]
pub struct Position {
    pub x: AttrExpr<f64>,
    pub y: AttrExpr<f64>,
}

/// Mirrors `SizeMixin` in Python.
#[derive(Debug)]
pub struct Size {
    pub width: AttrExpr<f64>,
    pub height: AttrExpr<f64>,
}

/// Mirrors `StyleMixin` (which extends `AlphaMixin`) in Python. Own (non-inherited)
/// literal defaults — used by shape nodes (rect/ellipse/path).
#[derive(Debug)]
pub struct Style {
    pub fill_color: AttrExpr<Color>,
    pub stroke_color: AttrExpr<Color>,
    pub stroke_width: AttrExpr<f64>,
    pub alpha: AttrExpr<f64>,
}

/// Mirrors `TextStyleMixin` in Python. Unlike `Style`, these fields are
/// inherited-from-ancestor when absent (see `eval.rs::eval_inherited`).
#[derive(Debug)]
pub struct TextStyle {
    pub fill_color: AttrExpr<Color>,
    pub stroke_color: AttrExpr<Color>,
    pub stroke_width: AttrExpr<f64>,
    pub alpha: AttrExpr<f64>,
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
        gap: Expr<f64>,
        align: Expr<f64>,
        reserve: bool,
    },
    Row {
        gap: Expr<f64>,
        align: Expr<f64>,
        reserve: bool,
    },
}

#[derive(Debug)]
pub enum NodeKind {
    /// Python: `Group(PositionMixin, SizeMixin, AlphaMixin)` + rotation + scale
    Group {
        position: Position,
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
        children: Vec<NodeId>,
    },
    /// Python: `Rect(PositionMixin, SizeMixin, StyleMixin)`
    Rect {
        position: Position,
        size: Size,
        z_level: AttrExpr<f64>,
        style: Style,
    },
    /// Python: `Ellipse(PositionMixin, SizeMixin, StyleMixin)`
    Ellipse {
        position: Position,
        size: Size,
        z_level: AttrExpr<f64>,
        style: Style,
    },
    /// Python: `Path(StyleMixin)`
    Path {
        style: Style,
        z_level: AttrExpr<f64>,
        crop_start: AttrExpr<f64>,
        crop_end: AttrExpr<f64>,
        children: Vec<NodeId>,
    },

    /// Python: `Text` — block of styled text with Line children
    Text {
        position: Position,
        text_style: TextStyle,
        z_level: AttrExpr<f64>,
        sh_language: Option<Arc<String>>,
        sh_theme: Option<Arc<String>>,
        children: Vec<NodeId>,
    },
    /// Group containing instance of other TextGroups or TextSpans.
    TextGroup {
        text_style: TextStyle,
        children: Vec<NodeId>,
    },
    /// A text run with a concrete string value
    TextSpan {
        text_style: TextStyle,
        text: AttrExpr<Arc<String>>,
    },

    /// Path commands; They always have Path as parent
    Move {
        position: Position,
    },
    Line {
        position: Position,
    },
    Cubic {
        position: Position,
        c1_x: AttrExpr<f64>,
        c1_y: AttrExpr<f64>,
        c2_x: AttrExpr<f64>,
        c2_y: AttrExpr<f64>,
    },
    Close,

    /// An image (SVG for now).  `path` is resolved relative to the project directory.
    Image {
        position: Position,
        size: Size,
        z_level: AttrExpr<f64>,
        alpha: AttrExpr<f64>,
        path: AttrExpr<Arc<String>>,
        /// When true, scale the image to fit inside the node box while
        /// preserving the SVG's intrinsic aspect ratio (letterbox/pillarbox).
        keep_aspect: AttrExpr<bool>,
        /// Optional child layer nodes (kind = "layer").
        children: Vec<NodeId>,
    },

    /// A layer within an SVG image.  Always a child of an Image node.
    Layer {
        position: Position,
        size: Size,
        z_level: AttrExpr<f64>,
        alpha: AttrExpr<f64>,
        /// Matches the SVG group `id` attribute.
        layer_name: Arc<String>,
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

#[derive(Debug)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub start: FrameId,
    pub end: Option<FrameId>,
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
        if let NodeKind::Image { path, .. } = &self.kind
            && let Some(expr) = path.get_expr()
        {
            expr.collect_strings(image_paths);
        }
    }
}

/// Root of the scene definition. Mirrors `Scene(SizeMixin)` in Python.
#[derive(Debug)]
pub(crate) struct SceneDef {
    pub size: Size,
    pub fill_color: AttrExpr<Color>,
    pub frames: u32,
    pub cues: Vec<u32>,
    pub children: Vec<NodeId>,
}

// ──────────────────────── Wire format: NodeDef ─────────────────────────────

#[derive(Debug, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    #[default]
    Group,
    Rect,
    Ellipse,
    Path,
    Text,
    #[serde(rename = "t_group")]
    TextGroup,
    #[serde(rename = "t_span")]
    TextSpan,
    #[serde(rename = "move")]
    Move,
    Line,
    Cubic,
    Close,
    Image,
    Layer,
}

/// The one sparse struct actually deserialized from JSON (`api-v2-impl.md` §A.5/§A.7):
/// no `#[serde(flatten)]`, no `#[serde(tag = ...)]` payload dispatch — `kind` is an
/// ordinary fieldless-enum field, and every attribute is `Option`. `Node::from_def`
/// (below) converts this into the internal `NodeKind` enum used by `eval.rs`/`layout.rs`.
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub(crate) struct NodeDef {
    pub kind: Kind,
    pub start: Option<FrameId>,
    pub end: Option<FrameId>,

    pub x: Option<Expr<f64>>,
    pub y: Option<Expr<f64>>,
    pub w: Option<Expr<f64>>,
    pub h: Option<Expr<f64>>,
    pub alpha: Option<Expr<f64>>,
    pub rotation: Option<Expr<f64>>,
    pub pivot_x: Option<Expr<f64>>,
    pub pivot_y: Option<Expr<f64>>,
    pub scale_x: Option<Expr<f64>>,
    pub scale_y: Option<Expr<f64>>,
    pub z: Option<Expr<f64>>,
    pub clip_x: Option<Expr<f64>>,
    pub clip_y: Option<Expr<f64>>,
    pub clip_w: Option<Expr<f64>>,
    pub clip_h: Option<Expr<f64>>,

    pub fill: Option<Expr<Color>>,
    pub stroke: Option<Expr<Color>>,
    pub stroke_width: Option<Expr<f64>>,

    pub crop_start: Option<Expr<f64>>,
    pub crop_end: Option<Expr<f64>>,

    pub c1_x: Option<Expr<f64>>,
    pub c1_y: Option<Expr<f64>>,
    pub c2_x: Option<Expr<f64>>,
    pub c2_y: Option<Expr<f64>>,

    pub font: Option<Expr<Arc<String>>>,
    pub font_size: Option<Expr<f64>>,
    pub font_weight: Option<Expr<f64>>,
    pub italic: Option<Expr<bool>>,
    pub text: Option<Expr<Arc<String>>>,
    pub sh_language: Option<Arc<String>>,
    pub sh_theme: Option<Arc<String>>,

    pub file: Option<Expr<Arc<String>>>,
    pub keep_aspect: Option<Expr<bool>>,
    pub layer_name: Option<Arc<String>>,

    pub layout: Option<Layout>,
    pub children: Vec<NodeId>,

    /// Debug-only Python call-site info (currently `{"stack": [...]}`); present
    /// only when the scene was generated with `--debug` (`info.py::get_info`),
    /// and omitted entirely otherwise. The engine treats this as an opaque
    /// blob — it never inspects or validates its shape, just carries it
    /// through to `animdef.rs`, which tags it with the node's wire id and
    /// forwards it to the frontend (`SceneInfo`/`SceneInfoMsg`) untouched.
    pub info: Option<serde_json::Value>,
}

impl NodeDef {
    fn position(&mut self) -> Position {
        Position {
            x: AttrExpr(self.x.take()),
            y: AttrExpr(self.y.take()),
        }
    }

    fn size(&mut self) -> Size {
        Size {
            width: AttrExpr(self.w.take()),
            height: AttrExpr(self.h.take()),
        }
    }

    fn style(&mut self) -> Style {
        Style {
            fill_color: AttrExpr(self.fill.take()),
            stroke_color: AttrExpr(self.stroke.take()),
            stroke_width: AttrExpr(self.stroke_width.take()),
            alpha: AttrExpr(self.alpha.take()),
        }
    }

    fn text_style(&mut self) -> TextStyle {
        TextStyle {
            fill_color: AttrExpr(self.fill.take()),
            stroke_color: AttrExpr(self.stroke.take()),
            stroke_width: AttrExpr(self.stroke_width.take()),
            alpha: AttrExpr(self.alpha.take()),
            font: AttrExpr(self.font.take()),
            font_size: AttrExpr(self.font_size.take()),
            font_weight: AttrExpr(self.font_weight.take()),
            italic: AttrExpr(self.italic.take()),
        }
    }

    fn z_level(&mut self) -> AttrExpr<f64> {
        AttrExpr(self.z.take())
    }
}

impl Node {
    /// Convert the flat wire-format `NodeDef` (array index `idx`) into the internal
    /// `NodeKind` enum used everywhere else in the engine. Kind-specific field
    /// validity (e.g. a `rect` shouldn't carry `children`) is not checked — the
    /// Python serializer is the sole producer of this format.
    pub(crate) fn from_def(idx: u32, mut def: NodeDef) -> anyhow::Result<Node> {
        let id = NodeId::new(idx);
        let children = std::mem::take(&mut def.children);
        let kind = match def.kind {
            Kind::Group => {
                let position = def.position();
                let size = def.size();
                let z_level = def.z_level();
                NodeKind::Group {
                    position,
                    size,
                    alpha: AttrExpr(def.alpha.take()),
                    rotation: AttrExpr(def.rotation.take()),
                    pivot_x: AttrExpr(def.pivot_x.take()),
                    pivot_y: AttrExpr(def.pivot_y.take()),
                    scale_x: AttrExpr(def.scale_x.take()),
                    scale_y: AttrExpr(def.scale_y.take()),
                    z_level,
                    clip_x: AttrExpr(def.clip_x.take()),
                    clip_y: AttrExpr(def.clip_y.take()),
                    clip_w: AttrExpr(def.clip_w.take()),
                    clip_h: AttrExpr(def.clip_h.take()),
                    layout: def
                        .layout
                        .take()
                        .ok_or_else(|| anyhow::anyhow!("group node {} missing `layout`", idx))?,
                    children,
                }
            }
            Kind::Rect => {
                let position = def.position();
                let size = def.size();
                let z_level = def.z_level();
                NodeKind::Rect {
                    position,
                    size,
                    z_level,
                    style: def.style(),
                }
            }
            Kind::Ellipse => {
                let position = def.position();
                let size = def.size();
                let z_level = def.z_level();
                NodeKind::Ellipse {
                    position,
                    size,
                    z_level,
                    style: def.style(),
                }
            }
            Kind::Path => {
                let z_level = def.z_level();
                NodeKind::Path {
                    style: def.style(),
                    z_level,
                    crop_start: AttrExpr(def.crop_start.take()),
                    crop_end: AttrExpr(def.crop_end.take()),
                    children,
                }
            }
            Kind::Text => {
                let position = def.position();
                let z_level = def.z_level();
                NodeKind::Text {
                    position,
                    text_style: def.text_style(),
                    z_level,
                    sh_language: def.sh_language.take(),
                    sh_theme: def.sh_theme.take(),
                    children,
                }
            }
            Kind::TextGroup => NodeKind::TextGroup {
                text_style: def.text_style(),
                children,
            },
            Kind::TextSpan => NodeKind::TextSpan {
                text_style: def.text_style(),
                text: AttrExpr(def.text.take()),
            },
            Kind::Move => NodeKind::Move {
                position: def.position(),
            },
            Kind::Line => NodeKind::Line {
                position: def.position(),
            },
            Kind::Cubic => {
                let position = def.position();
                NodeKind::Cubic {
                    position,
                    c1_x: AttrExpr(def.c1_x.take()),
                    c1_y: AttrExpr(def.c1_y.take()),
                    c2_x: AttrExpr(def.c2_x.take()),
                    c2_y: AttrExpr(def.c2_y.take()),
                }
            }
            Kind::Close => NodeKind::Close,
            Kind::Image => {
                let position = def.position();
                let size = def.size();
                let z_level = def.z_level();
                NodeKind::Image {
                    position,
                    size,
                    z_level,
                    alpha: AttrExpr(def.alpha.take()),
                    path: AttrExpr(def.file.take()),
                    keep_aspect: AttrExpr(def.keep_aspect.take()),
                    children,
                }
            }
            Kind::Layer => {
                let position = def.position();
                let size = def.size();
                let z_level = def.z_level();
                NodeKind::Layer {
                    position,
                    size,
                    z_level,
                    alpha: AttrExpr(def.alpha.take()),
                    layer_name: def.layer_name.take().ok_or_else(|| {
                        anyhow::anyhow!("layer node {} missing `layer_name`", idx)
                    })?,
                    children,
                }
            }
        };
        Ok(Node {
            id,
            parent: None,
            start: def.start.unwrap_or_default(),
            end: def.end,
            kind,
        })
    }
}
