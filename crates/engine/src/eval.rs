use std::collections::HashSet;
use std::cell::{Cell, RefCell};
use anyhow::bail;
use crate::animdef::AnimationDef;
use crate::avalue::{AnimatedValue, AnimatedValueKind};
use crate::basictypes::{AvId, NodeId};
use crate::defs::{CallExpr, CallParamsNodeTransform, Expr, Node, NodeKind, Position, SceneDef, Size, Style, TextStyle, Value};
use crate::FrameId;

const EVAL_DEPTH_MAX: u32 = 64;

pub(crate) struct EvalCtx<'a> {
    frame: FrameId,
    root: &'a AnimationDef,
    depth: Cell<u32>,
}

impl<'a> EvalCtx<'a> {

    pub fn new(frame: FrameId, root: &'a AnimationDef) -> Self {
        Self { frame, root, depth: Cell::new(0) }
    }

    pub fn clone_at_frame(&self, frame: FrameId) -> Self {
        Self { frame, root: self.root, depth: self.depth.clone() }
    }

    #[inline]
    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn av(&self, av_id: AvId) -> anyhow::Result<&AnimatedValue> {
        self.root.animated_values.get(&av_id).ok_or_else(|| {
            anyhow::anyhow!("Av {} not found", av_id)
        })
    }

    pub(crate) fn begin_eval(&self) -> anyhow::Result<()> {
        let new_depth = self.depth.get() + 1;
        if new_depth > EVAL_DEPTH_MAX {
            bail!("Evaluation depth reached");
        }
        self.depth.set(new_depth);
        Ok(())
    }

    pub(crate) fn end_eval(&self) {
        let new_depth = self.depth.get();
        assert!(new_depth > 0);
        self.depth.set(new_depth - 1);
    }

    pub fn eval_av(&self, av_id: AvId) -> anyhow::Result<Value> {
        self.av(av_id)?.eval(self)
    }

    pub fn av_f64(&self, av_id: AvId) -> anyhow::Result<f64> {
        self.eval_av(av_id)?.as_f64()
    }

    pub fn node(&self, node_id: NodeId) -> anyhow::Result<&'a Node> {
        self.root.nodes.get(&node_id).ok_or_else(|| {
            anyhow::anyhow!("node {:?} not found", node_id)
        })
    }
}

// ──────────────────────── Coordinate transformation ─────────────────────────

/// Collect ancestor chain from `node_id` up to (and including) the topmost ancestor.
/// Result: [node_id, parent, grandparent, ...]
fn ancestor_chain(ctx: &EvalCtx, node_id: NodeId) -> Vec<NodeId> {
    let mut chain = Vec::new();
    let mut current = node_id;
    while let Some(node) = ctx.root.nodes.get(&current) {
        if matches!(node.kind, NodeKind::Group { .. }) {
            chain.push(current);
        }
        match node.parent {
            Some(parent_id) => current = parent_id,
            None => break,
        }
    }
    chain
}

/// Get the affine transform parameters of a Group node at the current frame.
/// Returns (tx, ty, sx, sy, cos_r, sin_r).
fn group_transform(node: &Node, ctx: &EvalCtx) -> anyhow::Result<(f64, f64, f64, f64, f64, f64)> {
    match &node.kind {
        NodeKind::Group { position, scale_x, scale_y, rotation, .. } => {
            let tx = ctx.av_f64(position.x)?;
            let ty = ctx.av_f64(position.y)?;
            let sx = ctx.av_f64(*scale_x)?;
            let sy = ctx.av_f64(*scale_y)?;
            let r = ctx.av_f64(*rotation)?.to_radians();
            Ok((tx, ty, sx, sy, r.cos(), r.sin()))
        }
        _ => anyhow::bail!("node {:?} has no group transform (not a group)", node.id),
    }
}

/// Transform (lx, ly) from node n's local space into n's parent space.
/// parent_x = cos(r)*sx*lx - sin(r)*sy*ly + tx
/// parent_y = sin(r)*sx*lx + cos(r)*sy*ly + ty
fn from_node_pos(tx: f64, ty: f64, sx: f64, sy: f64, cos_r: f64, sin_r: f64, lx: f64, ly: f64) -> (f64, f64) {
    (cos_r * sx * lx - sin_r * sy * ly + tx,
     sin_r * sx * lx + cos_r * sy * ly + ty)
}

/// Transform (px, py) from parent space into node n's local space.
/// q = parent_pos - translation;  local_x = (cos(r)*qx + sin(r)*qy) / sx
fn into_node_pos(tx: f64, ty: f64, sx: f64, sy: f64, cos_r: f64, sin_r: f64, px: f64, py: f64) -> (f64, f64) {
    let qx = px - tx;
    let qy = py - ty;
    ((cos_r * qx + sin_r * qy) / sx,
     (-sin_r * qx + cos_r * qy) / sy)
}

