/// Standard CSS `cubic-bezier(x1, y1, x2, y2)` easing curve: control points
/// `P0=(0,0)`, `P1=(x1,y1)`, `P2=(x2,y2)`, `P3=(1,1)`. `remap(t)` treats `t`
/// as the bezier's X axis (normalized time), solves for the bezier parameter
/// `u` such that `Bx(u) = t` (Newton-Raphson with a bisection fallback for
/// robustness — the standard approach browsers use for `cubic-bezier()`),
/// then returns `By(u)` as the eased progress.
///
/// Distinct from (though built on the same cubic formula as) `paths.rs`'s
/// `cubic_bezier_point`, which evaluates a 2D *position* at a given
/// parameter — easing needs the inverse: solve for the parameter given an
/// X-coordinate, then read off Y.
struct CubicBezier {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl CubicBezier {
    /// Evaluate one axis of the bezier (P0=0, P3=1) at parameter `u`.
    fn axis(u: f64, p1: f64, p2: f64) -> f64 {
        let inv = 1.0 - u;
        3.0 * inv * inv * u * p1 + 3.0 * inv * u * u * p2 + u * u * u
    }

    /// Derivative of `axis` with respect to `u`.
    fn axis_derivative(u: f64, p1: f64, p2: f64) -> f64 {
        let inv = 1.0 - u;
        3.0 * inv * inv * p1 + 6.0 * inv * u * (p2 - p1) + 3.0 * u * u * (1.0 - p2)
    }

    fn solve_u_for_x(&self, x: f64) -> f64 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        // Newton-Raphson, starting from `x` itself (a reasonable guess since
        // these presets stay reasonably close to the identity curve).
        let mut u = x;
        for _ in 0..8 {
            let error = Self::axis(u, self.x1, self.x2) - x;
            if error.abs() < 1e-6 {
                return u;
            }
            let derivative = Self::axis_derivative(u, self.x1, self.x2);
            if derivative.abs() < 1e-6 {
                break;
            }
            u -= error / derivative;
        }
        // Bisection fallback if Newton-Raphson didn't converge (e.g. a flat
        // derivative region) — guaranteed to converge since `axis` is
        // monotonic in `u` for the x-coordinates of valid easing presets.
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..30 {
            u = (lo + hi) / 2.0;
            let cur_x = Self::axis(u, self.x1, self.x2);
            if (cur_x - x).abs() < 1e-6 {
                break;
            }
            if cur_x < x {
                lo = u;
            } else {
                hi = u;
            }
        }
        u
    }

    fn remap(&self, t: f64) -> f64 {
        let u = self.solve_u_for_x(t);
        Self::axis(u, self.y1, self.y2)
    }
}

/// Remap a linear progress fraction `t` ∈ [0, 1] through the named easing
/// preset. `t` outside the loop's actual usage is
/// always ∈ [0, 1] since it's a fraction between two adjacent keyframes.
pub(crate) fn remap(name: &str, t: f64) -> f64 {
    // Control points are the standard CSS equivalents for in/out/in_out;
    // out_back has no native CSS keyword, using a common overshoot
    // approximation (P1.y > 1 is what produces the overshoot-then-settle).
    let curve = match name {
        "linear" => return t,
        "in" => CubicBezier {
            x1: 0.42,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        },
        "out" => CubicBezier {
            x1: 0.0,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        },
        "in_out" => CubicBezier {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        },
        "out_back" => CubicBezier {
            x1: 0.34,
            y1: 1.56,
            x2: 0.64,
            y2: 1.0,
        },
        _ => unreachable!("unsupported ease name {name:?} should have been rejected at parse time"),
    };
    curve.remap(t)
}
