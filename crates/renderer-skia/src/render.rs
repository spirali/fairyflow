use renderer_core::glyph_cache::{PathVerb, VectorPath};
use renderer_core::highlight;
use renderer_core::image_cache::{self, CachedImageKind, RawPixmap};
use renderer_core::resources::Resources;
use renderer_core::text_layout::{build_span_text, collect_spans, get_or_build_line};
use renderer_core::{
    Color, ImageLayer, Node, NodeKind, PathCommand, Position, Scene, Size, Style, TextChild,
    TextSpan,
};
use resvg::usvg;
use serde::Serialize;
use std::sync::Arc;
use tiny_skia::{
    FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Point, Rect, Stroke, Transform,
};

// ── Color conversion ──────────────────────────────────────────────────────────

fn color_to_skia(c: &Color) -> tiny_skia::Color {
    let (r, g, b, a) = c.to_rgba_f32();
    tiny_skia::Color::from_rgba(r, g, b, a).unwrap_or(tiny_skia::Color::BLACK)
}

// ── RawPixmap → tiny-skia Pixmap ─────────────────────────────────────────────

fn raw_to_pixmap(raw: &RawPixmap) -> Option<Pixmap> {
    let size = tiny_skia::IntSize::from_wh(raw.width, raw.height)?;
    Pixmap::from_vec(raw.data.clone(), size)
}

// ── RasterRenderer ───────────────────────────────────────────────────────────

pub struct RasterRenderer;

impl RasterRenderer {
    pub fn render_scene(&self, scene: &Scene, scale: f32) -> Pixmap {
        let width = (scene.width as f32 * scale).round() as u32;
        let height = (scene.height as f32 * scale).round() as u32;
        let mut pixmap =
            Pixmap::new(width.max(1), height.max(1)).expect("invalid scene dimensions");
        pixmap.fill(color_to_skia(&scene.fill_color));
        self.render_children(
            &scene.children,
            &mut pixmap,
            Transform::from_scale(scale, scale),
            1.0,
        );
        pixmap
    }

