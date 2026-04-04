use anyhow::bail;
use crate::basictypes::NodeId;
use crate::eval::EvalCtx;
use crate::FrameId;
use crate::nodes::NodeKind;

/// Evaluate a cubic Bézier at parameter `u` ∈ [0, 1].
/// p0/c1/c2/p1 are all absolute coordinates.
#[inline]
fn cubic_bezier_point(
    p0x: f64, p0y: f64,
    c1x: f64, c1y: f64,
    c2x: f64, c2y: f64,
    p1x: f64, p1y: f64,
    u: f64,
) -> (f64, f64) {
    let inv = 1.0 - u;
    let inv2 = inv * inv;
    let inv3 = inv2 * inv;
    let u2 = u * u;
    let u3 = u2 * u;
    (
        inv3 * p0x + 3.0 * inv2 * u * c1x + 3.0 * inv * u2 * c2x + u3 * p1x,
        inv3 * p0y + 3.0 * inv2 * u * c1y + 3.0 * inv * u2 * c2y + u3 * p1y,
    )
}

/// Arc length of a cubic Bézier via fixed-step numerical integration.
fn cubic_arc_length(
    p0x: f64, p0y: f64,
    c1x: f64, c1y: f64,
    c2x: f64, c2y: f64,
    p1x: f64, p1y: f64,
) -> f64 {
    const STEPS: usize = 64;
    let mut len = 0.0;
    let mut prev = (p0x, p0y);
    for i in 1..=STEPS {
        let u = i as f64 / STEPS as f64;
        let cur = cubic_bezier_point(p0x, p0y, c1x, c1y, c2x, c2y, p1x, p1y, u);
        let dx = cur.0 - prev.0;
        let dy = cur.1 - prev.1;
        len += (dx * dx + dy * dy).sqrt();
        prev = cur;
    }
    len
}

pub(crate) fn follow_path(ctx: &EvalCtx, node: NodeId, start_frame: FrameId, end_frame: FrameId) -> anyhow::Result<(f64, f64)> {
    let node = ctx.node(node)?;
    let NodeKind::Path { children, .. } = &node.kind else {
        anyhow::bail!("expected path node, got {:?}", node.kind);
    };

    // Enumerate path segments as (start_point, end_point, optional cubic control points).
    enum Seg {
        Line(f64, f64, f64, f64),
        Cubic(f64, f64, f64, f64, f64, f64, f64, f64),
    }

    let mut segments: Vec<Seg> = Vec::new();
    let mut cur_x = 0.0f64;
    let mut cur_y = 0.0f64;
    let mut first_point: Option<(f64, f64)> = None;

    for &child_id in children {
        let child = ctx.node(child_id)?;
        match &child.kind {
            NodeKind::Move { position } => {
                cur_x = position.x.eval_f64(ctx)?;
                cur_y = position.y.eval_f64(ctx)?;
                if first_point.is_none() {
                    first_point = Some((cur_x, cur_y));
                }
            }
            NodeKind::Line { position } => {
                let x = position.x.eval_f64(ctx)?;
                let y = position.y.eval_f64(ctx)?;
                if first_point.is_none() {
                    first_point = Some((cur_x, cur_y));
                }
                segments.push(Seg::Line(cur_x, cur_y, x, y));
                cur_x = x;
                cur_y = y;
            }
            NodeKind::Cubic { position, c1_x, c1_y, c2_x, c2_y } => {
                let x = position.x.eval_f64(ctx)?;
                let y = position.y.eval_f64(ctx)?;
                // c1 is relative to the start point, c2 is relative to the end point
                let c1x = cur_x + c1_x.eval_f64(ctx)?;
                let c1y = cur_y + c1_y.eval_f64(ctx)?;
                let c2x = x + c2_x.eval_f64(ctx)?;
                let c2y = y + c2_y.eval_f64(ctx)?;
                if first_point.is_none() {
                    first_point = Some((cur_x, cur_y));
                }
                segments.push(Seg::Cubic(cur_x, cur_y, c1x, c1y, c2x, c2y, x, y));
                cur_x = x;
                cur_y = y;
            }
            _ => bail!("unexpected node kind in path children: {:?}", child.id),
        }
    }

    let first = first_point.unwrap_or((0.0, 0.0));
    let last = (cur_x, cur_y);

    if segments.is_empty() {
        return Ok(first);
    }

    // Compute t ∈ [0, 1] from current frame.
    let t = if end_frame <= start_frame {
        0.0f64
    } else {
        let cf = ctx.frame().as_u32() as f64;
        let sf = start_frame.as_u32() as f64;
        let ef = end_frame.as_u32() as f64;
        ((cf - sf) / (ef - sf)).clamp(0.0, 1.0)
    };

    if t <= 0.0 {
        return Ok(first);
    }
    if t >= 1.0 {
        return Ok(last);
    }

    // Compute arc length of each segment.
    let seg_lengths: Vec<f64> = segments
        .iter()
        .map(|seg| match seg {
            Seg::Line(x0, y0, x1, y1) => {
                let dx = x1 - x0;
                let dy = y1 - y0;
                (dx * dx + dy * dy).sqrt()
            }
            Seg::Cubic(p0x, p0y, c1x, c1y, c2x, c2y, p1x, p1y) => {
                cubic_arc_length(*p0x, *p0y, *c1x, *c1y, *c2x, *c2y, *p1x, *p1y)
            }
        })
        .collect();

    let total_len: f64 = seg_lengths.iter().sum();
    if total_len == 0.0 {
        return Ok(first);
    }

    let target = t * total_len;
    let mut accumulated = 0.0;

    for (seg, &seg_len) in segments.iter().zip(seg_lengths.iter()) {
        if accumulated + seg_len >= target || std::ptr::eq(seg, segments.last().unwrap()) {
            // Interpolate within this segment.
            let local_t = if seg_len > 0.0 {
                ((target - accumulated) / seg_len).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return Ok(match seg {
                Seg::Line(x0, y0, x1, y1) => (
                    x0 + local_t * (x1 - x0),
                    y0 + local_t * (y1 - y0),
                ),
                Seg::Cubic(p0x, p0y, c1x, c1y, c2x, c2y, p1x, p1y) => {
                    cubic_bezier_point(*p0x, *p0y, *c1x, *c1y, *c2x, *c2y, *p1x, *p1y, local_t)
                }
            });
        }
        accumulated += seg_len;
    }

    Ok(last)
}