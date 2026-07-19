use crate::FrameId;
use crate::basictypes::NodeId;
use crate::nodes::{AttrExpr, Node, NodeKind, Position, SceneDef, Size, Style, TextStyle};
use crate::paths::{path_length, point_in_path};
use crate::values::{Color, Eval, Expr, FloatCall, FloatParamsPair, Value};
use renderer_core::{Inheritable, Position as RcPosition, Size as RcSize};
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
        self.scene_def.size.width.eval_or(self, 0.0)
    }

    pub fn scene_height(&self) -> anyhow::Result<f64> {
        self.scene_def.size.height.eval_or(self, 0.0)
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
///
/// `NodeId::SCENE` (and, defensively, any other id absent from `ctx.nodes`) resolves
/// to an empty chain: the `Scene` has no position/rotation/scale of its own, so "no
/// group transforms to apply" is exactly the correct root/identity frame for it.
fn ancestor_chain(ctx: &EvalCtx, node_id: NodeId) -> Vec<NodeId> {
    if node_id == NodeId::SCENE {
        return Vec::new();
    }
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

struct GroupTransform {
    pos: RcPosition,
    scale: RcSize,
    cos_r: f64,
    sin_r: f64,
    pivot: RcPosition,
}

impl GroupTransform {
    /// Transform `local` from this group's local space into its parent space.
    fn to_parent(&self, local: RcPosition) -> RcPosition {
        let qx = local.x - self.pivot.x;
        let qy = local.y - self.pivot.y;
        RcPosition::new(
            self.cos_r * self.scale.width * qx - self.sin_r * self.scale.height * qy
                + self.pivot.x
                + self.pos.x,
            self.sin_r * self.scale.width * qx
                + self.cos_r * self.scale.height * qy
                + self.pivot.y
                + self.pos.y,
        )
    }

    /// Transform `parent_pt` from the parent space into this group's local space.
    fn to_local(&self, parent_pt: RcPosition) -> RcPosition {
        let qx = parent_pt.x - self.pivot.x - self.pos.x;
        let qy = parent_pt.y - self.pivot.y - self.pos.y;
        RcPosition::new(
            (self.cos_r * qx + self.sin_r * qy) / self.scale.width + self.pivot.x,
            (-self.sin_r * qx + self.cos_r * qy) / self.scale.height + self.pivot.y,
        )
    }
}

fn group_transform(node: &Node, ctx: &EvalCtx) -> anyhow::Result<GroupTransform> {
    match &node.kind {
        NodeKind::Group {
            scale_x,
            scale_y,
            rotation,
            pivot_x,
            pivot_y,
            ..
        } => {
            let pos = RcPosition::new(node.get_x(ctx)?, node.get_y(ctx)?);
            let scale = RcSize {
                width: scale_x.eval_or(ctx, 1.0)?,
                height: scale_y.eval_or(ctx, 1.0)?,
            };
            let r = rotation.eval_or(ctx, 0.0)?.to_radians();
            let w = node.get_width(ctx)?;
            let h = node.get_height(ctx)?;
            let pivot = RcPosition::new(
                pivot_x.eval_or(ctx, 0.5)? * w,
                pivot_y.eval_or(ctx, 0.5)? * h,
            );
            Ok(GroupTransform {
                pos,
                scale,
                cos_r: r.cos(),
                sin_r: r.sin(),
                pivot,
            })
        }
        _ => anyhow::bail!("node {:?} has no group transform (not a group)", node.id),
    }
}

/// Transform `pt` from source node's local coordinate space into target node's local
/// space. Either (or both) of `source`/`target` may be `NodeId::SCENE`, meaning the
/// top-level scene/root frame — see `ancestor_chain`. When both are `NodeId::SCENE`
/// the `source == target` fast path below applies (no transform needed).
fn node_transform(
    source: NodeId,
    target: NodeId,
    mut pt: RcPosition,
    ctx: &EvalCtx,
) -> anyhow::Result<RcPosition> {
    let _span = tracing::trace_span!(
        "node_transform",
        source = source.as_u64(),
        target = target.as_u64(),
        x = pt.x,
        y = pt.y
    )
    .entered();
    if source == target {
        return Ok(pt);
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

    // Go up: each node transforms from its local space to its parent's space
    for node_id in cs {
        let node = ctx.node(node_id)?;
        pt = group_transform(node, ctx)?.to_parent(pt);
    }

    // Go down: each node transforms from parent space into its local space
    for node_id in ct {
        let node = ctx.node(node_id)?;
        pt = group_transform(node, ctx)?.to_local(pt);
    }

    Ok(pt)
}

// ───────────────────────────── Expr / Call ──────────────────────────────────

impl<T: Value + DeserializeOwned + Debug + Clone> AttrExpr<T> {
    /// Evaluate this attribute, falling back to `default` when absent. For
    /// literal-default fields only (alpha, rotation, scale, pivot, clip,
    /// crop, c1/c2, keep_aspect, ...) — position/size use `Node::get_x` etc.
    /// (auto-layout fallback) and inheritable style/z fields use
    /// `eval_inherited` (parent-chain fallback) instead.
    pub fn eval_or<'a>(&'a self, ctx: &'a EvalCtx<'a>, default: T) -> anyhow::Result<T> {
        match self.get_expr() {
            None => Ok(default),
            Some(expr) => {
                if !ctx.begin_eval(expr) {
                    return Ok(T::recursive_value());
                }
                let result = expr.eval(ctx);
                ctx.end_eval(expr);
                result
            }
        }
    }
}

/// Resolve an inheritable attribute: if `node` has its own value, use it (`Own`);
/// otherwise walk `node.parent` until an ancestor has one, or fall back to
/// `root_default` at the top of the tree (`Inherited`). This reproduces v1's
/// `Inherited(...)` expression-chain semantics, which is now represented purely
/// by attribute *absence* instead of an explicit wrapper (`api-v2-impl.md` §A.4).
fn eval_inherited<'a, T, F>(
    ctx: &'a EvalCtx<'a>,
    node: &'a Node,
    get: F,
    root_default: T,
) -> anyhow::Result<Inheritable<T>>
where
    T: Value + DeserializeOwned + Debug + Clone + 'a,
    F: Fn(&'a NodeKind) -> Option<&'a AttrExpr<T>>,
{
    let mut current = node;
    let mut own = true;
    loop {
        if let Some(expr) = get(&current.kind).and_then(|a| a.get_expr()) {
            if !ctx.begin_eval(expr) {
                return Ok(Inheritable::Own(T::recursive_value()));
            }
            let v = expr.eval(ctx);
            ctx.end_eval(expr);
            let v = v?;
            return Ok(if own {
                Inheritable::Own(v)
            } else {
                Inheritable::Inherited(v)
            });
        }
        own = false;
        match current.parent {
            Some(pid) => current = ctx.node(pid)?,
            None => return Ok(Inheritable::Inherited(root_default)),
        }
    }
}

fn z_level_of(kind: &NodeKind) -> Option<&AttrExpr<f64>> {
    match kind {
        NodeKind::Group { z_level, .. }
        | NodeKind::Rect { z_level, .. }
        | NodeKind::Ellipse { z_level, .. }
        | NodeKind::Path { z_level, .. }
        | NodeKind::Text { z_level, .. }
        | NodeKind::Image { z_level, .. }
        | NodeKind::Layer { z_level, .. } => Some(z_level),
        _ => None,
    }
}

fn eval_z(ctx: &EvalCtx, node: &Node) -> anyhow::Result<Inheritable<f64>> {
    eval_inherited(ctx, node, z_level_of, 0.0)
}

fn text_style_of(kind: &NodeKind) -> Option<&TextStyle> {
    match kind {
        NodeKind::Text { text_style, .. }
        | NodeKind::TextGroup { text_style, .. }
        | NodeKind::TextSpan { text_style, .. } => Some(text_style),
        _ => None,
    }
}

/// Like `eval_inherited`, but for `TextStyle` fields specifically: the walk
/// must stop at the nearest `Text` ancestor rather than continuing past it.
/// `Text`'s own style is seeded with *literal* defaults in Python
/// (`_init_text_style`, own `_add_attr`), not `_add_from_parent` like
/// `TextGroup`/`TextSpan` — so an absent field there is already the terminal
/// value, not a cue to keep climbing into its (non-text) `Group` parent.
fn eval_text_inherited<'a, T, F>(
    ctx: &'a EvalCtx<'a>,
    node: &'a Node,
    get: F,
    root_default: T,
) -> anyhow::Result<Inheritable<T>>
where
    T: Value + DeserializeOwned + Debug + Clone + 'a,
    F: Fn(&'a TextStyle) -> &'a AttrExpr<T>,
{
    let mut current = node;
    let mut own = true;
    loop {
        let Some(ts) = text_style_of(&current.kind) else {
            // Structurally unreachable (t_group/t_span always chain up to a
            // Text ancestor) — fall back to the literal default defensively.
            return Ok(terminal(own, root_default));
        };
        if let Some(expr) = get(ts).get_expr() {
            if !ctx.begin_eval(expr) {
                return Ok(Inheritable::Own(T::recursive_value()));
            }
            let v = expr.eval(ctx);
            ctx.end_eval(expr);
            return Ok(terminal(own, v?));
        }
        if matches!(current.kind, NodeKind::Text { .. }) {
            return Ok(terminal(own, root_default));
        }
        own = false;
        match current.parent {
            Some(pid) => current = ctx.node(pid)?,
            None => return Ok(Inheritable::Inherited(root_default)),
        }
    }
}

fn terminal<T: Debug + Clone>(own: bool, v: T) -> Inheritable<T> {
    if own {
        Inheritable::Own(v)
    } else {
        Inheritable::Inherited(v)
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
                let (va, vb) = pair.eval(ctx)?;
                Ok(va + vb)
            }
            FloatCall::Sub(pair) => {
                let (va, vb) = pair.eval(ctx)?;
                Ok(va - vb)
            }
            FloatCall::Mul(pair) => {
                let (va, vb) = pair.eval(ctx)?;
                Ok(va * vb)
            }
            FloatCall::Div(pair) => {
                let (va, vb) = pair.eval(ctx)?;
                Ok(if vb.abs() < 0.000001 { 0.0 } else { va / vb })
            }
            FloatCall::Norm(pair) => {
                let (va, vb) = pair.eval(ctx)?;
                let d = va * va + vb * vb;
                Ok(if d < 0.0001 { 0.0 } else { va / d.sqrt() })
            }
            FloatCall::NodeTransformX(params) => {
                let xv = params.x.eval(ctx)?;
                let yv = params.y.eval(ctx)?;
                Ok(node_transform(params.source, params.target, RcPosition::new(xv, yv), ctx)?.x)
            }
            FloatCall::NodeTransformY(params) => {
                let xv = params.x.eval(ctx)?;
                let yv = params.y.eval(ctx)?;
                Ok(node_transform(params.source, params.target, RcPosition::new(xv, yv), ctx)?.y)
            }
            FloatCall::DefaultWidth { node } => {
                if *node == NodeId::SCENE {
                    ctx.scene_width()
                } else {
                    ctx.node(*node)?.default_width(ctx)
                }
            }
            FloatCall::DefaultHeight { node } => {
                if *node == NodeId::SCENE {
                    ctx.scene_height()
                } else {
                    ctx.node(*node)?.default_height(ctx)
                }
            }
            FloatCall::DefaultX { node } => {
                if *node == NodeId::SCENE {
                    Ok(0.0)
                } else {
                    ctx.node(*node)?.default_x(ctx)
                }
            }
            FloatCall::DefaultY { node } => {
                if *node == NodeId::SCENE {
                    Ok(0.0)
                } else {
                    ctx.node(*node)?.default_y(ctx)
                }
            }
            FloatCall::PathX(p) => {
                let t = p.t.eval(ctx)?;
                Ok(point_in_path(ctx, p.node, t)?.x)
            }
            FloatCall::PathY(p) => {
                let t = p.t.eval(ctx)?;
                Ok(point_in_path(ctx, p.node, t)?.y)
            }
            FloatCall::PathLength { node } => path_length(ctx, *node),
        }
    }
}

