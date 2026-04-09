use crate::glyph_cache::PathVerb;
use crate::PathCommand;

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
            PathCommand::Cubic { position, c1_x, c1_y, c2_x, c2_y, .. } => {
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
        Line { ex: f32, ey: f32 },
        Cubic { c1x: f32, c1y: f32, c2x: f32, c2y: f32, ex: f32, ey: f32 },
    }
    struct Seg {
        sx: f32,
        sy: f32,
        kind: SegKind,
        len: f32,
    }

    let mut segs: Vec<Seg> = Vec::new();
    let mut cur = (0.0f32, 0.0f32);
    let mut subpath_start = (0.0f32, 0.0f32);
    for cmd in commands {
        match cmd {
            PathCommand::Move { position, .. } => {
                cur = (position.x as f32, position.y as f32);
                subpath_start = cur;
            }
            PathCommand::Line { position, .. } => {
                let end = (position.x as f32, position.y as f32);
                let dx = end.0 - cur.0;
                let dy = end.1 - cur.1;
                let len = (dx * dx + dy * dy).sqrt();
                segs.push(Seg {
                    sx: cur.0,
                    sy: cur.1,
                    kind: SegKind::Line { ex: end.0, ey: end.1 },
                    len,
                });
                cur = end;
            }
            PathCommand::Cubic { position, c1_x, c1_y, c2_x, c2_y, .. } => {
                let end = (position.x as f32, position.y as f32);
                let c1 = (cur.0 + *c1_x as f32, cur.1 + *c1_y as f32);
                let c2 = (end.0 + *c2_x as f32, end.1 + *c2_y as f32);
                let len = cubic_arc_length_f32(cur.0, cur.1, c1.0, c1.1, c2.0, c2.1, end.0, end.1);
                segs.push(Seg {
                    sx: cur.0,
                    sy: cur.1,
                    kind: SegKind::Cubic {
                        c1x: c1.0,
                        c1y: c1.1,
                        c2x: c2.0,
                        c2y: c2.1,
                        ex: end.0,
                        ey: end.1,
                    },
                    len,
                });
                cur = end;
            }
            PathCommand::Close { .. } => {
                let (sx, sy) = subpath_start;
                if cur.0 != sx || cur.1 != sy {
                    let dx = sx - cur.0;
                    let dy = sy - cur.1;
                    let len = (dx * dx + dy * dy).sqrt();
                    segs.push(Seg {
                        sx: cur.0,
                        sy: cur.1,
                        kind: SegKind::Line { ex: sx, ey: sy },
                        len,
                    });
                    cur = subpath_start;
                }
            }
        }
    }

    let total_len: f32 = segs.iter().map(|s| s.len).sum();
    if total_len == 0.0 {
        return build_path_verbs(commands);
    }

    let start_dist = (crop_start as f32 * total_len).max(0.0);
    let end_dist = (crop_end as f32 * total_len).min(total_len);
    if start_dist >= end_dist {
        return Vec::new();
    }

    let mut verbs = Vec::new();
    let mut accumulated = 0.0f32;
    let mut last_end: Option<(f32, f32)> = None;

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
            SegKind::Line { ex, ey } => {
                let start_pt = (seg.sx + t1 * (ex - seg.sx), seg.sy + t1 * (ey - seg.sy));
                let end_pt = (seg.sx + t2 * (ex - seg.sx), seg.sy + t2 * (ey - seg.sy));
                if last_end != Some(start_pt) {
                    verbs.push(PathVerb::MoveTo(start_pt.0, start_pt.1));
                }
                verbs.push(PathVerb::LineTo(end_pt.0, end_pt.1));
                last_end = Some(end_pt);
            }
            SegKind::Cubic { c1x, c1y, c2x, c2y, ex, ey } => {
                let p0 = (seg.sx, seg.sy);
                let c1 = (*c1x, *c1y);
                let c2 = (*c2x, *c2y);
                let p3 = (*ex, *ey);
                let (sp0, sc1, sc2, sp3) = cubic_subsegment(p0, c1, c2, p3, t1, t2);
                if last_end != Some(sp0) {
                    verbs.push(PathVerb::MoveTo(sp0.0, sp0.1));
                }
                verbs.push(PathVerb::CubicTo(sc1.0, sc1.1, sc2.0, sc2.1, sp3.0, sp3.1));
                last_end = Some(sp3);
            }
        }

        accumulated = seg_end_acc;
    }

    verbs
}

// ── Private helpers (ported from renderer-skia) ───────────────────────────────

fn cubic_arc_length_f32(
    p0x: f32,
    p0y: f32,
    c1x: f32,
    c1y: f32,
    c2x: f32,
    c2y: f32,
    p1x: f32,
    p1y: f32,
) -> f32 {
    const STEPS: usize = 16;
    let mut len = 0.0f32;
    let mut prev = (p0x, p0y);
    for i in 1..=STEPS {
        let t = i as f32 / STEPS as f32;
        let inv = 1.0 - t;
        let inv2 = inv * inv;
        let inv3 = inv2 * inv;
        let t2 = t * t;
        let t3 = t2 * t;
        let x = inv3 * p0x + 3.0 * inv2 * t * c1x + 3.0 * inv * t2 * c2x + t3 * p1x;
        let y = inv3 * p0y + 3.0 * inv2 * t * c1y + 3.0 * inv * t2 * c2y + t3 * p1y;
        let dx = x - prev.0;
        let dy = y - prev.1;
        len += (dx * dx + dy * dy).sqrt();
        prev = (x, y);
    }
    len
}

fn split_cubic(
    p0: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
    p3: (f32, f32),
    t: f32,
) -> (
    ((f32, f32), (f32, f32), (f32, f32), (f32, f32)),
    ((f32, f32), (f32, f32), (f32, f32), (f32, f32)),
) {
    let lerp = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| -> (f32, f32) {
        (ax + t * (bx - ax), ay + t * (by - ay))
    };
    let m01 = lerp(p0, c1);
    let m12 = lerp(c1, c2);
    let m23 = lerp(c2, p3);
    let m012 = lerp(m01, m12);
    let m123 = lerp(m12, m23);
    let m0123 = lerp(m012, m123);
    ((p0, m01, m012, m0123), (m0123, m123, m23, p3))
}

fn cubic_subsegment(
    p0: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
    p3: (f32, f32),
    t1: f32,
    t2: f32,
) -> ((f32, f32), (f32, f32), (f32, f32), (f32, f32)) {
    let (_, right) = split_cubic(p0, c1, c2, p3, t1);
    let t_new = if t1 < 1.0 { (t2 - t1) / (1.0 - t1) } else { 1.0 };
    let (left, _) = split_cubic(right.0, right.1, right.2, right.3, t_new.clamp(0.0, 1.0));
    left
}