    fn render_children(
        &self,
        nodes: &[Node],
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
    ) {
        let mut order: Vec<usize> = (0..nodes.len()).collect();
        order.sort_by(|&a, &b| {
            node_z_level(&nodes[a])
                .partial_cmp(&node_z_level(&nodes[b]))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for i in order {
            self.render_node(&nodes[i], pixmap, parent_transform, parent_alpha);
        }
    }

    fn render_node(
        &self,
        node: &Node,
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
    ) {
        match &node.kind {
            NodeKind::Group {
                position,
                size,
                alpha,
                scale_x,
                scale_y,
                rotation,
                pivot_x,
                pivot_y,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
                z_level: _,
                children,
            } => {
                let pivot_x_abs = (pivot_x * size.width) as f32;
                let pivot_y_abs = (pivot_y * size.height) as f32;
                let transform = positional_transform(
                    position, *scale_x, *scale_y, *rotation,
                    pivot_x_abs, pivot_y_abs, parent_transform,
                );
                let alpha = parent_alpha * *alpha as f32;
                let children = children.clone();

                let needs_clip = *clip_x > 0.0 || *clip_y > 0.0 || *clip_w < 1.0 || *clip_h < 1.0;
                if !needs_clip {
                    self.render_children(&children, pixmap, transform, alpha);
                } else {
                    let w = pixmap.width();
                    let h = pixmap.height();
                    let mut offscreen = Pixmap::new(w, h).expect("offscreen pixmap");
                    self.render_children(&children, &mut offscreen, transform, alpha);

                    let lw = size.width as f32;
                    let lh = size.height as f32;
                    if let Some(clip_rect) = Rect::from_xywh(
                        *clip_x as f32 * lw,
                        *clip_y as f32 * lh,
                        *clip_w as f32 * lw,
                        *clip_h as f32 * lh,
                    ) {
                        let clip_path = PathBuilder::from_rect(clip_rect);
                        if let Some(mut mask) = Mask::new(w, h) {
                            mask.fill_path(&clip_path, FillRule::Winding, true, transform);
                            pixmap.draw_pixmap(
                                0,
                                0,
                                offscreen.as_ref(),
                                &PixmapPaint::default(),
                                Transform::identity(),
                                Some(&mask),
                            );
                        }
                    }
                }
            }
            NodeKind::Rect { position, size, style, z_level: _ } => {
                let transform = positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent_transform);
                let Some(rect) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32)
                else {
                    return;
                };
                let path = PathBuilder::from_rect(rect);
                fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
            }
            NodeKind::Ellipse { position, size, style, z_level: _ } => {
                let transform = positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent_transform);
                let Some(oval) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32)
                else {
                    return;
                };
                let Some(path) = PathBuilder::from_oval(oval) else {
                    return;
                };
                fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
            }
            NodeKind::Path { style, children, z_level: _, crop_start, crop_end } => {
                if let Some(path) = build_cropped_path(children, *crop_start, *crop_end) {
                    fill_and_stroke(&path, style, pixmap, parent_transform, parent_alpha);
                }
            }
            NodeKind::Text {
                position,
                lines,
                sh_language,
                sh_theme,
                ..
            } => {
                let transform = positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent_transform);
                let lines = lines.clone();
                let sh = sh_language.as_ref().map(|lang| {
                    let theme = sh_theme.as_ref().map(|s| s.as_str()).unwrap_or("InspiredGitHub");
                    (lang.as_str(), theme)
                });
                self.render_text_lines(&lines, pixmap, transform, parent_alpha, sh);
            }
            NodeKind::Image {
                position,
                size,
                alpha,
                path,
                keep_aspect,
                z_level: _,
                layers,
                hidden_layers,
                ..
            } => {
                let effective_alpha = parent_alpha * *alpha as f32;
                let layers = layers.clone();
                let hidden_layers = hidden_layers.clone();
                render_image(
                    path,
                    size,
                    *keep_aspect,
                    &layers,
                    &hidden_layers,
                    pixmap,
                    parent_transform,
                    position,
                    effective_alpha,
                );
            }
        }
    }

    fn render_text_lines(
        &self,
        lines: &[TextChild],
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
        sh: Option<(&str, &str)>,
    ) {
        let sh_ctx: Option<(Vec<Vec<usize>>, highlight::SyntaxColors)> = sh.map(|(lang, theme)| {
            let mut full_text = String::new();
            let span_starts: Vec<Vec<usize>> = lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    if i > 0 {
                        full_text.push('\n');
                    }
                    let mut spans: Vec<&TextSpan> = Vec::new();
                    collect_spans(line, &mut spans);
                    spans
                        .iter()
                        .map(|s| {
                            let start = full_text.len();
                            full_text.push_str(s.text.as_str());
                            start
                        })
                        .collect()
                })
                .collect();

            let resources = Resources::get();
            let colors = highlight::highlight_text(
                &full_text,
                lang,
                theme,
                &resources.syntax_set,
                &resources.theme_set,
            );
            (span_starts, colors)
        });

        let mut y_cursor = 0.0f32;
        for (line_idx, line) in lines.iter().enumerate() {
            let mut spans: Vec<&TextSpan> = Vec::new();
            collect_spans(line, &mut spans);
            if spans.is_empty() {
                continue;
            }

            let cached = get_or_build_line(&spans);

            let zwnj_ranges: Vec<(std::ops::Range<usize>, usize)> = if sh_ctx.is_some() {
                let (_, r) = build_span_text(&spans);
                r
            } else {
                Vec::new()
            };

            for glyph in &cached.glyphs {
                let span = spans[glyph.span_idx];
                let alpha = parent_alpha * *span.text_style.alpha.value() as f32;

                let fill_color = if let Some((ref span_starts, ref sh_colors)) = sh_ctx {
                    if span.text_style.fill_color.is_inherited() {
                        let zwnj_start = zwnj_ranges
                            .get(glyph.span_idx)
                            .map(|(r, _)| r.start)
                            .unwrap_or(0);
                        let offset_in_span = (glyph.cluster as usize)
                            .saturating_sub(zwnj_start)
                            .min(span.text.len());
                        let span_start_in_full = span_starts[line_idx]
                            .get(glyph.span_idx)
                            .copied()
                            .unwrap_or(0);
                        let byte_in_full = span_start_in_full + offset_in_span;

                        sh_colors
                            .color_at(byte_in_full)
                            .map(|c| color_to_skia(&c))
                            .or_else(|| {
                                span.text_style.fill_color.value().as_ref().map(|c| color_to_skia(c))
                            })
                    } else {
                        span.text_style.fill_color.value().as_ref().map(|c| color_to_skia(c))
                    }
                } else {
                    span.text_style.fill_color.value().as_ref().map(|c| color_to_skia(c))
                };

                let Some(path) = vector_path_to_skia(&glyph.path, y_cursor) else {
                    continue;
                };

                if let Some(mut color) = fill_color {
                    color.set_alpha(color.alpha() * alpha);
                    let mut paint = Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;
                    pixmap.fill_path(&path, &paint, FillRule::Winding, parent_transform, None);
                }
                if let Some(sc) = span.text_style.stroke_color.value() {
                    let mut color = color_to_skia(sc);
                    color.set_alpha(color.alpha() * alpha);
                    let mut paint = Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;
                    let stroke = Stroke {
                        width: *span.text_style.stroke_width.value() as f32,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&path, &paint, &stroke, parent_transform, None);
                }
            }

            y_cursor += cached.height;
        }
    }
}