// ─────────────────────────── Mixin eval impls ───────────────────────────────

impl Position {
    /// `owner` is the node this `Position` belongs to — needed to resolve the
    /// auto-layout default (`Node::default_x/default_y`) when an axis is absent.
    pub fn eval(&self, ctx: &EvalCtx, owner: &Node) -> anyhow::Result<renderer_core::Position> {
        Ok(renderer_core::Position {
            x: match self.x.get_expr() {
                Some(e) => e.eval(ctx)?,
                None => owner.default_x(ctx)?,
            },
            y: match self.y.get_expr() {
                Some(e) => e.eval(ctx)?,
                None => owner.default_y(ctx)?,
            },
        })
    }
}

impl Size {
    /// `owner` is the node this `Size` belongs to — needed to resolve the
    /// auto-layout default (`Node::default_width/default_height`) when absent.
    pub fn eval(&self, ctx: &EvalCtx, owner: &Node) -> anyhow::Result<renderer_core::Size> {
        Ok(renderer_core::Size {
            width: match self.width.get_expr() {
                Some(e) => e.eval(ctx)?,
                None => owner.default_width(ctx)?,
            },
            height: match self.height.get_expr() {
                Some(e) => e.eval(ctx)?,
                None => owner.default_height(ctx)?,
            },
        })
    }
}

