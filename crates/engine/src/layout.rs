use crate::basictypes::NodeId;
use crate::eval::EvalCtx;
use crate::nodes::{Layout, Node, NodeKind, Position, Size};
use crate::values::Eval;
use renderer_core::{
    AffineTransform, Position as RcPosition, Size as RcSize, positional_transform,
};

impl Node {
    pub fn get_position(&self) -> Option<&Position> {
        match &self.kind {
            NodeKind::Group { node_box, .. }
            | NodeKind::Rect { node_box, .. }
            | NodeKind::Ellipse { node_box, .. }
            | NodeKind::Image { node_box, .. }
            | NodeKind::Layer { node_box, .. }
            | NodeKind::Text { node_box, .. }
            | NodeKind::TextGroup { node_box, .. }
            | NodeKind::TextSpan { node_box, .. } => Some(&node_box.position),
            NodeKind::Move { position, .. }
            | NodeKind::Line { position, .. }
            | NodeKind::Cubic { position, .. } => Some(position),
            NodeKind::Path { .. } | NodeKind::Close => None,
        }
    }

    /// `TextGroup`/`TextSpan` carry a `NodeBox` (reused wholesale, see the
    /// type's doc comment) but deliberately stay out of this — `size`/
    /// `z_level` aren't meaningful for a text run (no settable size; paragraph
    /// order is draw order), and routing them through here would also pull
    /// them into `NodeKind::node_box()`-driven rotation-aware AABB/world-bounds
    /// code (`aabb_offset`/`collect_world_bounds`) that hasn't been reasoned
    /// through for the text-run coordinate-space subtleties (`get_x`/`get_y`
    /// resolve in final/fit-scaled space via `text_default_pos`, while
    /// `auto_width`/`auto_height` resolve in raw/natural space) — out of scope
    /// for this slice.
    pub fn get_size(&self) -> Option<&Size> {
        match &self.kind {
            NodeKind::Group { node_box, .. }
            | NodeKind::Rect { node_box, .. }
            | NodeKind::Ellipse { node_box, .. }
            | NodeKind::Image { node_box, .. }
            | NodeKind::Layer { node_box, .. }
            | NodeKind::Text { node_box, .. } => Some(&node_box.size),
            NodeKind::Move { .. }
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
        match p.x.get_expr() {
            Some(e) => e.eval(ctx),
            None => self.auto_x(ctx),
        }
    }

    pub fn get_y(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(p) = self.get_position() else {
            return Ok(0.0);
        };
        match p.y.get_expr() {
            Some(e) => e.eval(ctx),
            None => self.auto_y(ctx),
        }
    }

    pub fn get_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(s) = self.get_size() else {
            return self.auto_width(ctx);
        };
        match s.width.get_expr() {
            Some(e) => e.eval(ctx),
            None => self.auto_width(ctx),
        }
    }