// ── Public free functions ─────────────────────────────────────────────────────

pub fn render_scene(scene: &Scene, scale: f32) -> Pixmap {
    RasterRenderer.render_scene(scene, scale)
}

/// Render `scene` fitted into `target_w × target_h`, preserving aspect ratio.
pub fn render_scene_fitted(scene: &Scene, target_w: u32, target_h: u32) -> Pixmap {
    let scale = (target_w as f32 / scene.width as f32).min(target_h as f32 / scene.height as f32);
    let rendered = render_scene(scene, scale);
    let rw = rendered.width();
    let rh = rendered.height();
    if rw == target_w && rh == target_h {
        return rendered;
    }
    let mut canvas = Pixmap::new(target_w, target_h).expect("invalid target resolution");
    canvas.fill(tiny_skia::Color::BLACK);
    let x = ((target_w - rw) / 2) as i32;
    let y = ((target_h - rh) / 2) as i32;
    canvas.draw_pixmap(
        x,
        y,
        rendered.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    canvas
}

/// Renders `scene` fitted (letterboxed) into `width × height` and writes the
/// result into `buffer` as `0x00RRGGBB` u32 values (softbuffer-compatible format).
pub fn render_scene_to_buffer(scene: &Scene, width: u32, height: u32, buffer: &mut [u32]) {
    let pixmap = render_scene_fitted(scene, width, height);
    for (dst, src) in buffer.iter_mut().zip(pixmap.pixels()) {
        *dst = ((src.red() as u32) << 16)
            | ((src.green() as u32) << 8)
            | (src.blue() as u32);
    }
}

// ── Node bounds ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NodeBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Find the axis-aligned bounding box of `node_id` in scene coordinates (scale = 1).
pub fn find_node_bounds(scene: &Scene, node_id: u64) -> Option<NodeBounds> {
    search_children(&scene.children, node_id, Transform::identity())
}

fn search_children(nodes: &[Node], node_id: u64, parent: Transform) -> Option<NodeBounds> {
    nodes.iter().find_map(|n| search_node(n, node_id, parent))
}

fn search_node(node: &Node, node_id: u64, parent: Transform) -> Option<NodeBounds> {
    match &node.kind {
        NodeKind::Group {
            position,
            size,
            scale_x,
            scale_y,
            rotation,
            pivot_x,
            pivot_y,
            children,
            ..
        } => {
            let pivot_x_abs = (pivot_x * size.width) as f32;
            let pivot_y_abs = (pivot_y * size.height) as f32;
            let t = positional_transform(position, *scale_x, *scale_y, *rotation, pivot_x_abs, pivot_y_abs, parent);
            if node.id == node_id {
                return Some(aabb(size.width as f32, size.height as f32, t));
            }
            search_children(children, node_id, t)
        }
        NodeKind::Rect { position, size, .. }
        | NodeKind::Ellipse { position, size, .. }
        | NodeKind::Image { position, size, .. } => {
            if node.id == node_id {
                let t = positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent);
                return Some(aabb(size.width as f32, size.height as f32, t));
            }
            None
        }
        NodeKind::Path { children, .. } => {
            if node.id == node_id {
                return path_bounds(children, parent);
            }
            children
                .iter()
                .find_map(|cmd| search_path_cmd(cmd, node_id, parent))
        }
        NodeKind::Text { position, .. } => {
            if node.id == node_id {
                let t = positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent);
                return Some(aabb(0.0, 0.0, t));
            }
            None
        }
    }
}

