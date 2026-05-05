use crate::FrameId;
use crate::basictypes::NodeId;
use crate::nodes::{AttrExpr, Node, NodeKind, Position, SceneDef, Size, Style, TextStyle};
use crate::paths::{path_length, point_in_path};
use crate::values::{Eval, Expr, FloatCall, FloatParamsPair, Value};
use renderer_core::Inheritable;
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

pub(crate) struct EvalCtx<'a> {
    frame: FrameId,
    scene_def: &'a SceneDef,
    nodes: &'a HashMap<NodeId, Node>,
    evaluating_exprs: RefCell<HashSet<usize>>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(frame: FrameId, scene_def: &'a SceneDef, nodes: &'a HashMap<NodeId, Node>) -> Self {
        Self {
            frame,
            scene_def,
            nodes,
            evaluating_exprs: RefCell::new(HashSet::new()),
        }
    }

    pub fn scene_width(&self) -> anyhow::Result<f64> {
        self.scene_def.size.width.eval(self)
    }

    pub fn scene_height(&self) -> anyhow::Result<f64> {
        self.scene_def.size.height.eval(self)
    }

    #[inline]
    pub fn frame(&self) -> FrameId {
        self.frame
    }

    #[must_use]
    pub(crate) fn begin_eval<T: Value + DeserializeOwned>(&self, expr: &'a Expr<T>) -> bool {
        self.evaluating_exprs
            .borrow_mut()
            .insert(expr as *const _ as usize)
    }

    pub(crate) fn end_eval<T: Value + DeserializeOwned>(&self, expr: &'a Expr<T>) {
        assert!(
            self.evaluating_exprs
                .borrow_mut()
                .remove(&(expr as *const _ as usize))
        )
    }

    pub fn node(&self, node_id: NodeId) -> anyhow::Result<&'a Node> {
        self.nodes
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
    while let Some(node) = ctx.nodes.get(&current) {
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
/// Returns (tx, ty, sx, sy, cos_r, sin_r, pivot_x_abs, pivot_y_abs).
fn group_transform(
    node: &Node,
    ctx: &EvalCtx,
) -> anyhow::Result<(f64, f64, f64, f64, f64, f64, f64, f64)> {
    match &node.kind {
        NodeKind::Group {
            position,
            size,
            scale_x,
            scale_y,
            rotation,
            pivot_x,
            pivot_y,
            ..
        } => {
            let tx = position.x.eval(ctx)?;
            let ty = position.y.eval(ctx)?;
            let sx = scale_x.eval(ctx)?;
            let sy = scale_y.eval(ctx)?;
            let r = rotation.eval(ctx)?.to_radians();
            let w = size.width.eval(ctx)?;
            let h = size.height.eval(ctx)?;
            let pvx = pivot_x.eval(ctx)? * w;
            let pvy = pivot_y.eval(ctx)? * h;
            Ok((tx, ty, sx, sy, r.cos(), r.sin(), pvx, pvy))
        }
        _ => anyhow::bail!("node {:?} has no group transform (not a group)", node.id),
    }
}

/// Transform (lx, ly) from node n's local space into n's parent space.
/// Pivot is expressed in absolute local-space coordinates (pivot_x * width, pivot_y * height).
fn from_node_pos(
    tx: f64,
    ty: f64,
    sx: f64,
    sy: f64,
    cos_r: f64,
    sin_r: f64,
    pvx: f64,
    pvy: f64,
    lx: f64,
    ly: f64,
) -> (f64, f64) {
    let qx = lx - pvx;
    let qy = ly - pvy;
    (
        cos_r * sx * qx - sin_r * sy * qy + pvx + tx,
        sin_r * sx * qx + cos_r * sy * qy + pvy + ty,
    )
}

/// Transform (px, py) from parent space into node n's local space.
/// Pivot is expressed in absolute local-space coordinates (pivot_x * width, pivot_y * height).
fn into_node_pos(
    tx: f64,
    ty: f64,
    sx: f64,
    sy: f64,
    cos_r: f64,
    sin_r: f64,
    pvx: f64,
    pvy: f64,
    px: f64,
    py: f64,
) -> (f64, f64) {
    let qx = px - pvx - tx;
    let qy = py - pvy - ty;
    (
        (cos_r * qx + sin_r * qy) / sx + pvx,
        (-sin_r * qx + cos_r * qy) / sy + pvy,
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
        let (tx, ty, sx, sy, cos_r, sin_r, pvx, pvy) = group_transform(node, ctx)?;
        (px, py) = from_node_pos(tx, ty, sx, sy, cos_r, sin_r, pvx, pvy, px, py);
    }

    // Go down: each node transforms from parent space into its local space
    for node_id in ct {
        let node = ctx.node(node_id)?;
        let (tx, ty, sx, sy, cos_r, sin_r, pvx, pvy) = group_transform(node, ctx)?;
        (px, py) = into_node_pos(tx, ty, sx, sy, cos_r, sin_r, pvx, pvy, px, py);
    }

    Ok((px, py))
}

// ───────────────────────────── Expr / Call ──────────────────────────────────

impl<T: Value + DeserializeOwned + Clone> Eval<T> for AttrExpr<T> {
    fn eval<'a>(&'a self, ctx: &'a EvalCtx<'a>) -> anyhow::Result<T> {
        let expr = self.get_expr();
        if !ctx.begin_eval(expr) {
            return Ok(T::recursive_value());
        }
        let result = expr.eval(ctx);
        ctx.end_eval(expr);
        result
    }
}