impl Style {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::Style> {
        Ok(renderer_core::Style {
            fill_color: self
                .fill_color
                .eval_or(ctx, Color::recursive_value())?
                .into_inner(),
            stroke_color: self
                .stroke_color
                .eval_or(ctx, Color::recursive_value())?
                .into_inner(),
            stroke_width: self.stroke_width.eval_or(ctx, 1.0)?,
            alpha: self.alpha.eval_or(ctx, 1.0)?,
        })
    }
}

impl TextStyle {
    pub fn eval_as_inheritable(
        &self,
        ctx: &EvalCtx,
        owner: &Node,
    ) -> anyhow::Result<renderer_core::TextStyle> {
        // `self` is `owner`'s own text_style; fields are (re-)read from `owner`
        // via `text_style_of` so the walk-up-parents fallback shares one code path.
        Ok(renderer_core::TextStyle {
            fill_color: eval_text_inherited(
                ctx,
                owner,
                |ts| &ts.fill_color,
                Color::recursive_value(),
            )?
            .map(|v| v.clone().into_inner()),
            stroke_color: eval_text_inherited(
                ctx,
                owner,
                |ts| &ts.stroke_color,
                Color::recursive_value(),
            )?
            .map(|v| v.clone().into_inner()),
            stroke_width: eval_text_inherited(ctx, owner, |ts| &ts.stroke_width, 1.0)?,
            alpha: eval_text_inherited(ctx, owner, |ts| &ts.alpha, 1.0)?,
            font_family: eval_text_inherited(
                ctx,
                owner,
                |ts| &ts.font,
                std::sync::Arc::new("sans-serif".to_string()),
            )?,
            font_size: eval_text_inherited(ctx, owner, |ts| &ts.font_size, 16.0)?,
            font_weight: eval_text_inherited(ctx, owner, |ts| &ts.font_weight, 400.0)?,
            italic: eval_text_inherited(ctx, owner, |ts| &ts.italic, false)?,
        })
    }
}

