use crate::{Node, NodeKind, Position};

/// Portable 2D affine transform.
///
/// A point `(x, y)` maps to `(x*a + y*c + e, x*b + y*d + f)`.
/// This is the standard 2D affine matrix stored in column-major order, matching
/// the convention used by both `tiny_skia::Transform` and `krilla::geom::Transform`.
#[derive(Clone, Copy, Debug)]
pub struct AffineTransform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl AffineTransform {
    pub fn identity() -> Self {
        Self { a: 1., b: 0., c: 0., d: 1., e: 0., f: 0. }
    }

    pub fn from_translate(tx: f32, ty: f32) -> Self {
        Self { a: 1., b: 0., c: 0., d: 1., e: tx, f: ty }
    }

    pub fn from_scale(sx: f32, sy: f32) -> Self {
        Self { a: sx, b: 0., c: 0., d: sy, e: 0., f: 0. }
    }

    pub fn from_rotate_degrees(deg: f32) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        Self { a: c, b: s, c: -s, d: c, e: 0., f: 0. }
    }

    /// Compose transforms: apply `self` first, then `other`.
    /// Equivalent to tiny-skia's `post_concat`.
    pub fn concat(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }
}

/// Build the local-to-parent transform for a positioned node, then compose with `parent`.
///
/// Equivalent to tiny-skia chain:
/// `translate(-pivot) · scale · rotate · translate(pos + pivot) · parent`
pub fn positional_transform(
    position: &Position,
    scale_x: f64,
    scale_y: f64,
    rotation: f64,
    pivot_x: f32,
    pivot_y: f32,
    parent: AffineTransform,
) -> AffineTransform {
    AffineTransform::from_translate(-pivot_x, -pivot_y)
        .concat(AffineTransform::from_scale(scale_x as f32, scale_y as f32))
        .concat(AffineTransform::from_rotate_degrees(rotation as f32))
        .concat(AffineTransform::from_translate(
            position.x as f32 + pivot_x,
            position.y as f32 + pivot_y,
        ))
        .concat(parent)
}

/// Extract the z-level value from any node kind.
pub fn node_z_level(node: &Node) -> f64 {
    match &node.kind {
        NodeKind::Group { z_level, .. }
        | NodeKind::Rect { z_level, .. }
        | NodeKind::Ellipse { z_level, .. }
        | NodeKind::Path { z_level, .. }
        | NodeKind::Text { z_level, .. }
        | NodeKind::Image { z_level, .. } => *z_level.value(),
    }
}
