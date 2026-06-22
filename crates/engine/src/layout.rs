use crate::eval::EvalCtx;
use crate::nodes::{Layout, Node, NodeKind, Position, Size};
use crate::values::Eval;
use renderer_core::Position as RcPosition;

impl Node {
    pub fn get_position(&self) -> Option<&Position> {
        match &self.kind {
            NodeKind::Group { position, .. }
            | NodeKind::Rect { position, .. }
            | NodeKind::Ellipse { position, .. }
            | NodeKind::Move { position, .. }
            | NodeKind::Line { position, .. }
            | NodeKind::Text { position, .. }
            | NodeKind::Image { position, .. }
            | NodeKind::Layer { position, .. }
            | NodeKind::Cubic { position, .. } => Some(position),
            NodeKind::TextGroup { .. }
            | NodeKind::TextSpan { .. }
            | NodeKind::Path { .. }
            | NodeKind::Close => None,
        }
    }

    pub fn get_size(&self) -> Option<&Size> {
        match &self.kind {
            NodeKind::Group { size, .. }
            | NodeKind::Rect { size, .. }
            | NodeKind::Ellipse { size, .. }
            | NodeKind::Image { size, .. }
            | NodeKind::Layer { size, .. } => Some(size),
            NodeKind::Text { .. }
            | NodeKind::Move { .. }
            | NodeKind::Line { .. }
            | NodeKind::Cubic { .. }
            | NodeKind::TextGroup { .. }
            | NodeKind::TextSpan { .. }
            | NodeKind::Path { .. }
            | NodeKind::Close => None,
        }
    }

    pub fn get_x(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(p) = self.get_position() else {
            return Ok(0.0);
        };
        p.x.eval(ctx)
    }

    pub fn get_y(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(p) = self.get_position() else {
            return Ok(0.0);
        };
        p.y.eval(ctx)
    }