impl<T: Value + DeserializeOwned + Debug + Clone> AttrExpr<T> {
    fn eval_as_inheritable(&self, ctx: &EvalCtx) -> anyhow::Result<Inheritable<T>> {
        Ok(match self.get_expr() {
            Expr::Inherited { .. } => Inheritable::Inherited(self.eval(ctx)?),
            _ => Inheritable::Own(self.eval(ctx)?),
        })
    }
}

impl Eval<(f64, f64)> for FloatParamsPair {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<(f64, f64)> {
        Ok((self.a.eval(ctx)?, self.b.eval(ctx)?))
    }
}

impl Eval<f64> for FloatCall {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        match self {
            FloatCall::Add(pair) => {
                tracing::trace!("Call::Add");
                let (va, vb) = pair.eval(ctx)?;
                tracing::trace!(a = va, b = vb, result = va + vb, "Call::Add result");
                Ok(va + vb)
            }
            FloatCall::Sub(pair) => {
                tracing::trace!("Call::Sub");
                let (va, vb) = pair.eval(ctx)?;
                tracing::trace!(a = va, b = vb, result = va - vb, "Call::Sub result");
                Ok(va - vb)
            }
            FloatCall::Mul(pair) => {
                tracing::trace!("Call::Mul");
                let (va, vb) = pair.eval(ctx)?;
                tracing::trace!(a = va, b = vb, result = va * vb, "Call::Mul result");
                Ok(va * vb)
            }
            FloatCall::Div(pair) => {
                tracing::trace!("Call::Div");
                let (va, vb) = pair.eval(ctx)?;
                let result = if vb.abs() < 0.000001 { 0.0 } else { va / vb };
                tracing::trace!(a = va, b = vb, result = result, "Call::Div result");
                Ok(result)
            }
            FloatCall::Norm(pair) => {
                tracing::trace!("Call::Mul");
                let (va, vb) = pair.eval(ctx)?;
                let d = va * va + vb * vb;
                let result = if d < 0.0001 { 0.0 } else { va / d.sqrt() };
                tracing::trace!(a = va, b = vb, result = result, "Call::Norm result");
                Ok(result)
            }
            FloatCall::NodeTransformX(params) => {
                tracing::trace!(
                    source = params.source.as_u64(),
                    target = params.target.as_u64(),
                    "Call::NodeTransformX"
                );
                let xv = params.x.eval(ctx)?;
                let yv = params.y.eval(ctx)?;
                let (px, _py) = node_transform(params.source, params.target, xv, yv, ctx)?;
                Ok(px)
            }
            FloatCall::NodeTransformY(params) => {
                tracing::trace!(
                    source = params.source.as_u64(),
                    target = params.target.as_u64(),
                    "Call::NodeTransformY"
                );
                let xv = params.x.eval(ctx)?;
                let yv = params.y.eval(ctx)?;
                let (_px, py) = node_transform(params.source, params.target, xv, yv, ctx)?;
                Ok(py)
            }
            FloatCall::DefaultWidth { node } => {
                let node = ctx.node(*node)?;
                Ok(node.default_width(ctx)?)
            }
            FloatCall::DefaultHeight { node } => {
                let node = ctx.node(*node)?;
                Ok(node.default_height(ctx)?)
            }
            FloatCall::DefaultX { node } => {
                let node = ctx.node(*node)?;
                Ok(node.default_x(ctx)?)
            }
            FloatCall::DefaultY { node } => {
                let node = ctx.node(*node)?;
                Ok(node.default_y(ctx)?)
            }
            FloatCall::PathX(p) => {
                let t = p.t.eval(ctx)?;
                Ok(point_in_path(ctx, p.node, t)?.0)
            }
            FloatCall::PathY(p) => {
                let t = p.t.eval(ctx)?;
                Ok(point_in_path(ctx, p.node, t)?.1)
            }
            FloatCall::PathLength { node } => path_length(ctx, *node),
        }
    }
}

