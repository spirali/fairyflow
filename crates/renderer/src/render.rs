use std::cell::RefCell;
use serde::Serialize;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Point, Rect, Stroke, Transform};
use parley::{Alignment, AlignmentOptions, FontContext, FontStack, LayoutContext, PositionedLayoutItem, StyleProperty};
use skrifa::{GlyphId, MetadataProvider, instance::{LocationRef, NormalizedCoord, Size as SkrifaSize}, outline::{DrawSettings, OutlinePen}, raw::FontRef as ReadFontsRef};
use crate::resources::Resources;
use crate::scene::{NodeKind, PathCommand, Position, Scene, Node, Style, TextChild, TextSpan};

thread_local! {
    static RENDERER: RefCell<Renderer> = RefCell::new(Renderer::new(Resources::get()));
}

pub struct Renderer {
    font_cx: FontContext,
    layout_cx: LayoutContext<usize>,
}

impl Renderer {
    pub fn new(resources: &Resources) -> Renderer {
        Renderer {
            font_cx: resources.font_cx().clone(),
            layout_cx: LayoutContext::new(),
        }
    }

    pub fn render_scene(&mut self, scene: &Scene, scale: f32) -> Pixmap {
        let width = (scene.width as f32 * scale).round() as u32;
        let height = (scene.height as f32 * scale).round() as u32;
        let mut pixmap = Pixmap::new(width.max(1), height.max(1)).expect("invalid scene dimensions");
        pixmap.fill(scene.fill_color.to_skia_color());
        self.render_children(&scene.children, &mut pixmap, Transform::from_scale(scale, scale), 1.0);
        pixmap
    }

    fn render_children(&mut self, nodes: &[Node], pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
        for node in nodes {
            self.render_node(node, pixmap, parent_transform, parent_alpha);
        }
    }

    fn render_node(&mut self, node: &Node, pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
        match &node.kind {
            NodeKind::Group { position, size: _, alpha, scale_x, scale_y, rotation, children } => {
                let transform = positional_transform(position, *scale_x, *scale_y, *rotation, parent_transform);
                let alpha = parent_alpha * *alpha as f32;
                // Clone to avoid holding a borrow on node.kind while calling self methods.
                let children = children.clone();
                self.render_children(&children, pixmap, transform, alpha);
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
                let lines = lines.clone();
                self.render_text_lines(&lines, pixmap, transform, parent_alpha);
            }
        }
    }