// ──────────────────────────── Node eval impls ───────────────────────────────

impl Node {
    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::Node> {
        let _span = tracing::trace_span!("node.eval", node_id = self.id.as_u64()).entered();
        let kind = match &self.kind {
            NodeKind::Group {
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
                ..
            } => renderer_core::NodeKind::Group {
                position: RcPosition::new(self.get_x(ctx)?, self.get_y(ctx)?),
                size: RcSize {
                    width: self.get_width(ctx)?,
                    height: self.get_height(ctx)?,
                },
                alpha: alpha.eval_or(ctx, 1.0)?,
                rotation: rotation.eval_or(ctx, 0.0)?,
                pivot_x: pivot_x.eval_or(ctx, 0.5)?,
                pivot_y: pivot_y.eval_or(ctx, 0.5)?,
                scale_x: scale_x.eval_or(ctx, 1.0)?,
                scale_y: scale_y.eval_or(ctx, 1.0)?,
                clip_x: clip_x.eval_or(ctx, 0.0)?,
                clip_y: clip_y.eval_or(ctx, 0.0)?,
                clip_w: clip_w.eval_or(ctx, 1.0)?,
                clip_h: clip_h.eval_or(ctx, 1.0)?,
                z_level: eval_z(ctx, self)?,
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
                ..
            } => renderer_core::NodeKind::Rect {
                position: position.eval(ctx, self)?,
                size: size.eval(ctx, self)?,
                style: style.eval(ctx)?,
                z_level: eval_z(ctx, self)?,
            },
            NodeKind::Ellipse {
                position,
                size,
                style,
                ..
            } => renderer_core::NodeKind::Ellipse {
                position: position.eval(ctx, self)?,
                size: size.eval(ctx, self)?,
                style: style.eval(ctx)?,
                z_level: eval_z(ctx, self)?,
            },
            NodeKind::Path {
                style,
                children,
                crop_start,
                crop_end,
                ..
            } => renderer_core::NodeKind::Path {
                style: style.eval(ctx)?,
                z_level: eval_z(ctx, self)?,
                crop_start: crop_start.eval_or(ctx, 0.0)?,
                crop_end: crop_end.eval_or(ctx, 1.0)?,
                children: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_path_cmd(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            NodeKind::Text {
                position,
                text_style,
                sh_language,
                sh_theme,
                children,
                ..
            } => renderer_core::NodeKind::Text {
                position: position.eval(ctx, self)?,
                text_style: text_style.eval_as_inheritable(ctx, self)?,
                sh_language: sh_language.clone(),
                sh_theme: sh_theme.clone(),
                z_level: eval_z(ctx, self)?,
                lines: children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            },
            NodeKind::Image {
                position,
                size,
                alpha,
                path,
                keep_aspect,
                children,
                ..
            } => renderer_core::NodeKind::Image {
                position: position.eval(ctx, self)?,
                size: size.eval(ctx, self)?,
                z_level: eval_z(ctx, self)?,
                alpha: alpha.eval_or(ctx, 1.0)?,
                path: path.eval_or(ctx, std::sync::Arc::new(String::new()))?,
                keep_aspect: keep_aspect.eval_or(ctx, true)?,
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
                all_svg_layers: renderer_core::svg_image_layers(
                    &path.eval_or(ctx, std::sync::Arc::new(String::new()))?,
                ),
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
                position: position.eval(ctx, self)?,
            }),
            NodeKind::Line { position } => Ok(renderer_core::PathCommand::Line {
                id: self.id.as_u64(),
                position: position.eval(ctx, self)?,
            }),
            NodeKind::Cubic {
                position,
                c1_x,
                c1_y,
                c2_x,
                c2_y,
            } => Ok(renderer_core::PathCommand::Cubic {
                id: self.id.as_u64(),
                position: position.eval(ctx, self)?,
                c1_x: c1_x.eval_or(ctx, 0.0)?,
                c1_y: c1_y.eval_or(ctx, 0.0)?,
                c2_x: c2_x.eval_or(ctx, 0.0)?,
                c2_y: c2_y.eval_or(ctx, 0.0)?,
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
                text_style: text_style.eval_as_inheritable(ctx, self)?,
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
                alpha,
                layer_name,
                ..
            } => Ok(renderer_core::ImageLayer {
                id: self.id.as_u64(),
                layer_name: layer_name.clone(),
                position: position.eval(ctx, self)?,
                size: size.eval(ctx, self)?,
                z_level: eval_z(ctx, self)?,
                alpha: alpha.eval_or(ctx, 1.0)?,
            }),
            _ => anyhow::bail!("expected layer node, got {:?}", self.id),
        }
    }

    pub fn eval_as_text_span(&self, ctx: &EvalCtx) -> anyhow::Result<renderer_core::TextSpan> {
        match &self.kind {
            NodeKind::TextSpan { text_style, text } => Ok(renderer_core::TextSpan {
                id: self.id.as_u64(),
                text: text.eval_or(ctx, std::sync::Arc::new(String::new()))?,
                text_style: text_style.eval_as_inheritable(ctx, self)?,
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
            .eval_or(ctx, Color::recursive_value())?
            .into_inner();
        let mut children = Vec::with_capacity(self.children.len());
        for &id in &self.children {
            let node = ctx.node(id)?;
            if node.is_active(ctx.frame()) {
                children.push(node.eval(ctx)?);
            }
        }
        Ok(renderer_core::Scene {
            width: self.size.width.eval_or(ctx, 0.0)?,
            height: self.size.height.eval_or(ctx, 0.0)?,
            fill_color,
            children,
        })
    }
}