/// Transform point (x, y) from source node's local coordinate space into target node's local space.
fn node_transform(source: NodeId, target: NodeId, x: f64, y: f64, ctx: &EvalCtx) -> anyhow::Result<(f64, f64)> {
    let _span = tracing::trace_span!(
        "node_transform",
        source = source.as_u64(), target = target.as_u64(), x, y
    ).entered();
    if source == target {
        return Ok((x, y));
    }

    let mut cs = ancestor_chain(ctx, source); // [source, ..., top]
    let mut ct = ancestor_chain(ctx, target); // [target, ..., top]

    // Strip common suffix to find LCA
    while cs.last() == ct.last() && !cs.is_empty() && !ct.is_empty() {
        cs.pop();
        ct.pop();
    }
    // cs = path from source up to (not including) LCA
    // ct reversed = path from just-below-LCA down to target
    ct.reverse();

    let mut px = x;
    let mut py = y;

    // Go up: each node transforms from its local space to its parent's space
    for node_id in cs {
        let node = ctx.node(node_id)?;
        let (tx, ty, sx, sy, cos_r, sin_r) = group_transform(node, ctx)?;
        (px, py) = from_node_pos(tx, ty, sx, sy, cos_r, sin_r, px, py);
    }

    // Go down: each node transforms from parent space into its local space
    for node_id in ct {
        let node = ctx.node(node_id)?;
        let (tx, ty, sx, sy, cos_r, sin_r) = group_transform(node, ctx)?;
        (px, py) = into_node_pos(tx, ty, sx, sy, cos_r, sin_r, px, py);
    }

    Ok((px, py))
}

// ───────────────────────────── Expr / Call ──────────────────────────────────

impl Expr {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<Value> {
        match self {
            Expr::Const(v) => Ok(v.clone()),
            Expr::Call(call) => call.eval(ctx),
            Expr::Av(av_id) => {
                tracing::trace!(av_id = %av_id.get_id(), "Expr::Av");
                ctx.eval_av(av_id.get_id())
            }
        }
    }

    pub fn eval_at_frame(&self, ctx: &EvalCtx, frame: FrameId) -> anyhow::Result<Value> {
        match self {
            Expr::Const(v) => Ok(v.clone()),
            Expr::Call(call) => {
                let new_ctx = ctx.clone_at_frame(frame);
                call.eval(&new_ctx)
            },
            Expr::Av(av_id) => {
                tracing::trace!(av_id = %av_id.get_id(), "Expr::Av");
                let new_ctx = ctx.clone_at_frame(frame);
                new_ctx.eval_av(av_id.get_id())
            }
        }
    }
}

impl CallExpr {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<Value> {
        match self {
            CallExpr::Hold { av } => {
                tracing::trace!(av_id = %av.get_id(), "Call::Hold");
                let av = ctx.av(av.get_id())?;
                match &av.kind {
                    AnimatedValueKind::Const { value } => {
                        Ok(value.eval(ctx)?)
                    }
                    AnimatedValueKind::Animated { values } => {
                        let (f, v) = values.scan_left(ctx.frame().prev()).unwrap();
                        let result = v.value.eval_at_frame(ctx, f);
                        println!("{:?}", result);
                        result
                    }
                }
            }
            CallExpr::Add(pair) => {
                tracing::trace!("Call::Add");
                let va = pair.a.eval(ctx)?.as_f64()?;
                let vb = pair.b.eval(ctx)?.as_f64()?;
                tracing::trace!(a = va, b = vb, result = va + vb, "Call::Add result");
                Ok(Value::Float(va + vb))
            }
            CallExpr::NodeTransformX(params) => {
                tracing::trace!(
                    source = params.source.get_id().as_u64(), target = params.target.get_id().as_u64(),
                    "Call::NodeTransformX"
                );
                let xv = params.x.eval(ctx)?.as_f64()?;
                let yv = params.y.eval(ctx)?.as_f64()?;
                let (px, _py) = node_transform(params.source.get_id(), params.target.get_id(), xv, yv, ctx)?;
                Ok(Value::Float(px))
            }
            CallExpr::NodeTransformY(params) => {
                tracing::trace!(
                    source = params.source.get_id().as_u64(), target = params.target.get_id().as_u64(),
                    "Call::NodeTransformY"
                );
                let xv = params.x.eval(ctx)?.as_f64()?;
                let yv = params.y.eval(ctx)?.as_f64()?;
                let (_px, py) = node_transform(params.source.get_id(), params.target.get_id(), xv, yv, ctx)?;
                Ok(Value::Float(py))
            }
        }
    }
}

// ─────────────────────────── Mixin eval impls ───────────────────────────────

impl Position {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Position> {
        Ok(renderer::Position { x: ctx.av_f64(self.x)?, y: ctx.av_f64(self.y)? })
    }
}

