use crate::FrameId;
use crate::animdef::AnimationDef;
use crate::avalue::AnimatedValue;
use crate::basictypes::{AvId, NodeId};
use crate::defs::{
    CallExpr, CallParamsNodeTransform, Expr, Node, NodeKind, Position, SceneDef, Size, Style,
    TextStyle, Value,
};
use anyhow::bail;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use renderer::Inheritable;

const EVAL_DEPTH_MAX: u32 = 64;

pub(crate) struct EvalCtx<'a> {
    frame: FrameId,
    root: &'a AnimationDef,
    depth: Cell<u32>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(frame: FrameId, root: &'a AnimationDef) -> Self {
        Self {
            frame,
            root,
            depth: Cell::new(0),
        }
    }

    #[inline]
    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn av(&self, av_id: AvId) -> anyhow::Result<&AnimatedValue> {
        self.root
            .animated_values
            .get(&av_id)
            .ok_or_else(|| anyhow::anyhow!("Av {} not found", av_id))
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

    pub fn node(&self, node_id: NodeId) -> anyhow::Result<&'a Node> {
        self.root
            .nodes
            .get(&node_id)
            .ok_or_else(|| anyhow::anyhow!("node {:?} not found", node_id))
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
        NodeKind::Group {
            position,
            scale_x,
            scale_y,
            rotation,
            ..
        } => {
            let tx = position.x.eval_f64(ctx)?;
            let ty = position.y.eval_f64(ctx)?;
            let sx = scale_x.eval_f64(ctx)?;
            let sy = scale_y.eval_f64(ctx)?;
            let r = rotation.eval_f64(ctx)?.to_radians();
            Ok((tx, ty, sx, sy, r.cos(), r.sin()))
        }
        _ => anyhow::bail!("node {:?} has no group transform (not a group)", node.id),
    }
}

/// Transform (lx, ly) from node n's local space into n's parent space.
/// parent_x = cos(r)*sx*lx - sin(r)*sy*ly + tx
/// parent_y = sin(r)*sx*lx + cos(r)*sy*ly + ty
fn from_node_pos(
    tx: f64,
    ty: f64,
    sx: f64,
    sy: f64,
    cos_r: f64,
    sin_r: f64,
    lx: f64,
    ly: f64,
) -> (f64, f64) {
    (
        cos_r * sx * lx - sin_r * sy * ly + tx,
        sin_r * sx * lx + cos_r * sy * ly + ty,
    )
}

/// Transform (px, py) from parent space into node n's local space.
/// q = parent_pos - translation;  local_x = (cos(r)*qx + sin(r)*qy) / sx
fn into_node_pos(
    tx: f64,
    ty: f64,
    sx: f64,
    sy: f64,
    cos_r: f64,
    sin_r: f64,
    px: f64,
    py: f64,
) -> (f64, f64) {
    let qx = px - tx;
    let qy = py - ty;
    (
        (cos_r * qx + sin_r * qy) / sx,
        (-sin_r * qx + cos_r * qy) / sy,
    )
}

/// Transform point (x, y) from source node's local coordinate space into target node's local space.
fn node_transform(
    source: NodeId,
    target: NodeId,
    x: f64,
    y: f64,
    ctx: &EvalCtx,
) -> anyhow::Result<(f64, f64)> {
    let _span = tracing::trace_span!(
        "node_transform",
        source = source.as_u64(),
        target = target.as_u64(),
        x,
        y
    )
    .entered();
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

/// Walk parent links to find the nearest `Text` ancestor of `node_id`.
fn find_text_ancestor(
    node_id: crate::basictypes::NodeId,
    ctx: &EvalCtx,
) -> anyhow::Result<crate::basictypes::NodeId> {
    let mut current = node_id;
    loop {
        let node = ctx.node(current)?;
        if matches!(node.kind, NodeKind::Text { .. }) {
            return Ok(current);
        }
        current = node
            .parent
            .ok_or_else(|| anyhow::anyhow!("node {:?} has no Text ancestor", node_id))?;
    }
}

/// Returns the position `(x, y)` of a `TextGroup` or `TextSpan` node within its
/// parent `Text` element.  For all other node kinds returns `(0, 0)`.
fn text_default_pos(
    node_id: crate::basictypes::NodeId,
    ctx: &EvalCtx,
) -> anyhow::Result<(f32, f32)> {
    let node = ctx.node(node_id)?;
    match &node.kind {
        NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {}
        _ => return Ok((0.0, 0.0)),
    }
    let text_id = find_text_ancestor(node_id, ctx)?;
    let NodeKind::Text { children, .. } = &ctx.node(text_id)?.kind else {
        unreachable!()
    };
    let lines = children
        .iter()
        .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(renderer::measure_text_node_pos(&lines, node_id.as_u64()).unwrap_or((0.0, 0.0)))
}

/// Returns `(width, height)` for the natural (unwrapped) size of a node.
/// - `Text`: measures all lines.
/// - `TextGroup` / `TextSpan`: measures the node's own content as a single line.
/// - All other kinds: returns `(0, 0)`.
fn text_default_size(
    node_id: crate::basictypes::NodeId,
    ctx: &EvalCtx,
) -> anyhow::Result<(f32, f32)> {
    let node = ctx.node(node_id)?;
    match &node.kind {
        NodeKind::Text { children, .. } => {
            let lines = children
                .iter()
                .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(renderer::measure_text(&lines))
        }
        NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
            let child = node.eval_as_text_child(ctx)?;
            Ok(renderer::measure_text(&[child]))
        }
        _ => Ok((0.0, 0.0)),
    }
}