// ─────────────────────────── Mixin eval impls ───────────────────────────────

impl Position {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::Position> {
        Ok(renderer_core::Position {
            x: self.x.eval(ctx)?,
            y: self.y.eval(ctx)?,
        })
    }
}

impl Size {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::Size> {
        Ok(renderer_core::Size {
            width: self.width.eval(ctx)?,
            height: self.height.eval(ctx)?,
        })
    }
}

impl Style {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::Style> {
        Ok(renderer_core::Style {
            fill_color: self.fill_color.eval(ctx)?.map(|c| c.into_inner()),
            stroke_color: self.stroke_color.eval(ctx)?.map(|c| c.into_inner()),
            stroke_width: self.stroke_width.eval(ctx)?,
            alpha: self.alpha.eval(ctx)?,
        })
    }
}

impl TextStyle {
    pub fn eval_as_inheritable(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::TextStyle> {
        Ok(renderer_core::TextStyle {
            fill_color: self
                .style
                .fill_color
                .eval_as_inheritable(ctx)?
                .map(|v| v.as_ref().map(|v| v.clone().into_inner())),
            stroke_color: self
                .style
                .stroke_color
                .eval_as_inheritable(ctx)?
                .map(|v| v.as_ref().map(|v| v.clone().into_inner())),
            stroke_width: self.style.stroke_width.eval_as_inheritable(ctx)?,
            alpha: self.style.alpha.eval_as_inheritable(ctx)?,
            font_family: self.font.eval_as_inheritable(ctx)?,
            font_size: self.font_size.eval_as_inheritable(ctx)?,
            font_weight: self.font_weight.eval_as_inheritable(ctx)?,
            italic: self.italic.eval_as_inheritable(ctx)?,
        })
    }
}

// ──────────────────────────── Node eval impls ───────────────────────────────

impl Node {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::Node> {
        let _span = tracing::trace_span!("node.eval", node_id = self.id.as_u64()).entered();
        let kind = match &self.kind {
            NodeKind::Group {
                position,
                size,
                alpha,
                rotation,
                pivot_x,
                pivot_y,
                scale_x,
                scale_y,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
                layout: _,
                children,
                z_level,
            } => renderer_core::NodeKind::Group {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                alpha: alpha.eval(ctx)?,
                rotation: rotation.eval(ctx)?,
                pivot_x: pivot_x.eval(ctx)?,
                pivot_y: pivot_y.eval(ctx)?,
                scale_x: scale_x.eval(ctx)?,
                scale_y: scale_y.eval(ctx)?,
                clip_x: clip_x.eval(ctx)?,
                clip_y: clip_y.eval(ctx)?,
                clip_w: clip_w.eval(ctx)?,
                clip_h: clip_h.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?,
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
            } => renderer_core::NodeKind::Rect {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?,
            },
            NodeKind::Ellipse {
                position,
                size,
                style,
                z_level,
            } => renderer_core::NodeKind::Ellipse {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?,
            },
            NodeKind::Path {
                style,
                z_level,
                children,
                crop_start,
                crop_end,
            } => renderer_core::NodeKind::Path {
                style: style.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?,
                crop_start: crop_start.eval(ctx)?,
                crop_end: crop_end.eval(ctx)?,
                children: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_path_cmd(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            NodeKind::Text {
                position,
                text_style,
                z_level,
                sh_language,
                sh_theme,
                children,
            } => renderer_core::NodeKind::Text {
                position: position.eval(ctx)?,
                text_style: text_style.eval_as_inheritable(ctx)?,
                sh_language: sh_language.clone(),
                sh_theme: sh_theme.clone(),
                z_level: z_level.eval_as_inheritable(ctx)?,
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
                children,
            } => renderer_core::NodeKind::Image {
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?,
                alpha: alpha.eval(ctx)?,
                path: path.eval(ctx)?,
                keep_aspect: keep_aspect.eval(ctx)?,
                layers: {
                    let mut result = Vec::new();
                    for &id in children {
                        let node = ctx.node(id)?;
                        if node.is_active(ctx.frame()) {
                            result.push(node.eval_as_image_layer(ctx)?);
                        }
                    }
                    result
                },
                hidden_layers: {
                    let mut result = Vec::new();
                    for &id in children {
                        let node = ctx.node(id)?;
                        if !node.is_active(ctx.frame())
                            && let NodeKind::Layer { layer_name, .. } = &node.kind
                        {
                            result.push(layer_name.clone());
                        }
                    }
                    result
                },
                all_svg_layers: renderer_core::svg_image_layers(&path.eval(ctx)?),
            },
            _ => anyhow::bail!(
                "path command / text-internal nodes cannot appear as scene tree nodes"
            ),
        };
        Ok(renderer_core::Node {
            id: self.id.as_u64(),
            kind,
        })
    }

    pub fn eval_as_path_cmd(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::PathCommand> {
        match &self.kind {
            NodeKind::Move { position } => Ok(renderer_core::PathCommand::Move {
                id: self.id.as_u64(),
                position: position.eval(ctx)?,
            }),
            NodeKind::Line { position } => Ok(renderer_core::PathCommand::Line {
                id: self.id.as_u64(),
                position: position.eval(ctx)?,
            }),
            NodeKind::Cubic {
                position,
                c1_x,
                c1_y,
                c2_x,
                c2_y,
            } => Ok(renderer_core::PathCommand::Cubic {
                id: self.id.as_u64(),
                position: position.eval(ctx)?,
                c1_x: c1_x.eval(ctx)?,
                c1_y: c1_y.eval(ctx)?,
                c2_x: c2_x.eval(ctx)?,
                c2_y: c2_y.eval(ctx)?,
            }),
            NodeKind::Close => Ok(renderer_core::PathCommand::Close {
                id: self.id.as_u64(),
            }),
            _ => anyhow::bail!("expected path command node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_child(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::TextChild> {
        match &self.kind {
            NodeKind::TextGroup { .. } => Ok(renderer_core::TextChild::Group(
                self.eval_as_text_group(ctx)?,
            )),
            NodeKind::TextSpan { .. } => {
                Ok(renderer_core::TextChild::Span(self.eval_as_text_span(ctx)?))
            }
            _ => anyhow::bail!("expected t_group or t_span node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_group(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::TextGroup> {
        match &self.kind {
            NodeKind::TextGroup {
                text_style,
                children,
            } => Ok(renderer_core::TextGroup {
                id: self.id.as_u64(),
                text_style: text_style.eval_as_inheritable(ctx)?,
                children: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            }),
            _ => anyhow::bail!("expected t_group node, got {:?}", self.id),
        }
    }

    pub fn eval_as_image_layer(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::ImageLayer> {
        match &self.kind {
            NodeKind::Layer {
                position,
                size,
                z_level,
                alpha,
                layer_name,
                ..
            } => Ok(renderer_core::ImageLayer {
                id: self.id.as_u64(),
                layer_name: layer_name.clone(),
                position: position.eval(ctx)?,
                size: size.eval(ctx)?,
                z_level: z_level.eval_as_inheritable(ctx)?,
                alpha: alpha.eval(ctx)?,
            }),
            _ => anyhow::bail!("expected layer node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_span(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::TextSpan> {
        match &self.kind {
            NodeKind::TextSpan { text_style, text } => Ok(renderer_core::TextSpan {
                id: self.id.as_u64(),
                text: text.eval(ctx)?,
                text_style: text_style.eval_as_inheritable(ctx)?,
            }),
            _ => anyhow::bail!("expected TextSpan node, got {:?}", self.id),
        }
    }
}

// ─────────────────────────── SceneDef eval impl ─────────────────────────────

impl SceneDef {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::Scene> {
        let _span = tracing::debug_span!("frame", frame = ctx.frame().as_u32()).entered();
        let fill_color = self
            .fill_color
            .eval(ctx)?
            .map(|x| x.into_inner())
            .unwrap_or_default();
        let mut children = Vec::with_capacity(self.children.len());
        for &id in &self.children {
            let node = ctx.node(id)?;
            if node.is_active(ctx.frame()) {
                children.push(node.eval(ctx)?);
            }
        }
        Ok(renderer_core::Scene {
            width: self.size.width.eval(ctx)?,
            height: self.size.height.eval(ctx)?,
            fill_color,
            children,
        })
    }
}
