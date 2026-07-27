use crate::basictypes::{FrameId, NodeId};
use crate::eval::EvalCtx;
use crate::values::{Color, Expr, Paint, Value};
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

/// Mirrors `PositionMixin` in Python.
#[derive(Debug)]
pub struct NodeBox {
    pub position: Position,
    pub size: Size,
    pub z_level: AttrExpr<f64>,
    pub rotation: AttrExpr<f64>,
    pub pivot_x: AttrExpr<f64>,
    pub pivot_y: AttrExpr<f64>,
    pub scale_x: AttrExpr<f64>,
    pub scale_y: AttrExpr<f64>,
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
    pub fill_color: AttrExpr<Paint>,
    pub stroke_color: AttrExpr<Color>,
    pub stroke_width: AttrExpr<f64>,
    pub alpha: AttrExpr<f64>,
    /// Both set together from the Python `stroke(dash=(on, off))` call; only
    /// their joint presence (not `Value`-level animation) signals a dashed
    /// stroke, so both stay ordinary `Expr<f64>` — no new `Value` impl needed.
    pub dash_on: AttrExpr<f64>,
    pub dash_off: AttrExpr<f64>,
    pub dash_offset: AttrExpr<f64>,
}

/// Mirrors `TextStyleMixin` in Python. Unlike `Style`, these fields are
/// inherited-from-ancestor when absent (see `eval.rs::eval_inherited`).
#[derive(Debug)]
pub struct TextStyle {
    pub fill_color: AttrExpr<Paint>,
    pub stroke_color: AttrExpr<Color>,
    pub stroke_width: AttrExpr<f64>,
    pub alpha: AttrExpr<f64>,
    pub font: AttrExpr<Arc<String>>,
    pub font_size: AttrExpr<f64>,
    pub font_weight: AttrExpr<f64>,
    pub italic: AttrExpr<bool>,
    /// `underline()`/`strike()` 0..1 progress — inherited
    /// exactly like `italic`, root default `0.0`.
    pub underline: AttrExpr<f64>,
    pub strike: AttrExpr<f64>,
}

/// Non-inherited per-node override for `underline()`/`strike()` styling
/// (`color=`/`width=`/`offset=`). Unlike `TextStyle`, there is no
/// ancestor-walk and no literal root default here — `color` defaults to the
/// run's own resolved fill and `width`/`offset` default to the font's own
/// underline/strikeout metrics, both resolved at render time from whichever
/// concrete value is already in scope there (renderer-core/renderer-skia/
/// renderer-pdf), not by climbing the node tree.
#[derive(Debug)]
pub struct DecorationStyle {
    pub color: AttrExpr<Paint>,
    pub width: AttrExpr<f64>,
    pub offset: AttrExpr<f64>,
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
    Grid {
        cols: u32,
        gap_x: Expr<f64>,
        gap_y: Expr<f64>,
        reserve: bool,
    },
}

#[derive(Debug)]
pub struct Camera {
    pub zoom: AttrExpr<f64>,
    pub x: AttrExpr<f64>,
    pub y: AttrExpr<f64>,
}

#[derive(Debug)]
pub struct Padding {
    pub top: AttrExpr<f64>,
    pub right: AttrExpr<f64>,
    pub bottom: AttrExpr<f64>,
    pub left: AttrExpr<f64>,
}

#[derive(Debug)]
pub enum NodeKind {
    /// Python: `Group(PositionMixin, SizeMixin, AlphaMixin)` + rotation + scale
    Group {
        node_box: NodeBox,
        alpha: AttrExpr<f64>,
        clip_x: AttrExpr<f64>,
        clip_y: AttrExpr<f64>,
        clip_w: AttrExpr<f64>,
        clip_h: AttrExpr<f64>,
        padding: Padding,
        layout: Layout,
        camera: Camera,
        children: Vec<NodeId>,
    },
    /// Python: `Rect(PositionMixin, SizeMixin, StyleMixin)` + rotation + scale
    Rect {
        node_box: NodeBox,
        style: Style,
        radius: AttrExpr<f64>,
    },
    /// Python: `Ellipse(PositionMixin, SizeMixin, StyleMixin)` + rotation + scale
    Ellipse {
        node_box: NodeBox,
        style: Style,
    },
    /// Python: `Path(StyleMixin)` + rotation + scale
    Path {
        style: Style,
        z_level: AttrExpr<f64>,
        crop_start: AttrExpr<f64>,
        crop_end: AttrExpr<f64>,
        children: Vec<NodeId>,
    },