    pub fn get_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(s) = self.get_size() else {
            return self.auto_height(ctx);
        };
        match s.height.get_expr() {
            Some(e) => e.eval(ctx),
            None => self.auto_height(ctx),
        }
    }

    /// Returns the position of the AABB's top-left corner relative to the node's
    /// own (x, y) position in the parent space.
    /// For nodes without a `NodeBox` (`Path`, `Text`, path commands, text runs)
    /// this is always (0, 0), since they carry no rotation/scale/pivot.
    pub fn aabb_offset(&self, ctx: &EvalCtx) -> anyhow::Result<RcPosition> {
        match self.kind.node_box() {
            Some(node_box) => {
                let w = self.get_width(ctx)?;
                let h = self.get_height(ctx)?;
                let sx = node_box.scale_x.eval_or(ctx, 1.0)?;
                let sy = node_box.scale_y.eval_or(ctx, 1.0)?;
                let r = node_box.rotation.eval_or(ctx, 0.0)?.to_radians();
                let pvx = node_box.pivot_x.eval_or(ctx, w * 0.5)?;
                let pvy = node_box.pivot_y.eval_or(ctx, h * 0.5)?;
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
            None => Ok(RcPosition::new(0.0, 0.0)),
        }
    }

    pub fn get_outer_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let width = self.get_width(ctx)?;
        Ok(match self.kind.node_box() {
            Some(node_box) => {
                let sx = node_box.scale_x.eval_or(ctx, 1.0)?;
                let sy = node_box.scale_y.eval_or(ctx, 1.0)?;
                let r = node_box.rotation.eval_or(ctx, 0.0)?.to_radians();
                let height = self.get_height(ctx)?;
                r.cos().abs() * sx * width + r.sin().abs() * sy * height
            }
            None => width,
        })
    }

    pub fn get_outer_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let height = self.get_height(ctx)?;
        Ok(match self.kind.node_box() {
            Some(node_box) => {
                let sx = node_box.scale_x.eval_or(ctx, 1.0)?;
                let sy = node_box.scale_y.eval_or(ctx, 1.0)?;
                let r = node_box.rotation.eval_or(ctx, 0.0)?.to_radians();
                let width = self.get_width(ctx)?;
                r.sin().abs() * sx * width + r.cos().abs() * sy * height
            }
            None => height,
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

    /// Returns the evaluated `(left, top, right, bottom)` padding of this
    /// node's parent Group, or all-zero if this node has no parent (scene
    /// root).
    pub fn parent_padding(&self, ctx: &EvalCtx) -> anyhow::Result<(f64, f64, f64, f64)> {
        if let Some(parent) = self.parent {
            let node = ctx.node(parent)?;
            let NodeKind::Group { padding, .. } = &node.kind else {
                unreachable!()
            };
            Ok((
                padding.left.eval_or(ctx, 0.0)?,
                padding.top.eval_or(ctx, 0.0)?,
                padding.right.eval_or(ctx, 0.0)?,
                padding.bottom.eval_or(ctx, 0.0)?,
            ))
        } else {
            Ok((0.0, 0.0, 0.0, 0.0))
        }
    }

    pub fn auto_x(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match self.kind {
            // Layers live inside an Image, not a Group, so layout doesn't apply.
            NodeKind::Layer { .. } | NodeKind::Close => 0.0,
            NodeKind::Group { .. }
            | NodeKind::Rect { .. }
            | NodeKind::Ellipse { .. }
            | NodeKind::Text { .. }
            | NodeKind::Image { .. } => {
                let off = self.aabb_offset(ctx)?;
                let (pl, _pt, pr, _pb) = self.parent_padding(ctx)?;
                match self.parent_layout(ctx)? {
                    Layout::Center => {
                        let parent_w = self.get_parent_width(ctx)?;
                        let self_w = self.get_outer_width(ctx)?;
                        pl + (parent_w - pl - pr - self_w) / 2.0 - off.x
                    }
                    Layout::Column { align, .. } => {
                        let parent_w = self.get_parent_width(ctx)?;
                        let self_w = self.get_outer_width(ctx)?;
                        pl + (parent_w - pl - pr - self_w) * align.eval(ctx)? - off.x
                    }
                    Layout::Row { gap, reserve, .. } => {
                        let parent = ctx.node(self.parent.unwrap())?;
                        let NodeKind::Group { children, .. } = &parent.kind else {
                            unreachable!()
                        };
                        let mut x = pl;
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
                    Layout::Grid {
                        cols,
                        gap_x,
                        reserve,
                        ..
                    } => {
                        let parent = ctx.node(self.parent.unwrap())?;
                        let NodeKind::Group { children, .. } = &parent.kind else {
                            unreachable!()
                        };
                        let cols = (*cols).max(1) as usize;
                        let (col_widths, _row_heights, idx) =
                            grid_dims(ctx, children, cols, *reserve, Some(self.id), true)?;
                        match idx {
                            Some(idx) => {
                                let col = idx % cols;
                                pl + col_widths[..col].iter().sum::<f64>()
                                    + col as f64 * gap_x.eval(ctx)?
                                    - off.x
                            }
                            None => 0.0,
                        }
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

    pub fn auto_y(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match self.kind {
            // Layers live inside an Image, not a Group, so layout doesn't apply.
            NodeKind::Layer { .. } | NodeKind::Close => 0.0,
            NodeKind::Group { .. }
            | NodeKind::Rect { .. }
            | NodeKind::Ellipse { .. }
            | NodeKind::Text { .. }
            | NodeKind::Image { .. } => {
                let off = self.aabb_offset(ctx)?;
                let (_pl, pt, _pr, pb) = self.parent_padding(ctx)?;
                match self.parent_layout(ctx)? {
                    Layout::Center => {
                        let parent_h = self.get_parent_height(ctx)?;
                        let self_h = self.get_outer_height(ctx)?;
                        pt + (parent_h - pt - pb - self_h) / 2.0 - off.y
                    }
                    Layout::Column { gap, reserve, .. } => {
                        let parent = ctx.node(self.parent.unwrap())?;
                        let NodeKind::Group { children, .. } = &parent.kind else {
                            unreachable!()
                        };
                        let mut y = pt;
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
                        pt + (parent_h - pt - pb - self_h) * align.eval(ctx)? - off.y
                    }
                    Layout::Grid {
                        cols,
                        gap_y,
                        reserve,
                        ..
                    } => {
                        let parent = ctx.node(self.parent.unwrap())?;
                        let NodeKind::Group { children, .. } = &parent.kind else {
                            unreachable!()
                        };
                        let cols = (*cols).max(1) as usize;
                        let (_col_widths, row_heights, idx) =
                            grid_dims(ctx, children, cols, *reserve, Some(self.id), true)?;
                        match idx {
                            Some(idx) => {
                                let row = idx / cols;
                                pt + row_heights[..row].iter().sum::<f64>()
                                    + row as f64 * gap_y.eval(ctx)?
                                    - off.y
                            }
                            None => 0.0,
                        }
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

    pub fn auto_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
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
                let path_val = path.eval_or(ctx, std::sync::Arc::new(String::new()))?;
                let Some((nw, _nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                nw as f64
            }
            NodeKind::Group {
                children,
                layout,
                padding,
                ..
            } => {
                let content_w = match layout {
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
                    Layout::Grid {
                        cols,
                        gap_x,
                        reserve,
                        ..
                    } => {
                        let cols = (*cols).max(1) as usize;
                        let (col_widths, row_heights, _) =
                            grid_dims(ctx, children, cols, *reserve, None, false)?;
                        if row_heights.is_empty() {
                            0.0
                        } else {
                            col_widths.iter().sum::<f64>() + (cols - 1) as f64 * gap_x.eval(ctx)?
                        }
                    }
                };
                content_w + padding.left.eval_or(ctx, 0.0)? + padding.right.eval_or(ctx, 0.0)?
            }
            NodeKind::Text {
                children,
                node_box,
                wrap,
                text_align,
                ..
            } => {
                let size = &node_box.size;
                let lines = children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child_for_layout(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let (wrap_px, align) = resolve_text_wrap_align(wrap, *text_align, ctx)?;
                let (iw, ih) = renderer_core::measure_text(&lines, wrap_px, align);
                if size
                    .height
                    .get_expr()
                    .is_none_or(|e| e.is_auto_height_of(self.id))
                {
                    // Both dimensions are defaults → return natural width.
                    iw as f64
                } else {
                    // Height is explicit → scale width to preserve aspect ratio.
                    let h = size.height.get_expr().unwrap().eval(ctx)?;
                    if ih == 0.0 {
                        0.0
                    } else {
                        h * iw as f64 / ih as f64
                    }
                }
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                let child = self.eval_as_text_child_for_layout(ctx)?;
                renderer_core::measure_text(&[child], None, renderer_core::TextAlign::Left).0 as f64
            }
            NodeKind::Image { path, node_box, .. } => {
                let path_val = path.eval_or(ctx, std::sync::Arc::new(String::new()))?;
                let Some((nw, nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                if node_box
                    .size
                    .height
                    .get_expr()
                    .is_none_or(|e| e.is_auto_height_of(self.id))
                {
                    // Both dimensions are defaults → return natural width.
                    nw as f64
                } else {
                    // Height is explicit → scale width to preserve aspect ratio.
                    let h = node_box.size.height.get_expr().unwrap().eval(ctx)?;
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

    pub fn auto_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
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
                let path_val = path.eval_or(ctx, std::sync::Arc::new(String::new()))?;
                let Some((_nw, nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                nh as f64
            }
            NodeKind::Group {
                children,
                layout,
                padding,
                ..
            } => {
                let content_h = match layout {
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
                    Layout::Grid {
                        cols,
                        gap_y,
                        reserve,
                        ..
                    } => {
                        let cols = (*cols).max(1) as usize;
                        let (_col_widths, row_heights, _) =
                            grid_dims(ctx, children, cols, *reserve, None, false)?;
                        if row_heights.is_empty() {
                            0.0
                        } else {
                            row_heights.iter().sum::<f64>()
                                + (row_heights.len() - 1) as f64 * gap_y.eval(ctx)?
                        }
                    }
                };
                content_h + padding.top.eval_or(ctx, 0.0)? + padding.bottom.eval_or(ctx, 0.0)?
            }
            NodeKind::Text {
                children,
                node_box,
                wrap,
                text_align,
                ..
            } => {
                let size = &node_box.size;
                let lines = children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child_for_layout(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let (wrap_px, align) = resolve_text_wrap_align(wrap, *text_align, ctx)?;
                let (iw, ih) = renderer_core::measure_text(&lines, wrap_px, align);
                if size
                    .width
                    .get_expr()
                    .is_none_or(|e| e.is_auto_width_of(self.id))
                {
                    // Both dimensions are defaults → return natural height.
                    ih as f64
                } else {
                    // Width is explicit → scale height to preserve aspect ratio.
                    let w = size.width.get_expr().unwrap().eval(ctx)?;
                    if iw == 0.0 {
                        0.0
                    } else {
                        w * ih as f64 / iw as f64
                    }
                }
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                let child = self.eval_as_text_child_for_layout(ctx)?;
                renderer_core::measure_text(&[child], None, renderer_core::TextAlign::Left).1 as f64
            }
            NodeKind::Image { path, node_box, .. } => {
                let path_val = path.eval_or(ctx, std::sync::Arc::new(String::new()))?;
                let Some((nw, nh)) = renderer_core::measure_image(path_val.as_str()) else {
                    return Ok(0.0);
                };
                if node_box
                    .size
                    .width
                    .get_expr()
                    .is_none_or(|e| e.is_auto_width_of(self.id))
                {
                    // Both dimensions are defaults → return natural height.
                    nh as f64
                } else {
                    // Width is explicit → scale height to preserve aspect ratio.
                    let w = node_box.size.width.get_expr().unwrap().eval(ctx)?;
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

/// Content-space scale + centering offset a `Text` node applies to its laid-out
/// children when `size(w=, h=)` differs from the natural (unscaled) extent —
/// mirrors `Image`'s `keep_aspect` fit math (`render_image`/`ImagePlacement` in
/// the renderers). Reduces to `(1, 1, 0, 0)` whenever the box equals the natural
/// extent (the untouched-default case), since `auto_width`/`auto_height` already
/// resolve to that extent.
fn text_content_fit(text_node: &Node, ctx: &EvalCtx) -> anyhow::Result<(f64, f64, f64, f64)> {
    let NodeKind::Text {
        children,
        keep_aspect,
        wrap,
        text_align,
        ..
    } = &text_node.kind
    else {
        unreachable!()
    };
    let lines = children
        .iter()
        .map(|&id| ctx.node(id)?.eval_as_text_child_for_layout(ctx))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let (wrap_px, align) = resolve_text_wrap_align(wrap, *text_align, ctx)?;
    let (iw, ih) = renderer_core::measure_text(&lines, wrap_px, align);
    if iw == 0.0 || ih == 0.0 {
        return Ok((1.0, 1.0, 0.0, 0.0));
    }
    let (iw, ih) = (iw as f64, ih as f64);
    let box_w = text_node.get_width(ctx)?;
    let box_h = text_node.get_height(ctx)?;
    if keep_aspect.eval_or(ctx, true)? {
        let s = (box_w / iw).min(box_h / ih);
        Ok((s, s, (box_w - iw * s) / 2.0, (box_h - ih * s) / 2.0))
    } else {
        Ok((box_w / iw, box_h / ih, 0.0, 0.0))
    }
}

/// Raw, pre-fit-scale position of a text-run node (`TextGroup`/`TextSpan`)
/// within its owning `Text`'s laid-out paragraph — the same coordinate space
/// `auto_width`/`auto_height` already measure a run's own natural box in
/// (`renderer_core::measure_text`, no `text_content_fit` mapping applied).
/// Split out from `text_default_pos` so callers that need to compose this
/// with other *raw*-space quantities (a run's own pivot, `layout.rs`'s
/// `nearest_run_transform_component`) don't accidentally mix it with
/// `text_default_pos`'s final/fit-scaled result — the two are different units.
fn text_raw_pos(node: &Node, ctx: &EvalCtx) -> anyhow::Result<(f32, f32)> {
    let text_node = node.text_ancestor(ctx)?;
    let NodeKind::Text {
        children,
        wrap,
        text_align,
        ..
    } = &text_node.kind
    else {
        unreachable!()
    };
    let lines = children
        .iter()
        .map(|&id| ctx.node(id)?.eval_as_text_child_for_layout(ctx))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let (wrap_px, align) = resolve_text_wrap_align(wrap, *text_align, ctx)?;
    Ok(
        renderer_core::measure_text_node_pos(&lines, node.id.as_u64(), wrap_px, align)
            .unwrap_or((0.0, 0.0)),
    )
}

/// Unscaled position of a text-run node (`TextGroup`/`TextSpan`) within its
/// owning `Text`'s laid-out paragraph, then mapped through that `Text`'s
/// content-fit scale/offset (`text_content_fit`) so `word.at("right")` lands
/// on the glyph in scaled space, not the raw glyph-space one.
fn text_default_pos(node: &Node, ctx: &EvalCtx) -> anyhow::Result<(f32, f32)> {
    let text_node = node.text_ancestor(ctx)?;
    let (lx, ly) = text_raw_pos(node, ctx)?;
    let (sx, sy, off_x, off_y) = text_content_fit(text_node, ctx)?;
    Ok((
        (lx as f64 * sx + off_x) as f32,
        (ly as f64 * sy + off_y) as f32,
    ))
}

/// Which axis `nearest_position_override_delta` should resolve.
#[derive(Clone, Copy)]
pub(crate) enum PosAxis {
    X,
    Y,
}

/// Walk from `leaf` (a `TextGroup`/`TextSpan`) up through its `TextGroup` ancestors,
/// stopping at (not including) the owning `Text` block, to find the nearest
/// self-or-ancestor with an explicit override on `axis`. Returns the delta to add to
/// `leaf`'s own natural (paragraph-layout) position on that axis —
/// `override_value - that_node's_own_natural_position` — so the whole subtree under
/// the winning node moves as a rigid unit — siblings keep their places, the
/// overridden run just draws elsewhere. `None` if no ancestor-or-self
/// has that axis set — the common case, and cheap: no `text_default_pos` re-layout
/// call happens unless a winner is actually found.
pub(crate) fn nearest_position_override_delta(
    ctx: &EvalCtx,
    leaf: &Node,
    axis: PosAxis,
) -> anyhow::Result<Option<f64>> {
    let mut current = leaf;
    loop {
        let position = match &current.kind {
            NodeKind::TextGroup { node_box, .. } | NodeKind::TextSpan { node_box, .. } => {
                &node_box.position
            }
            _ => return Ok(None),
        };
        let expr = match axis {
            PosAxis::X => position.x.get_expr(),
            PosAxis::Y => position.y.get_expr(),
        };
        if let Some(expr) = expr {
            let explicit = expr.eval(ctx)?;
            let (nx, ny) = text_default_pos(current, ctx)?;
            let natural = match axis {
                PosAxis::X => nx as f64,
                PosAxis::Y => ny as f64,
            };
            return Ok(Some(explicit - natural));
        }
        match current.parent {
            Some(parent_id) => current = ctx.node(parent_id)?,
            None => return Ok(None),
        }
    }
}

pub(crate) fn nearest_run_transform_component(
    ctx: &EvalCtx,
    leaf: &Node,
) -> anyhow::Result<Option<AffineTransform>> {
    let mut current = leaf;
    loop {
        let node_box = match &current.kind {
            NodeKind::TextGroup { node_box, .. } | NodeKind::TextSpan { node_box, .. } => node_box,
            _ => return Ok(None),
        };
        let has_transform = node_box.rotation.get_expr().is_some()
            || node_box.scale_x.get_expr().is_some()
            || node_box.scale_y.get_expr().is_some()
            || node_box.pivot_x.get_expr().is_some()
            || node_box.pivot_y.get_expr().is_some();
        if has_transform {
            let w = current.auto_width(ctx)?;
            let h = current.auto_height(ctx)?;
            // Raw space, not `text_default_pos`'s final/fit-scaled space —
            // must match `w`/`h` (also raw, via `auto_width`/`auto_height`)
            // since this whole transform is applied to raw glyph coordinates
            // before the block's shared fit-scale is applied (see the
            // renderers' `render_text_lines`).
            let (nx, ny) = text_raw_pos(current, ctx)?;
            let rotation = node_box.rotation.eval_or(ctx, 0.0)?;
            let scale_x = node_box.scale_x.eval_or(ctx, 1.0)?;
            let scale_y = node_box.scale_y.eval_or(ctx, 1.0)?;
            let pivot_x = node_box.pivot_x.eval_or(ctx, w * 0.5)?;
            let pivot_y = node_box.pivot_y.eval_or(ctx, h * 0.5)?;
            let local = AffineTransform::from_translate(-nx, -ny).concat(positional_transform(
                RcPosition::new(nx as f64, ny as f64),
                RcSize {
                    width: scale_x,
                    height: scale_y,
                },
                rotation,
                pivot_x as f32,
                pivot_y as f32,
                AffineTransform::identity(),
            ));
            return Ok(Some(local));
        }
        match current.parent {
            Some(parent_id) => current = ctx.node(parent_id)?,
            None => return Ok(None),
        }
    }
}

/// Resolve a `Text` node's own `wrap`/`text_align` to the concrete values
/// `renderer_core::layout_text` (and friends) expect: `wrap` in `f32` pixels
/// (absent = wrapping off), `text_align` defaulting to `Left`.
fn resolve_text_wrap_align(
    wrap: &crate::nodes::AttrExpr<f64>,
    text_align: Option<renderer_core::TextAlign>,
    ctx: &EvalCtx,
) -> anyhow::Result<(Option<f32>, renderer_core::TextAlign)> {
    let wrap_px = wrap
        .get_expr()
        .map(|e| e.eval(ctx))
        .transpose()?
        .map(|w| w as f32);
    Ok((wrap_px, text_align.unwrap_or_default()))
}

/// Per-column widths (max width of any counted child in that column) and
/// per-row heights (max height), scanning `children` once in row-major
/// order (row = i/cols, col = i%cols, i counting only reserve-or-active
/// children — same predicate `Row`/`Column` already use). `outer` selects
/// `get_outer_width`/`get_outer_height` (position queries, matching
/// `auto_x`/`auto_y`'s convention) vs. plain `get_width`/`get_height` (the
/// group's own auto-size, matching `auto_width`/`auto_height`'s
/// convention) — these two already intentionally differ for `Row`/`Column`
/// today, and `Grid` preserves the same split rather than a third one.
///
/// `col_widths` always has exactly `cols` entries; `row_heights` grows to
/// however many rows the counted children actually fill. If `find` matches
/// a counted child, also returns its 0-based row-major index.
fn grid_dims(
    ctx: &EvalCtx,
    children: &[NodeId],
    cols: usize,
    reserve: bool,
    find: Option<NodeId>,
    outer: bool,
) -> anyhow::Result<(Vec<f64>, Vec<f64>, Option<usize>)> {
    let mut col_widths = vec![0.0f64; cols];
    let mut row_heights: Vec<f64> = Vec::new();
    let mut i = 0usize;
    let mut found = None;
    for &child_id in children {
        let node = ctx.node(child_id)?;
        if !(reserve || node.is_active(ctx.frame())) {
            continue;
        }
        if Some(child_id) == find {
            found = Some(i);
        }
        let (row, col) = (i / cols, i % cols);
        if row >= row_heights.len() {
            row_heights.resize(row + 1, 0.0);
        }
        let (w, h) = if outer {
            (node.get_outer_width(ctx)?, node.get_outer_height(ctx)?)
        } else {
            (node.get_width(ctx)?, node.get_height(ctx)?)
        };
        col_widths[col] = col_widths[col].max(w);
        row_heights[row] = row_heights[row].max(h);
        i += 1;
    }
    Ok((col_widths, row_heights, found))
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
