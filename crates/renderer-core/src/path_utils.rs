use crate::PathCommand;
use crate::Position;
use crate::glyph_cache::PathVerb;

/// Convert `PathCommand` list to backend-independent path verbs.
pub fn build_path_verbs(commands: &[PathCommand]) -> Vec<PathVerb> {
    let mut verbs = Vec::new();
    let mut cur = (0.0f32, 0.0f32);
    for cmd in commands {
        match cmd {
            PathCommand::Move { position, .. } => {
                cur = (position.x as f32, position.y as f32);
                verbs.push(PathVerb::MoveTo(cur.0, cur.1));
            }
            PathCommand::Line { position, .. } => {
                cur = (position.x as f32, position.y as f32);
                verbs.push(PathVerb::LineTo(cur.0, cur.1));
            }
            PathCommand::Cubic {
                position,
                c1_x,
                c1_y,
                c2_x,
                c2_y,
                ..
            } => {
                let end = (position.x as f32, position.y as f32);
                let c1 = (cur.0 + *c1_x as f32, cur.1 + *c1_y as f32);
                let c2 = (end.0 + *c2_x as f32, end.1 + *c2_y as f32);
                verbs.push(PathVerb::CubicTo(c1.0, c1.1, c2.0, c2.1, end.0, end.1));
                cur = end;
            }
            PathCommand::Close { .. } => {
                verbs.push(PathVerb::Close);
            }
        }
    }
    verbs
}