fn search_path_cmd(cmd: &PathCommand, node_id: u64, parent: Transform) -> Option<NodeBounds> {
    let (id, pos) = match cmd {
        PathCommand::Move { id, position } => (id, position),
        PathCommand::Line { id, position } => (id, position),
        PathCommand::Cubic { id, position, .. } => (id, position),
        PathCommand::Close => return None,
    };
    if *id == node_id {
        let mut pts = [Point::from_xy(pos.x as f32, pos.y as f32)];
        parent.map_points(&mut pts);
        Some(NodeBounds {
            x: pts[0].x,
            y: pts[0].y,
            width: 0.0,
            height: 0.0,
        })
    } else {
        None
    }
}

fn path_bounds(cmds: &[PathCommand], t: Transform) -> Option<NodeBounds> {
    let mut pts: Vec<Point> = cmds
        .iter()
        .filter_map(|cmd| {
            let pos = match cmd {
                PathCommand::Move { position, .. } => position,
                PathCommand::Line { position, .. } => position,
                PathCommand::Cubic { position, .. } => position,
                PathCommand::Close => return None,
            };
            Some(Point::from_xy(pos.x as f32, pos.y as f32))
        })
        .collect();
    if pts.is_empty() {
        return None;
    }
    t.map_points(&mut pts);
    let min_x = pts.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = pts.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    Some(NodeBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

fn aabb(w: f32, h: f32, t: Transform) -> NodeBounds {
    let mut pts = [
        Point::from_xy(0.0, 0.0),
        Point::from_xy(w, 0.0),
        Point::from_xy(0.0, h),
        Point::from_xy(w, h),
    ];
    t.map_points(&mut pts);
    let min_x = pts.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = pts.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    NodeBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

fn positional_transform(
    position: &Position,
    scale_x: f64,
    scale_y: f64,
    rotation: f64,
    pivot_x: f32,
    pivot_y: f32,
    parent: Transform,
) -> Transform {
    Transform::from_translate(-pivot_x, -pivot_y)
        .post_scale(scale_x as f32, scale_y as f32)
        .post_rotate(rotation as f32)
        .post_translate(position.x as f32 + pivot_x, position.y as f32 + pivot_y)
        .post_concat(parent)
}

fn node_z_level(node: &Node) -> f64 {
    match &node.kind {
        NodeKind::Group { z_level, .. }
        | NodeKind::Rect { z_level, .. }
        | NodeKind::Ellipse { z_level, .. }
        | NodeKind::Path { z_level, .. }
        | NodeKind::Text { z_level, .. }
        | NodeKind::Image { z_level, .. } => *z_level.value(),
    }
}

fn fill_and_stroke(
    path: &tiny_skia::Path,
    style: &Style,
    pixmap: &mut Pixmap,
    transform: Transform,
    parent_alpha: f32,
) {
    let alpha = parent_alpha * style.alpha as f32;
    if let Some(ref fc) = style.fill_color {
        let mut color = color_to_skia(fc);
        color.set_alpha(color.alpha() * alpha);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(path, &paint, FillRule::Winding, transform, None);
    }
    if let Some(ref sc) = style.stroke_color {
        let mut color = color_to_skia(sc);
        color.set_alpha(color.alpha() * alpha);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: style.stroke_width as f32,
            ..Default::default()
        };
        pixmap.stroke_path(path, &paint, &stroke, transform, None);
    }
}

/// Convert a `VectorPath` to a tiny-skia `Path`, shifting all points down by `y_offset`.
fn vector_path_to_skia(vp: &VectorPath, y_offset: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for verb in &vp.verbs {
        match verb {
            PathVerb::MoveTo(x, y) => pb.move_to(*x, y + y_offset),
            PathVerb::LineTo(x, y) => pb.line_to(*x, y + y_offset),
            PathVerb::QuadTo(cx, cy, x, y) => pb.quad_to(*cx, cy + y_offset, *x, y + y_offset),
            PathVerb::CubicTo(cx0, cy0, cx1, cy1, x, y) => {
                pb.cubic_to(*cx0, cy0 + y_offset, *cx1, cy1 + y_offset, *x, y + y_offset)
            }
            PathVerb::Close => pb.close(),
        }
    }
    pb.finish()
}

// ── Path building ─────────────────────────────────────────────────────────────

fn build_path(commands: &[PathCommand]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let mut cur = (0.0f32, 0.0f32);
    for cmd in commands {
        match cmd {
            PathCommand::Move { position, .. } => {
                cur = (position.x as f32, position.y as f32);
                pb.move_to(cur.0, cur.1);
            }
            PathCommand::Line { position, .. } => {
                cur = (position.x as f32, position.y as f32);
                pb.line_to(cur.0, cur.1);
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
                pb.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
                cur = end;
            }
            PathCommand::Close => {
                pb.close();
            }
        }
    }
    pb.finish()
}

/// Arc length of a cubic Bézier (absolute control points) via fixed-step integration.
fn cubic_arc_length_f32(
    p0x: f32, p0y: f32,
    c1x: f32, c1y: f32,
    c2x: f32, c2y: f32,
    p1x: f32, p1y: f32,
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

fn build_cropped_path(commands: &[PathCommand], crop_start: f64, crop_end: f64) -> Option<tiny_skia::Path> {
    if crop_start <= 0.0 && crop_end >= 1.0 {
        return build_path(commands);
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
                segs.push(Seg { sx: cur.0, sy: cur.1, kind: SegKind::Line { ex: end.0, ey: end.1 }, len });
                cur = end;
            }
            PathCommand::Cubic { position, c1_x, c1_y, c2_x, c2_y, .. } => {
                let end = (position.x as f32, position.y as f32);
                let c1 = (cur.0 + *c1_x as f32, cur.1 + *c1_y as f32);
                let c2 = (end.0 + *c2_x as f32, end.1 + *c2_y as f32);
                let len = cubic_arc_length_f32(cur.0, cur.1, c1.0, c1.1, c2.0, c2.1, end.0, end.1);
                segs.push(Seg { sx: cur.0, sy: cur.1, kind: SegKind::Cubic { c1x: c1.0, c1y: c1.1, c2x: c2.0, c2y: c2.1, ex: end.0, ey: end.1 }, len });
                cur = end;
            }
            PathCommand::Close => {
                let (sx, sy) = subpath_start;
                if cur.0 != sx || cur.1 != sy {
                    let dx = sx - cur.0;
                    let dy = sy - cur.1;
                    let len = (dx * dx + dy * dy).sqrt();
                    segs.push(Seg { sx: cur.0, sy: cur.1, kind: SegKind::Line { ex: sx, ey: sy }, len });
                    cur = subpath_start;
                }
            }
        }
    }

    let total_len: f32 = segs.iter().map(|s| s.len).sum();
    if total_len == 0.0 {
        return build_path(commands);
    }

    let start_dist = (crop_start as f32 * total_len).max(0.0);
    let end_dist = (crop_end as f32 * total_len).min(total_len);
    if start_dist >= end_dist {
        return None;
    }

    let mut pb = PathBuilder::new();
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
                    pb.move_to(start_pt.0, start_pt.1);
                }
                pb.line_to(end_pt.0, end_pt.1);
                last_end = Some(end_pt);
            }
            SegKind::Cubic { c1x, c1y, c2x, c2y, ex, ey } => {
                let p0 = (seg.sx, seg.sy);
                let c1 = (*c1x, *c1y);
                let c2 = (*c2x, *c2y);
                let p3 = (*ex, *ey);
                let (sp0, sc1, sc2, sp3) = cubic_subsegment(p0, c1, c2, p3, t1, t2);
                if last_end != Some(sp0) {
                    pb.move_to(sp0.0, sp0.1);
                }
                pb.cubic_to(sc1.0, sc1.1, sc2.0, sc2.1, sp3.0, sp3.1);
                last_end = Some(sp3);
            }
        }

        accumulated = seg_end_acc;
    }

    pb.finish()
}