impl Expr {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<Value> {
        match self {
            Expr::Const(v) => Ok(v.clone()),
            Expr::Call(call) => call.eval(ctx),
            Expr::Av(av_id) => {
                tracing::trace!(av_id = %av_id.get_id(), "Expr::Av");
                ctx.av(av_id.get_id())?.eval(ctx)
            },
            Expr::Inherited { expr } => expr.eval(ctx),
        }
    }

    pub fn eval_as_inheritable(&self, ctx: &EvalCtx) -> anyhow::Result<Inheritable<Value>> {
        Ok(match self {
            Expr::Inherited { expr } => Inheritable::Inherited(expr.eval(ctx)?),
            e => Inheritable::Own(e.eval(ctx)?)
        })
    }

    /// Shortcut for the most used eval
    pub fn eval_f64(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        self.eval(ctx)?.as_f64()
    }
}

impl CallExpr {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<Value> {
        match self {
            CallExpr::Add(pair) => {
                tracing::trace!("Call::Add");
                let va = pair.a.eval(ctx)?.as_f64()?;
                let vb = pair.b.eval(ctx)?.as_f64()?;
                tracing::trace!(a = va, b = vb, result = va + vb, "Call::Add result");
                Ok(Value::Float(va + vb))
            },
            CallExpr::Sub(pair) => {
                tracing::trace!("Call::Sub");
                let va = pair.a.eval(ctx)?.as_f64()?;
                let vb = pair.b.eval(ctx)?.as_f64()?;
                tracing::trace!(a = va, b = vb, result = va - vb, "Call::Sub result");
                Ok(Value::Float(va - vb))
            },
            CallExpr::Mul(pair) => {
                tracing::trace!("Call::Mul");
                let va = pair.a.eval(ctx)?.as_f64()?;
                let vb = pair.b.eval(ctx)?.as_f64()?;
                tracing::trace!(a = va, b = vb, result = va * vb, "Call::Mul result");
                Ok(Value::Float(va * vb))
            }
            CallExpr::NodeTransformX(params) => {
                tracing::trace!(
                    source = params.source.get_id().as_u64(),
                    target = params.target.get_id().as_u64(),
                    "Call::NodeTransformX"
                );
                let xv = params.x.eval(ctx)?.as_f64()?;
                let yv = params.y.eval(ctx)?.as_f64()?;
                let (px, _py) =
                    node_transform(params.source.get_id(), params.target.get_id(), xv, yv, ctx)?;
                Ok(Value::Float(px))
            }
            CallExpr::NodeTransformY(params) => {
                tracing::trace!(
                    source = params.source.get_id().as_u64(),
                    target = params.target.get_id().as_u64(),
                    "Call::NodeTransformY"
                );
                let xv = params.x.eval(ctx)?.as_f64()?;
                let yv = params.y.eval(ctx)?.as_f64()?;
                let (_px, py) =
                    node_transform(params.source.get_id(), params.target.get_id(), xv, yv, ctx)?;
                Ok(Value::Float(py))
            }
            CallExpr::DefaultWidth { node } => {
                let (w, _h) = text_default_size(node.get_id(), ctx)?;
                Ok(Value::Float(w as f64))
            }
            CallExpr::DefaultHeight { node } => {
                let (_w, h) = text_default_size(node.get_id(), ctx)?;
                Ok(Value::Float(h as f64))
            }
            CallExpr::DefaultX { node } => {
                let (x, _y) = text_default_pos(node.get_id(), ctx)?;
                Ok(Value::Float(x as f64))
            }
            CallExpr::DefaultY { node } => {
                let (_x, y) = text_default_pos(node.get_id(), ctx)?;
                Ok(Value::Float(y as f64))
            }
        }
    }
}

// ─────────────────────────── Mixin eval impls ───────────────────────────────

impl Position {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Position> {
        Ok(renderer::Position {
            x: self.x.eval_f64(ctx)?,
            y: self.y.eval_f64(ctx)?,
        })
    }
}

