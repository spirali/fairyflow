use crate::basictypes::NodeId;
use crate::eval::EvalCtx;
use crate::nodes::NodeKind;
use crate::values::Eval;
use anyhow::bail;
use renderer_core::Position;

/// Evaluate a cubic Bézier at parameter `u` ∈ [0, 1].
/// p0/c1/c2/p1 are all absolute coordinates.
#[inline]
fn cubic_bezier_point(p0: Position, c1: Position, c2: Position, p1: Position, u: f64) -> Position {
    let inv = 1.0 - u;
    let inv2 = inv * inv;
    let inv3 = inv2 * inv;
    let u2 = u * u;
    let u3 = u2 * u;
    Position::new(
        inv3 * p0.x + 3.0 * inv2 * u * c1.x + 3.0 * inv * u2 * c2.x + u3 * p1.x,
        inv3 * p0.y + 3.0 * inv2 * u * c1.y + 3.0 * inv * u2 * c2.y + u3 * p1.y,
    )
}

/// Arc length of a cubic Bézier via fixed-step numerical integration.
fn cubic_arc_length(p0: Position, c1: Position, c2: Position, p1: Position) -> f64 {
    const STEPS: usize = 64;
    let mut len = 0.0;
    let mut prev = p0;
    for i in 1..=STEPS {
        let u = i as f64 / STEPS as f64;
        let cur = cubic_bezier_point(p0, c1, c2, p1, u);
        let dx = cur.x - prev.x;
        let dy = cur.y - prev.y;
        len += (dx * dx + dy * dy).sqrt();
        prev = cur;
    }
    len
}

enum Seg {
    Line(Position, Position),
    Cubic(Position, Position, Position, Position),
}

impl Seg {
    fn arc_length(&self) -> f64 {
        match self {
            Seg::Line(p0, p1) => {
                let dx = p1.x - p0.x;
                let dy = p1.y - p0.y;
                (dx * dx + dy * dy).sqrt()
            }
            Seg::Cubic(p0, c1, c2, p1) => cubic_arc_length(*p0, *c1, *c2, *p1),
        }
    }
}

struct PathSegments {
    segments: Vec<Seg>,
    first: Position,
    last: Position,
}

fn build_segments(ctx: &EvalCtx, node: NodeId) -> anyhow::Result<PathSegments> {
    let node = ctx.node(node)?;
    let NodeKind::Path { children, .. } = &node.kind else {
        anyhow::bail!("expected path node, got {:?}", node.kind);
    };

    let mut segments: Vec<Seg> = Vec::with_capacity(children.len());
    let mut cur = Position::new(0.0, 0.0);
    let mut subpath_start: Option<Position> = None;
    let mut first_point: Option<Position> = None;

    for &child_id in children {
        let child = ctx.node(child_id)?;
        match &child.kind {
            NodeKind::Move { position } => {
                cur = Position::new(position.x.eval(ctx)?, position.y.eval(ctx)?);
                subpath_start = Some(cur);
                if first_point.is_none() {
                    first_point = Some(cur);
                }
            }
            NodeKind::Line { position } => {
                let end = Position::new(position.x.eval(ctx)?, position.y.eval(ctx)?);
                if first_point.is_none() {
                    first_point = Some(cur);
                }
                segments.push(Seg::Line(cur, end));
                cur = end;
            }
            NodeKind::Cubic {
                position,
                c1_x,
                c1_y,
                c2_x,
                c2_y,
            } => {
                let end = Position::new(position.x.eval(ctx)?, position.y.eval(ctx)?);
                // c1 is relative to the start point, c2 is relative to the end point
                let c1 = Position::new(cur.x + c1_x.eval(ctx)?, cur.y + c1_y.eval(ctx)?);
                let c2 = Position::new(end.x + c2_x.eval(ctx)?, end.y + c2_y.eval(ctx)?);
                if first_point.is_none() {
                    first_point = Some(cur);
                }
                segments.push(Seg::Cubic(cur, c1, c2, end));
                cur = end;
            }
            NodeKind::Close => {
                let start = subpath_start.unwrap_or(Position::new(0.0, 0.0));
                if cur.x != start.x || cur.y != start.y {
                    segments.push(Seg::Line(cur, start));
                }
                cur = start;
            }
            _ => bail!("unexpected node kind in path children: {:?}", child.id),
        }
    }

    Ok(PathSegments {
        first: first_point.unwrap_or(Position::new(0.0, 0.0)),
        last: cur,
        segments,
    })
}

pub(crate) fn path_length(ctx: &EvalCtx, node: NodeId) -> anyhow::Result<f64> {
    let path = build_segments(ctx, node)?;
    Ok(path.segments.iter().map(|s| s.arc_length()).sum())
}

pub(crate) fn point_in_path(ctx: &EvalCtx, node: NodeId, t: f64) -> anyhow::Result<Position> {
    let path = build_segments(ctx, node)?;
    let PathSegments {
        segments,
        first,
        last,
    } = path;

    if segments.is_empty() {
        return Ok(first);
    }

    if t <= 0.0 {
        return Ok(first);
    }
    if t >= 1.0 {
        return Ok(last);
    }

    // Compute arc length of each segment.
    let seg_lengths: Vec<f64> = segments.iter().map(|s| s.arc_length()).collect();

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
                Seg::Line(p0, p1) => Position::new(
                    p0.x + local_t * (p1.x - p0.x),
                    p0.y + local_t * (p1.y - p0.y),
                ),
                Seg::Cubic(p0, c1, c2, p1) => cubic_bezier_point(*p0, *c1, *c2, *p1, local_t),
            });
        }
        accumulated += seg_len;
    }

    Ok(last)
}