    pub fn get_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(s) = self.get_size() else {
            return self.default_width(ctx);
        };
        s.width.eval(ctx)
    }

    pub fn get_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(s) = self.get_size() else {
            return self.default_height(ctx);
        };
        s.height.eval(ctx)
    }

    /// Returns the position of the AABB's top-left corner relative to the node's
    /// own (x, y) position in the parent space.
    /// For non-group nodes (no rotation/scale) this is always (0, 0).
    pub fn aabb_offset(&self, ctx: &EvalCtx) -> anyhow::Result<RcPosition> {
        match &self.kind {
            NodeKind::Group {
                size,
                scale_x,
                scale_y,
                rotation,
                pivot_x,
                pivot_y,
                ..
            } => {
                let w = size.width.eval(ctx)?;
                let h = size.height.eval(ctx)?;
                let sx = scale_x.eval(ctx)?;
                let sy = scale_y.eval(ctx)?;
                let r = rotation.eval(ctx)?.to_radians();
                let pvx = pivot_x.eval(ctx)? * w;
                let pvy = pivot_y.eval(ctx)? * h;

                // final_x = pvx + cx*(lx−pvx) + dx*(ly−pvy)  where cx=cos r·sx, dx=−sin r·sy
                // min over lx ∈ {0,w}: if cx≥0 → cx*(0−pvx) = −cx*pvx, else cx*(w−pvx)
                // min over ly ∈ {0,h}: if dx≥0 → dx*(0−pvy) = −dx*pvy, else dx*(h−pvy)
                let cx = r.cos() * sx;
                let dx = -r.sin() * sy;
                let min_x = pvx
                    + (if cx >= 0.0 { -cx * pvx } else { cx * (w - pvx) })
                    + (if dx >= 0.0 { -dx * pvy } else { dx * (h - pvy) });

                let cy = r.sin() * sx;
                let dy = r.cos() * sy;
                let min_y = pvy
                    + (if cy >= 0.0 { -cy * pvx } else { cy * (w - pvx) })
                    + (if dy >= 0.0 { -dy * pvy } else { dy * (h - pvy) });

                Ok(RcPosition::new(min_x, min_y))
            }
            _ => Ok(RcPosition::new(0.0, 0.0)),
        }
    }

    pub fn get_outer_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let width = self.get_width(ctx)?;
        Ok(match &self.kind {
            NodeKind::Group {
                scale_x,
                scale_y,
                rotation,
                ..
            } => {
                let sx = scale_x.eval(ctx)?;
                let sy = scale_y.eval(ctx)?;
                let r = rotation.eval(ctx)?.to_radians();
                let height = self.get_height(ctx)?;
                r.cos().abs() * sx * width + r.sin().abs() * sy * height
            }
            _ => width,
        })
    }

    pub fn get_outer_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let height = self.get_height(ctx)?;
        Ok(match &self.kind {
            NodeKind::Group {
                scale_x,
                scale_y,
                rotation,
                ..
            } => {
                let sx = scale_x.eval(ctx)?;
                let sy = scale_y.eval(ctx)?;
                let r = rotation.eval(ctx)?.to_radians();
                let width = self.get_width(ctx)?;
                r.sin().abs() * sx * width + r.cos().abs() * sy * height
            }
            _ => height,
        })
    }

    pub fn get_parent_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        if let Some(parent) = self.parent {
            let node = ctx.node(parent)?;
            node.get_width(ctx)
        } else {
            ctx.scene_width()
        }
    }

    pub fn get_parent_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        if let Some(parent) = self.parent {
            let node = ctx.node(parent)?;
            node.get_height(ctx)
        } else {
            ctx.scene_height()
        }
    }

    pub fn parent_layout<'a>(&self, ctx: &'a EvalCtx) -> anyhow::Result<&'a Layout> {
        if let Some(parent) = self.parent {
            let node = ctx.node(parent)?;
            match &node.kind {
                NodeKind::Group { layout, .. } => Ok(layout),
                _ => unreachable!(),
            }
        } else {
            Ok(&Layout::Center)
        }
    }

    pub fn default_x(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match self.kind {
            // Layers live inside an Image, not a Group, so layout doesn't apply.
            NodeKind::Layer { .. } | NodeKind::Close => 0.0,
            NodeKind::Group { .. }
            | NodeKind::Rect { .. }
            | NodeKind::Ellipse { .. }
            | NodeKind::Text { .. }
            | NodeKind::Image { .. } => {
                let off = self.aabb_offset(ctx)?;
                match self.parent_layout(ctx)? {
                    Layout::Center => {
                        let parent_w = self.get_parent_width(ctx)?;
                        let self_w = self.get_outer_width(ctx)?;
                        (parent_w - self_w) / 2.0 - off.x
                    }
                    Layout::Column { align, .. } => {
                        let parent_w = self.get_parent_width(ctx)?;
                        let self_w = self.get_outer_width(ctx)?;
                        (parent_w - self_w) * align.eval(ctx)? - off.x
                    }
                    Layout::Row { gap, reserve, .. } => {
                        let parent = ctx.node(self.parent.unwrap())?;
                        let NodeKind::Group { children, .. } = &parent.kind else {
                            unreachable!()
                        };
                        let mut x = 0.0f64;
                        let gap = gap.eval(ctx)?;
                        for child in children {
                            if *child == self.id {
                                return Ok(x - off.x);
                            }
                            let node = ctx.node(*child)?;
                            if *reserve || node.is_active(ctx.frame()) {
                                x += gap + node.get_outer_width(ctx)?;
                            }
                        }
                        0.0
                    }
                }
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                text_default_pos(self, ctx)?.0 as f64
            }
            NodeKind::Path { .. }
            | NodeKind::Move { .. }
            | NodeKind::Line { .. }
            | NodeKind::Cubic { .. } => 0.0,
        })
    }

    pub fn default_y(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match self.kind {
            // Layers live inside an Image, not a Group, so layout doesn't apply.
            NodeKind::Layer { .. } | NodeKind::Close => 0.0,
            NodeKind::Group { .. }
            | NodeKind::Rect { .. }
            | NodeKind::Ellipse { .. }
            | NodeKind::Text { .. }
            | NodeKind::Image { .. } => {
                let off = self.aabb_offset(ctx)?;
                match self.parent_layout(ctx)? {
                    Layout::Center => {
                        let parent_h = self.get_parent_height(ctx)?;
                        let self_h = self.get_outer_height(ctx)?;
                        (parent_h - self_h) / 2.0 - off.y
                    }
                    Layout::Column { gap, reserve, .. } => {
                        let parent = ctx.node(self.parent.unwrap())?;
                        let NodeKind::Group { children, .. } = &parent.kind else {
                            unreachable!()
                        };
                        let mut y = 0.0f64;
                        let gap = gap.eval(ctx)?;
                        for child in children {
                            if *child == self.id {
                                return Ok(y - off.y);
                            }
                            let node = ctx.node(*child)?;
                            if *reserve || node.is_active(ctx.frame()) {
                                y += gap + node.get_outer_height(ctx)?;
                            }
                        }
                        0.0
                    }
                    Layout::Row { align, .. } => {
                        let parent_h = self.get_parent_height(ctx)?;
                        let self_h = self.get_outer_height(ctx)?;
                        (parent_h - self_h) * align.eval(ctx)? - off.y
                    }
                }
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                text_default_pos(self, ctx)?.1 as f64
            }
            NodeKind::Path { .. }
            | NodeKind::Move { .. }
            | NodeKind::Line { .. }
            | NodeKind::Cubic { .. } => 0.0,
        })
    }

    pub fn default_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match &self.kind {
            NodeKind::Layer { .. } => {
                // Return the natural SVG width from the parent Image.
                let Some(parent_id) = self.parent else {
                    return Ok(0.0);
                };
                let parent = ctx.node(parent_id)?;
                let NodeKind::Image { path, .. } = &parent.kind else {
                    return Ok(0.0);
                };
                let path_val = path.eval(ctx)?;
                let Some((nw, _nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                nw as f64
            }
            NodeKind::Group {
                children, layout, ..
            } => match layout {
                Layout::Center | Layout::Column { .. } => {
                    let mut w = 0.0f64;
                    for child in children {
                        let node = ctx.node(*child)?;
                        w = w.max(node.get_width(ctx)?);
                    }
                    w
                }
                Layout::Row { gap, reserve, .. } => {
                    let mut w = 0.0f64;
                    let mut count: u32 = 0;
                    for child in children {
                        let node = ctx.node(*child)?;
                        if *reserve || node.is_active(ctx.frame()) {
                            w += node.get_width(ctx)?;
                            count += 1;
                        }
                    }
                    if count > 0 {
                        w += (count - 1) as f64 * gap.eval(ctx)?;
                    }
                    w
                }
            },
            NodeKind::Text { children, .. } => {
                let lines = children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                renderer_core::measure_text(&lines).0 as f64
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                let child = self.eval_as_text_child(ctx)?;
                renderer_core::measure_text(&[child]).0 as f64
            }
            NodeKind::Image { path, size, .. } => {
                let path_val = path.eval(ctx)?;
                let Some((nw, nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                if size.height.get_expr().is_default_height_of(self.id) {
                    // Both dimensions are defaults → return natural width.
                    nw as f64
                } else {
                    // Height is explicit → scale width to preserve aspect ratio.
                    let h = size.height.eval(ctx)?;
                    if nh == 0.0 {
                        0.0
                    } else {
                        h * nw as f64 / nh as f64
                    }
                }
            }
            _ => 0.0,
        })
    }

    pub fn default_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match &self.kind {
            NodeKind::Layer { .. } => {
                // Return the natural SVG height from the parent Image.
                let Some(parent_id) = self.parent else {
                    return Ok(0.0);
                };
                let parent = ctx.node(parent_id)?;
                let NodeKind::Image { path, .. } = &parent.kind else {
                    return Ok(0.0);
                };
                let path_val = path.eval(ctx)?;
                let Some((_nw, nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                nh as f64
            }
            NodeKind::Group {
                children, layout, ..
            } => match layout {
                Layout::Center | Layout::Row { .. } => {
                    let mut h = 0.0f64;
                    for child in children {
                        let node = ctx.node(*child)?;
                        h = h.max(node.get_height(ctx)?);
                    }
                    h
                }
                Layout::Column { gap, reserve, .. } => {
                    let mut h = 0.0f64;
                    let mut count: u32 = 0;
                    for child in children {
                        let node = ctx.node(*child)?;
                        if *reserve || node.is_active(ctx.frame()) {
                            h += node.get_height(ctx)?;
                            count += 1;
                        }
                    }
                    if count > 0 {
                        h += (count - 1) as f64 * gap.eval(ctx)?;
                    }
                    h
                }
            },
            NodeKind::Text { children, .. } => {
                let lines = children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                renderer_core::measure_text(&lines).1 as f64
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                let child = self.eval_as_text_child(ctx)?;
                renderer_core::measure_text(&[child]).1 as f64
            }
            NodeKind::Image { path, size, .. } => {
                let path_val = path.eval(ctx)?;
                let Some((nw, nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                if size.width.get_expr().is_default_width_of(self.id) {
                    // Both dimensions are defaults → return natural height.
                    nh as f64
                } else {
                    // Width is explicit → scale height to preserve aspect ratio.
                    let w = size.width.eval(ctx)?;
                    if nw == 0.0 {
                        0.0
                    } else {
                        w * nh as f64 / nw as f64
                    }
                }
            }
            _ => 0.0,
        })
    }
}

fn text_default_pos(node: &Node, ctx: &EvalCtx) -> anyhow::Result<(f32, f32)> {
    let NodeKind::Text { children, .. } = &node.text_ancestor(ctx)?.kind else {
        unreachable!()
    };
    let lines = children
        .iter()
        .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(renderer_core::measure_text_node_pos(&lines, node.id.as_u64()).unwrap_or((0.0, 0.0)))
}
/*
/// Returns the axis-aligned bounding box `(x, y, width, height)` of the immediate
/// children of a Group node, evaluated in the group's local coordinate space.
/// Returns `(0, 0, 0, 0)` for an empty group.
fn group_children_bbox(node_id: NodeId, ctx: &EvalCtx) -> anyhow::Result<(f64, f64, f64, f64)> {
    let node = ctx.node(node_id)?;
    let children = match &node.kind {
        NodeKind::Group { children, .. } => children,
        _ => anyhow::bail!("node {:?} is not a group", node_id),
    };

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let mut expand = |x: f64, y: f64, w: f64, h: f64| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    };

    for &child_id in children {
        let child = ctx.node(child_id)?;
        match &child.kind {
            NodeKind::Rect { position, size, .. }
            | NodeKind::Ellipse { position, size, .. }
            | NodeKind::Group { position, size, .. } => {
                expand(
                    position.x.eval_f64(ctx)?,
                    position.y.eval_f64(ctx)?,
                    size.width.eval_f64(ctx)?,
                    size.height.eval_f64(ctx)?,
                );
            }
            NodeKind::Text { position, .. } => {
                let x = position.x.eval_f64(ctx)?;
                let y = position.y.eval_f64(ctx)?;
                let (w, h) = text_default_size(child_id, ctx)?;
                expand(x, y, w as f64, h as f64);
            }
            NodeKind::Path { children: cmds, .. } => {
                for &cmd_id in cmds {
                    let cmd = ctx.node(cmd_id)?;
                    let pos = match &cmd.kind {
                        NodeKind::Move { position }
                        | NodeKind::Line { position }
                        | NodeKind::Cubic { position, .. } => position,
                        _ => continue,
                    };
                    expand(pos.x.eval_f64(ctx)?, pos.y.eval_f64(ctx)?, 0.0, 0.0);
                }
            }
            _ => {}
        }
    }

    if min_x == f64::INFINITY {
        return Ok((0.0, 0.0, 0.0, 0.0));
    }
    Ok((min_x, min_y, max_x - min_x, max_y - min_y))
}
 */