    /// Python: `Text(PositionMixin, SizeMixin, RotAndScaleMixin)` — block of styled
    /// text with Line children. `node_box.size` is the explicit/auto-derived box the
    /// laid-out text is scaled to fit (never re-lays-out text; that's `font(size=)`'s
    /// job); `node_box.rotation`/`scale_*`/`pivot_*` rotate/scale/pivot the whole
    /// resolved box, same as `Rect`/`Image`.
    Text {
        node_box: NodeBox,
        keep_aspect: AttrExpr<bool>,
        /// Maximum line width, in unscaled layout units, before wrapping.
        /// Absent (not `AttrExpr`-wrapped default handling) means wrapping is
        /// off — there's no "auto" fallback expression the way `width`/`height`
        /// have, so this is genuine absence rather than an unresolved default.
        wrap: AttrExpr<f64>,
        /// Block-default alignment; `None` resolves to `Left` at eval time.
        text_align: Option<renderer_core::TextAlign>,
        text_style: TextStyle,
        underline_style: DecorationStyle,
        strike_style: DecorationStyle,
        sh_language: Option<Arc<String>>,
        sh_theme: Option<Arc<String>>,
        /// Typewriter-reveal fraction — `1.0` (default) shows
        /// every glyph; the renderer applies the per-glyph cutoff.
        reveal: AttrExpr<f64>,
        children: Vec<NodeId>,
    },
    /// Group containing instance of other TextGroups or TextSpans.
    TextGroup {
        /// Reuses the same `NodeBox` shape as every other node — but only
        /// `position` (item 16's placeable-run override) and
        /// `rotation`/`scale_*`/`pivot_*` (per-run transform override) are
        /// ever consulted, by `nearest_position_override_delta`/
        /// `nearest_run_transform_component` (`layout.rs`). `size`/`z_level`
        /// are parsed (accepting `w`/`h`/`z` on the wire without erroring)
        /// but never read: a run's extent is always its measured glyphs
        /// (no settable size), and paragraph order is draw order (no z on
        /// text runs).
        node_box: NodeBox,
        text_style: TextStyle,
        underline_style: DecorationStyle,
        strike_style: DecorationStyle,
        /// Per-line alignment override; `None` inherits the owning `Text`
        /// block's own `text_align`.
        text_align: Option<renderer_core::TextAlign>,
        children: Vec<NodeId>,
    },
    /// A text run with a concrete string value
    TextSpan {
        /// See `TextGroup::node_box`.
        node_box: NodeBox,
        text_style: TextStyle,
        underline_style: DecorationStyle,
        strike_style: DecorationStyle,
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
    /// + rotation + scale
    Image {
        node_box: NodeBox,
        alpha: AttrExpr<f64>,
        path: AttrExpr<Arc<String>>,
        /// When true, scale the image to fit inside the node box while
        /// preserving the SVG's intrinsic aspect ratio (letterbox/pillarbox).
        keep_aspect: AttrExpr<bool>,
        /// Optional child layer nodes (kind = "layer").
        children: Vec<NodeId>,
    },

    /// A layer within an SVG image.  Always a child of an Image node. + rotation + scale
    Layer {
        node_box: NodeBox,
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

    pub fn node_box(&self) -> Option<&NodeBox> {
        match self {
            NodeKind::Group { node_box, .. }
            | NodeKind::Rect { node_box, .. }
            | NodeKind::Ellipse { node_box, .. }
            | NodeKind::Image { node_box, .. }
            | NodeKind::Layer { node_box, .. }
            | NodeKind::Text { node_box, .. } => Some(node_box),
            _ => None,
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
    pub camera: Camera,
    pub frames: u32,
    pub cues: Vec<u32>,
    pub flow: bool,
    pub notes: Vec<(u32, u32, String)>,
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
    #[serde(rename = "tline")]
    TextGroup,
    #[serde(rename = "tspan")]
    TextSpan,
    #[serde(rename = "move")]
    Move,
    Line,
    Cubic,
    Close,
    Image,
    Layer,
}

/// The one sparse struct actually deserialized from JSON:
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
    pub camera_zoom: Option<Expr<f64>>,
    pub camera_x: Option<Expr<f64>>,
    pub camera_y: Option<Expr<f64>>,
    pub padding_top: Option<Expr<f64>>,
    pub padding_right: Option<Expr<f64>>,
    pub padding_bottom: Option<Expr<f64>>,
    pub padding_left: Option<Expr<f64>>,

    pub fill: Option<Expr<Paint>>,
    pub stroke: Option<Expr<Color>>,
    pub stroke_width: Option<Expr<f64>>,
    pub dash_on: Option<Expr<f64>>,
    pub dash_off: Option<Expr<f64>>,
    pub dash_offset: Option<Expr<f64>>,

    pub radius: Option<Expr<f64>>,

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
    pub underline: Option<Expr<f64>>,
    pub underline_color: Option<Expr<Paint>>,
    pub underline_width: Option<Expr<f64>>,
    pub underline_offset: Option<Expr<f64>>,
    pub strike: Option<Expr<f64>>,
    pub strike_color: Option<Expr<Paint>>,
    pub strike_width: Option<Expr<f64>>,
    pub strike_offset: Option<Expr<f64>>,
    pub text: Option<Expr<Arc<String>>>,
    pub sh_language: Option<Arc<String>>,
    pub sh_theme: Option<Arc<String>>,
    pub wrap: Option<Expr<f64>>,
    pub text_align: Option<renderer_core::TextAlign>,
    pub reveal: Option<Expr<f64>>,

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

    fn node_box(&mut self) -> NodeBox {
        NodeBox {
            position: self.position(),
            size: self.size(),
            z_level: self.z_level(),
            rotation: AttrExpr(self.rotation.take()),
            pivot_x: AttrExpr(self.pivot_x.take()),
            pivot_y: AttrExpr(self.pivot_y.take()),
            scale_x: AttrExpr(self.scale_x.take()),
            scale_y: AttrExpr(self.scale_y.take()),
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
            dash_on: AttrExpr(self.dash_on.take()),
            dash_off: AttrExpr(self.dash_off.take()),
            dash_offset: AttrExpr(self.dash_offset.take()),
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
            underline: AttrExpr(self.underline.take()),
            strike: AttrExpr(self.strike.take()),
        }
    }

    fn underline_style(&mut self) -> DecorationStyle {
        DecorationStyle {
            color: AttrExpr(self.underline_color.take()),
            width: AttrExpr(self.underline_width.take()),
            offset: AttrExpr(self.underline_offset.take()),
        }
    }

    fn strike_style(&mut self) -> DecorationStyle {
        DecorationStyle {
            color: AttrExpr(self.strike_color.take()),
            width: AttrExpr(self.strike_width.take()),
            offset: AttrExpr(self.strike_offset.take()),
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
            Kind::Group => NodeKind::Group {
                node_box: def.node_box(),
                alpha: AttrExpr(def.alpha.take()),
                clip_x: AttrExpr(def.clip_x.take()),
                clip_y: AttrExpr(def.clip_y.take()),
                clip_w: AttrExpr(def.clip_w.take()),
                clip_h: AttrExpr(def.clip_h.take()),
                padding: Padding {
                    top: AttrExpr(def.padding_top.take()),
                    right: AttrExpr(def.padding_right.take()),
                    bottom: AttrExpr(def.padding_bottom.take()),
                    left: AttrExpr(def.padding_left.take()),
                },
                camera: Camera {
                    zoom: AttrExpr(def.camera_zoom.take()),
                    x: AttrExpr(def.camera_x.take()),
                    y: AttrExpr(def.camera_y.take()),
                },
                layout: def
                    .layout
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("group node {} missing `layout`", idx))?,
                children,
            },
            Kind::Rect => NodeKind::Rect {
                node_box: def.node_box(),
                style: def.style(),
                radius: AttrExpr(def.radius.take()),
            },
            Kind::Ellipse => NodeKind::Ellipse {
                node_box: def.node_box(),
                style: def.style(),
            },
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
            Kind::Text => NodeKind::Text {
                node_box: def.node_box(),
                keep_aspect: AttrExpr(def.keep_aspect.take()),
                wrap: AttrExpr(def.wrap.take()),
                text_align: def.text_align.take(),
                text_style: def.text_style(),
                underline_style: def.underline_style(),
                strike_style: def.strike_style(),
                sh_language: def.sh_language.take(),
                sh_theme: def.sh_theme.take(),
                reveal: AttrExpr(def.reveal.take()),
                children,
            },
            Kind::TextGroup => NodeKind::TextGroup {
                node_box: def.node_box(),
                text_style: def.text_style(),
                underline_style: def.underline_style(),
                strike_style: def.strike_style(),
                text_align: def.text_align.take(),
                children,
            },
            Kind::TextSpan => NodeKind::TextSpan {
                node_box: def.node_box(),
                text_style: def.text_style(),
                underline_style: def.underline_style(),
                strike_style: def.strike_style(),
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
            Kind::Image => NodeKind::Image {
                node_box: def.node_box(),
                alpha: AttrExpr(def.alpha.take()),
                path: AttrExpr(def.file.take()),
                keep_aspect: AttrExpr(def.keep_aspect.take()),
                children,
            },
            Kind::Layer => NodeKind::Layer {
                node_box: def.node_box(),
                alpha: AttrExpr(def.alpha.take()),
                layer_name: def
                    .layer_name
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("layer node {} missing `layer_name`", idx))?,
                children,
            },
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
