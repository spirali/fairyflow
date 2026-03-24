use serde::Serialize;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Point, Rect, Stroke, Transform};
use parley::{Alignment, AlignmentOptions, FontContext, FontStack, LayoutContext, PositionedLayoutItem, StyleProperty};
use skrifa::{GlyphId, MetadataProvider, instance::{LocationRef, NormalizedCoord, Size as SkrifaSize}, outline::{DrawSettings, OutlinePen}, raw::FontRef as ReadFontsRef};

use crate::scene::{NodeKind, PathCommand, Position, Scene, Node, Style, TextLine};

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
        NodeKind::Group { position, size, alpha: _, scale_x, scale_y, rotation, children } => {
            let t = positional_transform(position, *scale_x, *scale_y, *rotation, parent);
            if node.id == node_id {
                return Some(aabb(size.width as f32, size.height as f32, t));
            }
            search_children(children, node_id, t)
        }
        NodeKind::Rect { position, size, .. } | NodeKind::Ellipse { position, size, .. } => {
            if node.id == node_id {
                let t = positional_transform(position, 1.0, 1.0, 0.0, parent);
                return Some(aabb(size.width as f32, size.height as f32, t));
            }
            None
        }
        NodeKind::Path { children, .. } => {
            if node.id == node_id {
                return path_bounds(children, parent);
            }
            children.iter().find_map(|cmd| search_path_cmd(cmd, node_id, parent))
        }
        NodeKind::Text { position, .. } => {
            if node.id == node_id {
                let t = positional_transform(position, 1.0, 1.0, 0.0, parent);
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
    };
    if *id == node_id {
        let mut pts = [Point::from_xy(pos.x as f32, pos.y as f32)];
        parent.map_points(&mut pts);
        Some(NodeBounds { x: pts[0].x, y: pts[0].y, width: 0.0, height: 0.0 })
    } else {
        None
    }
}

fn path_bounds(cmds: &[PathCommand], t: Transform) -> Option<NodeBounds> {
    let mut pts: Vec<Point> = cmds.iter().map(|cmd| {
        let pos = match cmd {
            PathCommand::Move { position, .. } => position,
            PathCommand::Line { position, .. } => position,
            PathCommand::Cubic { position, .. } => position,
        };
        Point::from_xy(pos.x as f32, pos.y as f32)
    }).collect();
    if pts.is_empty() { return None; }
    t.map_points(&mut pts);
    let min_x = pts.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = pts.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    Some(NodeBounds { x: min_x, y: min_y, width: max_x - min_x, height: max_y - min_y })
}

fn aabb(w: f32, h: f32, t: Transform) -> NodeBounds {
    let mut pts = [
        Point::from_xy(0.0, 0.0), Point::from_xy(w, 0.0),
        Point::from_xy(0.0, h),   Point::from_xy(w, h),
    ];
    t.map_points(&mut pts);
    let min_x = pts.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let min_y = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_x = pts.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let max_y = pts.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    NodeBounds { x: min_x, y: min_y, width: max_x - min_x, height: max_y - min_y }
}

pub fn render_scene(scene: &Scene, scale: f32) -> Pixmap {
    let width = (scene.width as f32 * scale).round() as u32;
    let height = (scene.height as f32 * scale).round() as u32;
    let mut pixmap = Pixmap::new(width.max(1), height.max(1)).expect("invalid scene dimensions");
    pixmap.fill(scene.fill_color.to_skia_color());
    render_children(&scene.children, &mut pixmap, Transform::from_scale(scale, scale), 1.0);
    pixmap
}

fn render_children(nodes: &[Node], pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
    for node in nodes {
        render_node(node, pixmap, parent_transform, parent_alpha);
    }
}

fn render_node(node: &Node, pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
    match &node.kind {
        NodeKind::Group { position, size: _, alpha, scale_x, scale_y, rotation, children } => {
            let transform = positional_transform(position, *scale_x, *scale_y, *rotation, parent_transform);
            render_children(children, pixmap, transform, parent_alpha * *alpha as f32);
        }
        NodeKind::Rect { position, size, style } => {
            let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
            let Some(rect) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32) else { return };
            let path = PathBuilder::from_rect(rect);
            fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
        }
        NodeKind::Ellipse { position, size, style } => {
            let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
            let Some(oval) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32) else { return };
            let Some(path) = PathBuilder::from_oval(oval) else { return };
            fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
        }
        NodeKind::Path { style, children } => {
            if let Some(path) = build_path(children) {
                fill_and_stroke(&path, style, pixmap, parent_transform, parent_alpha);
            }
        }
        NodeKind::Text { position, lines } => {
            let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
            render_text(lines, pixmap, transform, parent_alpha);
        }
    }
}

fn positional_transform(position: &Position, scale_x: f64, scale_y: f64, rotation: f64, parent: Transform) -> Transform {
    // Order: Scale → Rotate → Translate, so the node's position is stable under scale/rotation.
    Transform::from_scale(scale_x as f32, scale_y as f32)
        .post_rotate(rotation as f32)
        .post_translate(position.x as f32, position.y as f32)
        .post_concat(parent)
}

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
            PathCommand::Cubic { position, c1_x, c1_y, c2_x, c2_y, .. } => {
                let end = (position.x as f32, position.y as f32);
                // c1 is relative to the start (cur), c2 is relative to the end point
                let c1 = (cur.0 + *c1_x as f32, cur.1 + *c1_y as f32);
                let c2 = (end.0 + *c2_x as f32, end.1 + *c2_y as f32);
                pb.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
                cur = end;
            }
        }
    }
    pb.finish()
}