// ── Image rendering ───────────────────────────────────────────────────────────

fn render_raster_pixmap(
    src: &RawPixmap,
    sx: f32,
    sy: f32,
    offset_x: f32,
    offset_y: f32,
    node_transform: Transform,
    pixmap: &mut Pixmap,
    effective_alpha: f32,
    dest_w: f32,
    dest_h: f32,
) {
    let Some(src_pixmap) = raw_to_pixmap(src) else { return };
    let mut paint = PixmapPaint::default();
    paint.quality = tiny_skia::FilterQuality::Bilinear;
    if (effective_alpha - 1.0).abs() < 1e-6 {
        let transform = Transform::from_scale(sx, sy)
            .post_translate(offset_x, offset_y)
            .post_concat(node_transform);
        pixmap.draw_pixmap(0, 0, src_pixmap.as_ref(), &paint, transform, None);
    } else {
        let w_u32 = dest_w.ceil() as u32;
        let h_u32 = dest_h.ceil() as u32;
        let Some(mut img_pixmap) = Pixmap::new(w_u32.max(1), h_u32.max(1)) else { return };
        let inner_transform = Transform::from_scale(sx, sy).post_translate(offset_x, offset_y);
        img_pixmap.draw_pixmap(0, 0, src_pixmap.as_ref(), &paint, inner_transform, None);
        let mut composite_paint = PixmapPaint::default();
        composite_paint.opacity = effective_alpha.clamp(0.0, 1.0);
        pixmap.draw_pixmap(0, 0, img_pixmap.as_ref(), &composite_paint, node_transform, None);
    }
}