impl Size {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Size> {
        Ok(renderer::Size {
            width: self.width.eval_f64(ctx)?,
            height: self.height.eval_f64(ctx)?,
        })
    }
}

impl Style {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Style> {
        Ok(renderer::Style {
            fill_color: self.fill_color.eval(ctx)?.into_color(),
            stroke_color: self.stroke_color.eval(ctx)?.into_color(),
            stroke_width: self.stroke_width.eval_f64(ctx)?,
            alpha: self.alpha.eval_f64(ctx)?,
        })
    }
}

// ──────────────────────────── Node eval impls ───────────────────────────────

impl Node {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Node> {
        let _span = tracing::trace_span!("node.eval", node_id = self.id.as_u64()).entered();
        let kind = match &self.kind {
            NodeKind::Group {
                position,
                size,
                alpha,
                rotation,
                scale_x,
                scale_y,
                children,
                z_level
            } => renderer::NodeKind::Group {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                alpha: alpha.eval_f64(ctx)?,
                rotation: rotation.eval_f64(ctx)?,
                scale_x: scale_x.eval_f64(ctx)?,
                scale_y: scale_y.eval_f64(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
                children: {
                    let mut result = Vec::new();
                    for &id in children {
                        let node = ctx.node(id)?;
                        if node.is_active(ctx.frame()) {
                            result.push(node.eval(ctx)?);
                        }
                    }
                    result
                },
            },
            NodeKind::Rect {
                position,
                size,
                style,
                z_level,
            } => renderer::NodeKind::Rect {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
            },
            NodeKind::Ellipse {
                position,
                size,
                style,
                z_level
            } => renderer::NodeKind::Ellipse {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
            },
            NodeKind::Path { style, z_level, children } => renderer::NodeKind::Path {
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
                children: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_path_cmd(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            NodeKind::Text {
                position, z_level, children, ..
            } => renderer::NodeKind::Text {
                position: position.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
                lines: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            _ => anyhow::bail!(
                "path command / text-internal nodes cannot appear as scene tree nodes"
            ),
        };
        Ok(renderer::Node {
            id: self.id.as_u64(),
            kind,
        })
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
            NodeKind::Cubic {
                position,
                c1_x,
                c1_y,
                c2_x,
                c2_y,
            } => Ok(renderer::PathCommand::Cubic {
                id: self.id.as_u64(),
                position: position.eval(ctx)?,
                c1_x: c1_x.eval_f64(ctx)?,
                c1_y: c1_y.eval_f64(ctx)?,
                c2_x: c2_x.eval_f64(ctx)?,
                c2_y: c2_y.eval_f64(ctx)?,
            }),
            _ => anyhow::bail!("expected path command node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_child(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::TextChild> {
        match &self.kind {
            NodeKind::TextGroup { .. } => {
                Ok(renderer::TextChild::Group(self.eval_as_text_group(ctx)?))
            }
            NodeKind::TextSpan { .. } => {
                Ok(renderer::TextChild::Span(self.eval_as_text_span(ctx)?))
            }
            _ => anyhow::bail!("expected t_group or t_span node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_group(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::TextGroup> {
        match &self.kind {
            NodeKind::TextGroup { children, .. } => Ok(renderer::TextGroup {
                id: self.id.as_u64(),
                children: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            }),
            _ => anyhow::bail!("expected t_group node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_span(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::TextSpan> {
        match &self.kind {
            NodeKind::TextSpan { text_style, text } => {
                let TextStyle {
                    style,
                    font,
                    font_size,
                    italic,
                } = text_style;
                Ok(renderer::TextSpan {
                    id: self.id.as_u64(),
                    text: text.eval(ctx)?.as_string_ref()?,
                    style: style.eval(ctx)?,
                    font_family: font.eval(ctx)?.as_string_ref()?,
                    font_size: font_size.eval_f64(ctx)?,
                    italic: italic.eval(ctx)?.as_bool()?,
                })
            }
            _ => anyhow::bail!("expected TextSpan node, got {:?}", self.id),
        }
    }
}

// ─────────────────────────── SceneDef eval impl ─────────────────────────────

impl SceneDef {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer::Scene> {
        let _span = tracing::debug_span!("frame", frame = ctx.frame().as_u32()).entered();
        let fill_color = self.fill_color.eval(ctx)?.into_color().unwrap_or_default();
        let mut children = Vec::with_capacity(self.children.len());
        for &id in &self.children {
            let node = ctx.node(id)?;
            if node.is_active(ctx.frame()) {
                children.push(node.eval(ctx)?);
            }
        }
        Ok(renderer::Scene {
            width: self.size.width.eval_f64(ctx)?,
            height: self.size.height.eval_f64(ctx)?,
            fill_color,
            children,
        })
    }
}