impl Size {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Size> {
        Ok(renderer::Size { width: ctx.av_f64(self.width)?, height: ctx.av_f64(self.height)? })
    }
}

impl Style {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Style> {
        Ok(renderer::Style {
            fill_color: ctx.eval_av(self.fill_color)?.into_color(),
            stroke_color: ctx.eval_av(self.stroke_color)?.into_color(),
            stroke_width: ctx.av_f64(self.stroke_width)?,
            alpha: ctx.av_f64(self.alpha)?,
        })
    }
}

impl TextStyle {
    fn eval_span(&self, ctx: &EvalCtx, text: String) -> anyhow::Result<renderer::TextSpan> {
        let style = self.style.eval(ctx)?;
        Ok(renderer::TextSpan {
            text,
            font_family: ctx.eval_av(self.font())?.into_str()?,
            fill_color: style.fill_color,
            stroke_color: style.stroke_color,
            stroke_width: style.stroke_width,
            alpha: style.alpha,
            italic: ctx.eval_av(self.italic())?.as_bool()?,
        })
    }
}

// ──────────────────────────── Node eval impls ───────────────────────────────

impl Node {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Node> {
        let _span = tracing::trace_span!("node.eval", node_id = self.id.as_u64()).entered();
        let kind = match &self.kind {
            NodeKind::Group { position, size, alpha, rotation, scale_x, scale_y, children } => {
                renderer::NodeKind::Group {
                    position: position.eval(ctx)?,
                    size: size.eval(ctx)?,
                    alpha: ctx.av_f64(*alpha)?,
                    rotation: ctx.av_f64(*rotation)?,
                    scale_x: ctx.av_f64(*scale_x)?,
                    scale_y: ctx.av_f64(*scale_y)?,
                    children: children.iter()
                        .map(|&id| ctx.node(id)?.eval(ctx))
                        .collect::<anyhow::Result<Vec<_>>>()?,
                }
            }
            NodeKind::Rect { position, size, style } => renderer::NodeKind::Rect {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                style: style.eval(ctx)?,
            },
            NodeKind::Ellipse { position, size, style } => renderer::NodeKind::Ellipse {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                style: style.eval(ctx)?,
            },
            NodeKind::Path { style, children } => renderer::NodeKind::Path {
                style: style.eval(ctx)?,
                children: children.iter()
                    .map(|&id| ctx.node(id)?.eval_as_path_cmd(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            NodeKind::Text { position, children, .. } => renderer::NodeKind::Text {
                position: position.eval(ctx)?,
                lines: children.iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_line(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            _ => anyhow::bail!("path command / text-internal nodes cannot appear as scene tree nodes"),
        };
        Ok(renderer::Node { id: self.id.as_u64(), kind })
    }

    pub fn eval_as_path_cmd(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::PathCommand> {
        match &self.kind {
            NodeKind::Move { position } => Ok(renderer::PathCommand::Move {
                id: self.id.as_u64(),
                position: position.eval(ctx)?,
            }),
            NodeKind::Line { position } => Ok(renderer::PathCommand::Line {
                id: self.id.as_u64(),
                position: position.eval(ctx)?,
            }),
            NodeKind::Cubic { position, c1_x, c1_y, c2_x, c2_y } => Ok(renderer::PathCommand::Cubic {
                id: self.id.as_u64(),
                position: position.eval(ctx)?,
                c1_x: ctx.av_f64(*c1_x)?,
                c1_y: ctx.av_f64(*c1_y)?,
                c2_x: ctx.av_f64(*c2_x)?,
                c2_y: ctx.av_f64(*c2_y)?,
            }),
            _ => anyhow::bail!("expected path command node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_line(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::TextLine> {
        match &self.kind {
            NodeKind::TextLine { children, .. } => Ok(renderer::TextLine {
                spans: children.iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_span(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            }),
            _ => anyhow::bail!("expected Line node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_span(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::TextSpan> {
        match &self.kind {
            NodeKind::TextSpan { text_style, text } => {
                let text_str = ctx.eval_av(*text)?.into_str()?;
                text_style.eval_span(ctx, text_str)
            }
            _ => anyhow::bail!("expected TextSpan node, got {:?}", self.id),
        }
    }
}

// ─────────────────────────── SceneDef eval impl ─────────────────────────────

impl SceneDef {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Scene> {
        let _span = tracing::debug_span!("frame", frame = ctx.frame().as_u32()).entered();
        let fill_color = ctx.eval_av(self.fill_color)?.into_color().unwrap_or_default();
        let children = self.children.iter()
            .map(|&id| ctx.node(id)?.eval(ctx))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(renderer::Scene {
            width: ctx.av_f64(self.size.width)?,
            height: ctx.av_f64(self.size.height)?,
            fill_color,
            children,
        })
    }
}
