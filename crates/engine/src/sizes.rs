use crate::basictypes::NodeId;
use crate::defs::{Node, NodeKind, Position, Size};
use crate::eval::EvalCtx;

struct BBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl BBox {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        BBox {
            x,
            y,
            width,
            height,
        }
    }

    pub fn new_zero_size(x: f64, y: f64) -> Self {
        BBox {
            x,
            y,
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn empty() -> Self {
        BBox {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

impl Node {
    /// Returns the axis-aligned bounding box of this node in its parent's coordinate space.
    /// For Group nodes, rotation and scaling are applied to the four corners.
    fn bbox_of_group(&self, ctx: &EvalCtx) -> anyhow::Result<BBox> {
        match &self.kind {
            NodeKind::Group {
                position,
                size,
                scale_x,
                scale_y,
                rotation,
                ..
            } => {
                let tx = position.x.eval_f64(ctx)?;
                let ty = position.y.eval_f64(ctx)?;
                let w = size.width.eval_f64(ctx)?;
                let h = size.height.eval_f64(ctx)?;
                let sx = scale_x.eval_f64(ctx)?;
                let sy = scale_y.eval_f64(ctx)?;
                let r = rotation.eval_f64(ctx)?.to_radians();
                let cos_r = r.cos();
                let sin_r = r.sin();

                // Transform the four corners of the group's local bounding box to parent space.
                // parent = (cos_r*sx*lx - sin_r*sy*ly + tx, sin_r*sx*lx + cos_r*sy*ly + ty)
                let corners = [(0.0f64, 0.0f64), (w, 0.0), (w, h), (0.0, h)];
                let mut min_x = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for (lx, ly) in corners {
                    let px = cos_r * sx * lx - sin_r * sy * ly + tx;
                    let py = sin_r * sx * lx + cos_r * sy * ly + ty;
                    min_x = min_x.min(px);
                    max_x = max_x.max(px);
                    min_y = min_y.min(py);
                    max_y = max_y.max(py);
                }
                Ok(BBox::new(min_x, min_y, max_x - min_x, max_y - min_y))
            }
            _ => {
                let x = self.get_x(ctx)?;
                let y = self.get_y(ctx)?;
                let w = self.get_width(ctx)?;
                let h = self.get_height(ctx)?;
                Ok(BBox::new(x, y, w, h))
            }
        }
    }

    pub fn get_position(&self) -> Option<&Position> {
        match &self.kind {
            NodeKind::Group { position, .. }
            | NodeKind::Rect { position, .. }
            | NodeKind::Ellipse { position, .. }
            | NodeKind::Move { position, .. }
            | NodeKind::Line { position, .. }
            | NodeKind::Text { position, .. }
            | NodeKind::Cubic { position, .. } => Some(&position),
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } | NodeKind::Path { .. } => None,
        }
    }

    pub fn get_size(&self) -> Option<&Size> {
        match &self.kind {
            NodeKind::Group { size, .. }
            | NodeKind::Rect { size, .. }
            | NodeKind::Ellipse { size, .. } => Some(&size),
            NodeKind::Text { .. }
            | NodeKind::Move { .. }
            | NodeKind::Line { .. }
            | NodeKind::Cubic { .. }
            | NodeKind::TextGroup { .. }
            | NodeKind::TextSpan { .. }
            | NodeKind::Path { .. } => None,
        }
    }

    pub fn get_x(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(p) = self.get_position() else {
            return Ok(0.0);
        };
        p.x.eval_f64(ctx)
    }

    pub fn get_y(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(p) = self.get_position() else {
            return Ok(0.0);
        };
        p.y.eval_f64(ctx)
    }

    pub fn get_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(s) = self.get_size() else {
            return Ok(0.0);
        };
        s.width.eval_f64(ctx)
    }

    pub fn get_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        let Some(s) = self.get_size() else {
            return Ok(0.0);
        };
        s.height.eval_f64(ctx)
    }

    pub fn default_width(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match &self.kind {
            NodeKind::Group { children, .. } => {
                let mut max_x: f64 = 0.0;
                for child in children {
                    let node = ctx.node(*child)?;
                    let bbox = node.bbox_of_group(ctx)?;
                    max_x = max_x.max(bbox.x + bbox.width);
                }
                max_x
            }
            NodeKind::Text { children, .. } => {
                let lines = children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                renderer::measure_text(&lines).0 as f64
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                let child = self.eval_as_text_child(ctx)?;
                renderer::measure_text(&[child]).0 as f64
            }
            _ => 0.0,
        })
    }

    pub fn default_height(&self, ctx: &EvalCtx) -> anyhow::Result<f64> {
        Ok(match &self.kind {
            NodeKind::Group { children, .. } => {
                let mut max_y: f64 = 0.0;
                for child in children {
                    let node = ctx.node(*child)?;
                    let bbox = node.bbox_of_group(ctx)?;
                    max_y = max_y.max(bbox.y + bbox.height);
                }
                max_y
            }
            NodeKind::Text { children, .. } => {
                let lines = children
                    .iter()
                    .map(|&id| ctx.node(id)?.eval_as_text_child(ctx))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                renderer::measure_text(&lines).1 as f64
            }
            NodeKind::TextGroup { .. } | NodeKind::TextSpan { .. } => {
                let child = self.eval_as_text_child(ctx)?;
                renderer::measure_text(&[child]).1 as f64
            }
            _ => 0.0,
        })
    }
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