/// Convert `PathCommand` list to backend-independent path verbs, cropping to
/// `[crop_start, crop_end]` (both in `[0.0, 1.0]` as fractions of total arc length).
pub fn build_cropped_path_verbs(
    commands: &[PathCommand],
    crop_start: f64,
    crop_end: f64,
) -> Vec<PathVerb> {
    if crop_start <= 0.0 && crop_end >= 1.0 {
        return build_path_verbs(commands);
    }

    #[derive(Clone)]
    enum SegKind {
        Line {
            end: Position,
        },
        Cubic {
            c1: Position,
            c2: Position,
            end: Position,
        },
    }

    struct Seg {
        start: Position,
        kind: SegKind,
        len: f64,
    }

    let mut segs: Vec<Seg> = Vec::new();
    let mut cur = Position::new(0.0, 0.0);
    let mut subpath_start = Position::new(0.0, 0.0);
    for cmd in commands {
        match cmd {
            PathCommand::Move { position, .. } => {
                cur = Position::new(position.x, position.y);
                subpath_start = cur;
            }
            PathCommand::Line { position, .. } => {
                let end = Position::new(position.x, position.y);
                let len = (end.x - cur.x).hypot(end.y - cur.y);
                segs.push(Seg {
                    start: cur,
                    kind: SegKind::Line { end },
                    len,
                });
                cur = end;
            }
            PathCommand::Cubic {
                position,
                c1_x,
                c1_y,
                c2_x,
                c2_y,
                ..
            } => {
                let end = Position::new(position.x, position.y);
                let bezier = CubicBezier {
                    p0: cur,
                    c1: Position::new(cur.x + c1_x, cur.y + c1_y),
                    c2: Position::new(end.x + c2_x, end.y + c2_y),
                    p3: end,
                };
                let len = bezier.arc_length();
                segs.push(Seg {
                    start: cur,
                    kind: SegKind::Cubic {
                        c1: bezier.c1,
                        c2: bezier.c2,
                        end,
                    },
                    len,
                });
                cur = end;
            }
            PathCommand::Close { .. } => {
                if cur != subpath_start {
                    let len = (subpath_start.x - cur.x).hypot(subpath_start.y - cur.y);
                    segs.push(Seg {
                        start: cur,
                        kind: SegKind::Line { end: subpath_start },
                        len,
                    });
                    cur = subpath_start;
                }
            }
        }
    }

    let total_len: f64 = segs.iter().map(|s| s.len).sum();
    if total_len == 0.0 {
        return build_path_verbs(commands);
    }

    let start_dist = (crop_start * total_len).max(0.0);
    let end_dist = (crop_end * total_len).min(total_len);
    if start_dist >= end_dist {
        return Vec::new();
    }

    let mut verbs = Vec::new();
    let mut accumulated = 0.0f64;
    let mut last_end: Option<Position> = None;

    for seg in &segs {
        let seg_end_acc = accumulated + seg.len;

        if seg_end_acc <= start_dist {
            accumulated = seg_end_acc;
            continue;
        }
        if accumulated >= end_dist {
            break;
        }

        let t1 = if accumulated < start_dist && seg.len > 0.0 {
            ((start_dist - accumulated) / seg.len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let t2 = if seg_end_acc > end_dist && seg.len > 0.0 {
            ((end_dist - accumulated) / seg.len).clamp(0.0, 1.0)
        } else {
            1.0
        };

        match &seg.kind {
            SegKind::Line { end } => {
                let start_pt = Position::new(
                    seg.start.x + t1 * (end.x - seg.start.x),
                    seg.start.y + t1 * (end.y - seg.start.y),
                );
                let end_pt = Position::new(
                    seg.start.x + t2 * (end.x - seg.start.x),
                    seg.start.y + t2 * (end.y - seg.start.y),
                );
                if last_end != Some(start_pt) {
                    verbs.push(PathVerb::MoveTo(start_pt.x as f32, start_pt.y as f32));
                }
                verbs.push(PathVerb::LineTo(end_pt.x as f32, end_pt.y as f32));
                last_end = Some(end_pt);
            }
            SegKind::Cubic { c1, c2, end } => {
                let sub = CubicBezier {
                    p0: seg.start,
                    c1: *c1,
                    c2: *c2,
                    p3: *end,
                }
                .subsegment(t1, t2);
                if last_end != Some(sub.p0) {
                    verbs.push(PathVerb::MoveTo(sub.p0.x as f32, sub.p0.y as f32));
                }
                verbs.push(PathVerb::CubicTo(
                    sub.c1.x as f32,
                    sub.c1.y as f32,
                    sub.c2.x as f32,
                    sub.c2.y as f32,
                    sub.p3.x as f32,
                    sub.p3.y as f32,
                ));
                last_end = Some(sub.p3);
            }
        }

        accumulated = seg_end_acc;
    }

    verbs
}

// ── Private helpers ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct CubicBezier {
    p0: Position,
    c1: Position,
    c2: Position,
    p3: Position,
}

impl CubicBezier {
    /// Arc length via fixed-step numerical integration.
    fn arc_length(self) -> f64 {
        const STEPS: usize = 16;
        let mut len = 0.0f64;
        let mut prev = self.p0;
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let inv = 1.0 - t;
            let inv2 = inv * inv;
            let inv3 = inv2 * inv;
            let t2 = t * t;
            let t3 = t2 * t;
            let cur = Position::new(
                inv3 * self.p0.x
                    + 3.0 * inv2 * t * self.c1.x
                    + 3.0 * inv * t2 * self.c2.x
                    + t3 * self.p3.x,
                inv3 * self.p0.y
                    + 3.0 * inv2 * t * self.c1.y
                    + 3.0 * inv * t2 * self.c2.y
                    + t3 * self.p3.y,
            );
            let dx = cur.x - prev.x;
            let dy = cur.y - prev.y;
            len += (dx * dx + dy * dy).sqrt();
            prev = cur;
        }
        len
    }

    /// Split at parameter `t`, returning the left and right halves.
    fn split(self, t: f64) -> (CubicBezier, CubicBezier) {
        let lerp =
            |a: Position, b: Position| Position::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y));
        let m01 = lerp(self.p0, self.c1);
        let m12 = lerp(self.c1, self.c2);
        let m23 = lerp(self.c2, self.p3);
        let m012 = lerp(m01, m12);
        let m123 = lerp(m12, m23);
        let m0123 = lerp(m012, m123);
        (
            CubicBezier {
                p0: self.p0,
                c1: m01,
                c2: m012,
                p3: m0123,
            },
            CubicBezier {
                p0: m0123,
                c1: m123,
                c2: m23,
                p3: self.p3,
            },
        )
    }

    /// Extract the sub-curve between parameters `t1` and `t2`.
    fn subsegment(self, t1: f64, t2: f64) -> CubicBezier {
        let (_, right) = self.split(t1);
        let t_new = if t1 < 1.0 {
            (t2 - t1) / (1.0 - t1)
        } else {
            1.0
        };
        let (left, _) = right.split(t_new.clamp(0.0, 1.0));
        left
    }
}