    fn render_text_lines(&mut self, lines: &[TextChild], pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
        let mut y_cursor = 0.0f32;
        for line in lines {
            let mut spans: Vec<&TextSpan> = Vec::new();
            collect_spans(line, &mut spans);

            let (full_text, ranges) = build_span_text(&spans);
            if full_text.is_empty() { continue; }

            let mut builder = self.layout_cx.ranged_builder(&mut self.font_cx, &full_text, 1.0, true);
            builder.push_default(StyleProperty::FontSize(DEFAULT_FONT_SIZE));
            for (range, span_idx) in &ranges {
                let span = spans[*span_idx];
                builder.push(StyleProperty::Brush(*span_idx), range.clone());
                builder.push(StyleProperty::FontSize(span.font_size as f32), range.clone());
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
                        // Look up the span per-glyph so each glyph uses its own style,
                        // even if parley placed multiple spans into one glyph run.
                        let span_idx = layout.styles()
                            .get(glyph.style_index())
                            .map(|s| s.brush)
                            .unwrap_or(0)
                            .min(spans.len().saturating_sub(1));
                        let span = spans[span_idx];
                        let fill_color = span.style.fill_color.as_ref().map(|c| c.to_skia_color());
                        let alpha = parent_alpha * span.style.alpha as f32;

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

    fn measure_text_lines(&mut self, lines: &[TextChild]) -> (f32, f32) {
        let mut total_width = 0.0f32;
        let mut total_height = 0.0f32;
        for line in lines {
            let mut spans: Vec<&TextSpan> = Vec::new();
            collect_spans(line, &mut spans);
            if spans.is_empty() { continue; }

            let (full_text, ranges) = build_span_text(&spans);
            if full_text.is_empty() { continue; }

            let mut builder = self.layout_cx.ranged_builder(&mut self.font_cx, &full_text, 1.0, true);
            builder.push_default(StyleProperty::FontSize(DEFAULT_FONT_SIZE));
            for (range, span_idx) in &ranges {
                let span = spans[*span_idx];
                builder.push(StyleProperty::FontSize(span.font_size as f32), range.clone());
                builder.push(StyleProperty::FontStack(FontStack::Source((span.font_family.as_str()).into())), range.clone());
                if span.italic {
                    builder.push(StyleProperty::FontStyle(parley::FontStyle::Italic), range.clone());
                }
            }
            let mut layout = builder.build(&full_text);
            layout.break_all_lines(None);
            layout.align(None, Alignment::Start, AlignmentOptions::default());

            total_width = total_width.max(layout.width());
            total_height += layout.height();
        }
        (total_width, total_height)
    }

    /// Find the top-left position of the first glyph run belonging to `target_id`
    /// within the text block described by `lines`.
    /// Returns `(x, y)` relative to the text node's origin, or `None` if not found.
    fn find_text_node_pos(&mut self, lines: &[TextChild], target_id: u64) -> Option<(f32, f32)> {
        let mut y_offset = 0.0f32;
        for line in lines {
            let mut tagged: Vec<(bool, &TextSpan)> = Vec::new();
            collect_spans_tagged(line, target_id, false, &mut tagged);

            let spans: Vec<&TextSpan> = tagged.iter().map(|(_, s)| *s).collect();
            if spans.is_empty() { continue; }

            let (full_text, ranges) = build_span_text(&spans);

            let mut builder = self.layout_cx.ranged_builder(&mut self.font_cx, &full_text, 1.0, true);
            builder.push_default(StyleProperty::FontSize(DEFAULT_FONT_SIZE));
            for (range, span_idx) in &ranges {
                let span = spans[*span_idx];
                builder.push(StyleProperty::Brush(*span_idx), range.clone());
                builder.push(StyleProperty::FontSize(span.font_size as f32), range.clone());
                builder.push(StyleProperty::FontStack(FontStack::Source((span.font_family.as_str()).into())), range.clone());
                if span.italic {
                    builder.push(StyleProperty::FontStyle(parley::FontStyle::Italic), range.clone());
                }
            }
            let mut layout = builder.build(&full_text);
            layout.break_all_lines(None);
            layout.align(None, Alignment::Start, AlignmentOptions::default());

            let first_target_idx = tagged.iter().position(|(is_target, _)| *is_target);
            if let Some(target_span_idx) = first_target_idx {
                // Walk every glyph individually — a single glyph run may contain glyphs
                // from several spans if parley does not split on brush changes.
                let mut x = 0.0f32;
                'search: for layout_line in layout.lines() {
                    for item in layout_line.items() {
                        let PositionedLayoutItem::GlyphRun(glyph_run) = item else { continue };
                        let mut run_x = glyph_run.offset();
                        for glyph in glyph_run.glyphs() {
                            let span_idx = layout.styles()
                                .get(glyph.style_index())
                                .map(|s| s.brush)
                                .unwrap_or(usize::MAX);
                            if span_idx == target_span_idx {
                                x = run_x;
                                break 'search;
                            }
                            run_x += glyph.advance;
                        }
                    }
                }
                return Some((x, y_offset));
            }

            y_offset += layout.height();
        }
        None
    }
}

/// Render a scene using the per-thread `Renderer` (initialised once per thread).
pub fn render_scene(scene: &Scene, scale: f32) -> Pixmap {
    RENDERER.with(|r| r.borrow_mut().render_scene(scene, scale))
}

/// Measure the natural (unwrapped) dimensions of a text block.
/// Returns `(width, height)` in logical pixels (scale = 1).
pub fn measure_text(lines: &[TextChild]) -> (f32, f32) {
    RENDERER.with(|r| r.borrow_mut().measure_text_lines(lines))
}

/// Find the position `(x, y)` of the first glyph belonging to the node with
/// `target_id` within the text block.  `y` is the top of the line that contains
/// the target; `x` is the left edge of its first glyph run.
/// Returns `None` if the target id is not found in the tree.
pub fn measure_text_node_pos(lines: &[TextChild], target_id: u64) -> Option<(f32, f32)> {
    RENDERER.with(|r| r.borrow_mut().find_text_node_pos(lines, target_id))
}

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

fn positional_transform(position: &Position, scale_x: f64, scale_y: f64, rotation: f64, parent: Transform) -> Transform {
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

/// Concatenates span texts separated by U+200C (ZERO WIDTH NON-JOINER).
/// The ZWNJ is invisible and zero-width but tells HarfBuzz not to form ligatures
/// across the boundary — preventing "ff" from two consecutive spans collapsing into
/// one ligature glyph that hides the second span from the layout.
/// Each returned range covers a span's text plus its trailing ZWNJ (if any).
fn build_span_text(spans: &[&TextSpan]) -> (String, Vec<(std::ops::Range<usize>, usize)>) {
    let mut full_text = String::new();
    let mut ranges = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        let start = full_text.len();
        full_text.push_str(&span.text);
        if i + 1 < spans.len() {
            full_text.push('\u{200C}');
        }
        ranges.push((start..full_text.len(), i));
    }
    (full_text, ranges)
}

fn collect_spans<'a>(child: &'a TextChild, out: &mut Vec<&'a TextSpan>) {
    match child {
        TextChild::Span(s) => out.push(s),
        TextChild::Group(g) => {
            for c in &g.children {
                collect_spans(c, out);
            }
        }
    }
}

/// Like `collect_spans`, but tags each span with whether it is inside the subtree
/// rooted at `target_id` (including the target itself if it is a span).
fn collect_spans_tagged<'a>(child: &'a TextChild, target_id: u64, in_target: bool, out: &mut Vec<(bool, &'a TextSpan)>) {
    match child {
        TextChild::Span(s) => out.push((in_target || s.id == target_id, s)),
        TextChild::Group(g) => {
            let inside = in_target || g.id == target_id;
            for c in &g.children {
                collect_spans_tagged(c, target_id, inside, out);
            }
        }
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