fn render_svg_tree(
    tree: &usvg::Tree,
    sx: f32,
    sy: f32,
    offset_x: f32,
    offset_y: f32,
    node_transform: Transform,
    pixmap: &mut Pixmap,
    effective_alpha: f32,
    dest_w: f32,
    dest_h: f32,
) {
    if (effective_alpha - 1.0).abs() < 1e-6 {
        let svg_transform = Transform::from_scale(sx, sy)
            .post_translate(offset_x, offset_y)
            .post_concat(node_transform);
        resvg::render(tree, svg_transform, &mut pixmap.as_mut());
    } else {
        let w_u32 = dest_w.ceil() as u32;
        let h_u32 = dest_h.ceil() as u32;
        let Some(mut img_pixmap) = Pixmap::new(w_u32.max(1), h_u32.max(1)) else {
            return;
        };
        let svg_transform =
            Transform::from_scale(sx, sy).post_translate(offset_x, offset_y);
        resvg::render(tree, svg_transform, &mut img_pixmap.as_mut());
        let mut paint = PixmapPaint::default();
        paint.opacity = effective_alpha.clamp(0.0, 1.0);
        pixmap.draw_pixmap(0, 0, img_pixmap.as_ref(), &paint, node_transform, None);
    }
}

fn render_image(
    path: &str,
    size: &Size,
    keep_aspect: bool,
    layers: &[ImageLayer],
    hidden_layers: &[Arc<String>],
    pixmap: &mut Pixmap,
    parent_transform: Transform,
    position: &Position,
    effective_alpha: f32,
) {
    let Some(cached) = image_cache::load_image(path) else { return };
    let dest_w = size.width as f32;
    let dest_h = size.height as f32;
    if dest_w <= 0.0 || dest_h <= 0.0 || cached.width <= 0.0 || cached.height <= 0.0 {
        return;
    }

    let (sx, sy, offset_x, offset_y) = if keep_aspect {
        let s = (dest_w / cached.width).min(dest_h / cached.height);
        let actual_w = cached.width * s;
        let actual_h = cached.height * s;
        (s, s, (dest_w - actual_w) / 2.0, (dest_h - actual_h) / 2.0)
    } else {
        (dest_w / cached.width, dest_h / cached.height, 0.0, 0.0)
    };

    let node_transform = positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent_transform);

    match &cached.kind {
        CachedImageKind::Svg { tree, .. } => {
            if layers.is_empty() && hidden_layers.is_empty() {
                render_svg_tree(tree, sx, sy, offset_x, offset_y, node_transform, pixmap, effective_alpha, dest_w, dest_h);
            } else {
                let all_labels = cached.image_layers.as_ref();
                if all_labels.is_none_or(|labels| labels.is_empty()) {
                    render_svg_tree(tree, sx, sy, offset_x, offset_y, node_transform, pixmap, effective_alpha, dest_w, dest_h);
                } else {
                    for label in all_labels.unwrap().iter() {
                        if hidden_layers.iter().any(|h| **h == *label) {
                            continue;
                        }
                        let override_ = layers.iter().find(|l| l.layer_name.as_str() == label.as_str());
                        let layer_alpha = override_
                            .map(|ov| effective_alpha * ov.alpha as f32)
                            .unwrap_or(effective_alpha);
                        if layer_alpha <= 0.0 {
                            continue;
                        }
                        let (lx, ly) = override_
                            .map(|ov| (ov.position.x as f32, ov.position.y as f32))
                            .unwrap_or((0.0, 0.0));
                        let Some(layer_cached) = image_cache::load_svg_layer(path, label) else { continue };
                        let layer_tree = match &layer_cached.kind {
                            CachedImageKind::Svg { tree, .. } => tree,
                            _ => continue,
                        };
                        let layer_transform = node_transform.post_translate(lx, ly);
                        render_svg_tree(layer_tree, sx, sy, offset_x, offset_y, layer_transform, pixmap, layer_alpha, dest_w, dest_h);
                    }
                }
            }
        }
        CachedImageKind::Raster { pixmap: src } => {
            render_raster_pixmap(src, sx, sy, offset_x, offset_y, node_transform, pixmap, effective_alpha, dest_w, dest_h);
        }
        CachedImageKind::Ora { layers: ora_layers } => {
            let all_labels = &cached.image_layers;
            if layers.is_empty() && hidden_layers.is_empty() {
                for layer_data in ora_layers.iter() {
                    let layer_ox = offset_x + layer_data.x as f32 * sx;
                    let layer_oy = offset_y + layer_data.y as f32 * sy;
                    render_raster_pixmap(&layer_data.pixmap, sx, sy, layer_ox, layer_oy, node_transform, pixmap, effective_alpha, dest_w, dest_h);
                }
            } else if let Some(all_labels) = all_labels {
                for label in all_labels.iter() {
                    if hidden_layers.iter().any(|h| **h == *label) {
                        continue;
                    }
                    let Some(layer_data) = ora_layers.iter().find(|l| l.name == label.as_str()) else {
                        continue;
                    };
                    let override_ = layers.iter().find(|l| l.layer_name.as_str() == label.as_str());
                    let layer_alpha = override_
                        .map(|ov| effective_alpha * ov.alpha as f32)
                        .unwrap_or(effective_alpha);
                    if layer_alpha <= 0.0 {
                        continue;
                    }
                    let (lx, ly) = override_
                        .map(|ov| (ov.position.x as f32, ov.position.y as f32))
                        .unwrap_or((0.0, 0.0));
                    let layer_ox = offset_x + layer_data.x as f32 * sx + lx;
                    let layer_oy = offset_y + layer_data.y as f32 * sy + ly;
                    render_raster_pixmap(&layer_data.pixmap, sx, sy, layer_ox, layer_oy, node_transform, pixmap, layer_alpha, dest_w, dest_h);
                }
            }
        }
    }
}
