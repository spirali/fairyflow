use crate::FrameId;
use crate::animdef::AnimationDef;
use crate::avalue::AnimatedValue;
use crate::basictypes::{AvId, NodeId};
use crate::nodes::{
    CallExpr, CallParamsNodeTransform, Expr, Node, NodeKind, Position, SceneDef, Size, Style,
    TextStyle, TopLevelExpr, Value,
};
use anyhow::bail;
use by_address::ByAddress;
use renderer::Inheritable;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

const EVAL_DEPTH_MAX: u32 = 64;

pub(crate) struct EvalCtx<'a> {
    frame: FrameId,
    root: &'a AnimationDef,
    evaluating_exprs: RefCell<HashSet<*const Expr>>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(frame: FrameId, root: &'a AnimationDef) -> Self {
        Self {
            frame,
            root,
            evaluating_exprs: RefCell::new(HashSet::new()),
        }
    }

    pub fn scene_width(&self) -> anyhow::Result<f64> {
        self.root.scene.size.width.eval_f64(self)
    }

    pub fn scene_height(&self) -> anyhow::Result<f64> {
        self.root.scene.size.height.eval_f64(self)
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

    #[must_use]
    pub(crate) fn begin_eval(&self, expr: &'a Expr) -> bool {
        self.evaluating_exprs.borrow_mut().insert(expr)
    }

    pub(crate) fn end_eval(&self, expr: &'a Expr) {
        assert!(
            self.evaluating_exprs
                .borrow_mut()
                .remove(&(expr as *const Expr))
        )
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

impl TopLevelExpr {
    pub fn eval<'a>(&'a self, ctx: &'a EvalCtx<'a>) -> anyhow::Result<Value> {
        let expr = self.get_expr();
        if !ctx.begin_eval(expr) {
            return Ok(Value::Recursive);
        }
        let result = expr.eval(ctx);
        ctx.end_eval(expr);
        result
    }

    pub fn eval_f64<'a>(&'a self, ctx: &'a EvalCtx<'a>) -> anyhow::Result<f64> {
        self.eval(ctx)?.as_f64()
    }

    pub fn eval_as_inheritable(&self, ctx: &EvalCtx) -> anyhow::Result<Inheritable<Value>> {
        Ok(match self.get_expr() {
            Expr::Inherited { .. } => Inheritable::Inherited(self.eval(ctx)?),
            _ => Inheritable::Own(self.eval(ctx)?),
        })
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
            }
            Expr::Inherited { expr } => expr.eval(ctx),
        }
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
            }
            CallExpr::Sub(pair) => {
                tracing::trace!("Call::Sub");
                let va = pair.a.eval(ctx)?.as_f64()?;
                let vb = pair.b.eval(ctx)?.as_f64()?;
                tracing::trace!(a = va, b = vb, result = va - vb, "Call::Sub result");
                Ok(Value::Float(va - vb))
            }
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
                let node = ctx.node(node.get_id())?;
                Ok(Value::Float(node.default_width(ctx)?))
            }
            CallExpr::DefaultHeight { node } => {
                let node = ctx.node(node.get_id())?;
                Ok(Value::Float(node.default_height(ctx)?))
            }
            CallExpr::DefaultX { node } => {
                let node = ctx.node(node.get_id())?;
                Ok(Value::Float(node.default_x(ctx)?))
            }
            CallExpr::DefaultY { node } => {
                let node = ctx.node(node.get_id())?;
                Ok(Value::Float(node.default_y(ctx)?))
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
                layout,
                children,
                z_level,
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
                z_level,
            } => renderer::NodeKind::Ellipse {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
            },
            NodeKind::Path {
                style,
                z_level,
                children,
            } => renderer::NodeKind::Path {
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
                children: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_path_cmd(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            NodeKind::Text {
                position,
                z_level,
                children,
                ..
            } => renderer::NodeKind::Text {
                position: position.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
                lines: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            NodeKind::Image {
                position,
                size,
                z_level,
                alpha,
                path,
                keep_aspect,
            } => renderer::NodeKind::Image {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?.map(|x| x.as_f64())?,
                alpha: alpha.eval_f64(ctx)?,
                path: path.eval(ctx)?.as_string_ref()?,
                keep_aspect: keep_aspect.eval(ctx)?.as_bool()?,
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