const DEFAULT_FONT_SIZE: f32 = 16.0;

fn render_text(lines: &[TextLine], pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
    let mut font_cx = FontContext::new();
    let mut layout_cx: LayoutContext<usize> = LayoutContext::new();

    let mut y_cursor = 0.0f32;
    for line in lines {
        // Concatenate all span texts to build the full line string, tracking byte ranges.
        let mut full_text = String::new();
        let mut ranges: Vec<(std::ops::Range<usize>, usize)> = Vec::new();
        for (i, span) in line.spans.iter().enumerate() {
            let start = full_text.len();
            full_text.push_str(&span.text);
            ranges.push((start..full_text.len(), i));
        }
        if full_text.is_empty() {
            continue;
        }

        let mut builder = layout_cx.ranged_builder(&mut font_cx, &full_text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(DEFAULT_FONT_SIZE));

        for (range, span_idx) in &ranges {
            let span = &line.spans[*span_idx];
            builder.push(StyleProperty::Brush(*span_idx), range.clone());
            builder.push(StyleProperty::FontStack(FontStack::Source((span.font_family.as_str()).into())), range.clone());
            if span.italic {
                builder.push(StyleProperty::FontStyle(parley::FontStyle::Italic), range.clone());
            }
        }

        let mut layout = builder.build(&full_text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        let line_height = layout.height();

        for layout_line in layout.lines() {
            for item in layout_line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else { continue };

                // Read the span index from the brush we stored in the parley style.
                let style_idx = glyph_run.glyphs().next()
                    .map(|g| g.style_index())
                    .unwrap_or(0);
                let span_idx = layout.styles()
                    .get(style_idx)
                    .map(|s| s.brush)
                    .unwrap_or(0);
                let span = &line.spans[span_idx.min(line.spans.len().saturating_sub(1))];
                let fill_color = span.style.fill_color.as_ref().map(|c| c.to_skia_color());
                let alpha = parent_alpha * span.style.alpha as f32;

                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let normalized_coords: Vec<NormalizedCoord> = run.normalized_coords().iter()
                    .map(|c| NormalizedCoord::from_bits(*c))
                    .collect();

                let font_ref = ReadFontsRef::from_index(font.data.as_ref(), font.index).unwrap();
                let outlines = font_ref.outline_glyphs();

                let mut run_x = glyph_run.offset();
                let run_y = glyph_run.baseline() + y_cursor;

                for glyph in glyph_run.glyphs() {
                    let gx = run_x + glyph.x;
                    let gy = run_y - glyph.y;
                    run_x += glyph.advance;

                    let glyph_id = GlyphId::from(glyph.id as u16);
                    let Some(outline) = outlines.get(glyph_id) else { continue };

                    let settings = DrawSettings::unhinted(
                        SkrifaSize::new(font_size),
                        LocationRef::new(&normalized_coords),
                    );
                    let mut pen = GlyphPen { x: gx, y: gy, pb: PathBuilder::new() };
                    let _ = outline.draw(settings, &mut pen);
                    let Some(path) = pen.pb.finish() else { continue };

                    if let Some(mut color) = fill_color {
                        color.set_alpha(color.alpha() * alpha);
                        let mut paint = Paint::default();
                        paint.set_color(color);
                        paint.anti_alias = true;
                        pixmap.fill_path(&path, &paint, FillRule::Winding, parent_transform, None);
                    }
                    if let Some(ref sc) = span.style.stroke_color {
                        let mut color = sc.to_skia_color();
                        color.set_alpha(color.alpha() * alpha);
                        let mut paint = Paint::default();
                        paint.set_color(color);
                        paint.anti_alias = true;
                        let stroke = Stroke { width: span.style.stroke_width as f32, ..Default::default() };
                        pixmap.stroke_path(&path, &paint, &stroke, parent_transform, None);
                    }
                }
            }
        }
        y_cursor += line_height;
    }
}

struct GlyphPen {
    x: f32,
    y: f32,
    pb: PathBuilder,
}

impl OutlinePen for GlyphPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.x + x, self.y - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.x + x, self.y - y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.pb.quad_to(self.x + cx0, self.y - cy0, self.x + x, self.y - y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.pb.cubic_to(self.x + cx0, self.y - cy0, self.x + cx1, self.y - cy1, self.x + x, self.y - y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

fn fill_and_stroke(path: &tiny_skia::Path, style: &Style, pixmap: &mut Pixmap, transform: Transform, parent_alpha: f32) {
    let alpha = parent_alpha * style.alpha as f32;
    if let Some(ref fc) = style.fill_color {
        let mut color = fc.to_skia_color();
        color.set_alpha(color.alpha() * alpha);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(path, &paint, FillRule::Winding, transform, None);
    }
    if let Some(ref sc) = style.stroke_color {
        let mut color = sc.to_skia_color();
        color.set_alpha(color.alpha() * alpha);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke { width: style.stroke_width as f32, ..Default::default() };
        pixmap.stroke_path(path, &paint, &stroke, transform, None);
    }
}
