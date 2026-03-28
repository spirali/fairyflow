use crate::glyph_cache::{self, CachedLine, PathVerb, VectorPath};
use crate::resources::Resources;
use crate::scene::{Node, NodeKind, PathCommand, Position, Scene, Style, TextChild, TextSpan};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontStack, LayoutContext, PositionedLayoutItem,
    StyleProperty,
};
use serde::Serialize;
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{LocationRef, NormalizedCoord, Size as SkrifaSize},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef as ReadFontsRef,
};
use std::cell::RefCell;
use std::sync::Arc;
use tiny_skia::{
    FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Point, Rect, Stroke, Transform,
};

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
            font_cx: resources.font_cx(),
            layout_cx: LayoutContext::new(),
        }
    }

    pub fn render_scene(&mut self, scene: &Scene, scale: f32) -> Pixmap {
        let width = (scene.width as f32 * scale).round() as u32;
        let height = (scene.height as f32 * scale).round() as u32;
        let mut pixmap =
            Pixmap::new(width.max(1), height.max(1)).expect("invalid scene dimensions");
        pixmap.fill(scene.fill_color.to_skia_color());
        self.render_children(
            &scene.children,
            &mut pixmap,
            Transform::from_scale(scale, scale),
            1.0,
        );
        pixmap
    }

    fn render_children(
        &mut self,
        nodes: &[Node],
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
    ) {
        for node in nodes {
            self.render_node(node, pixmap, parent_transform, parent_alpha);
        }
    }

    fn render_node(
        &mut self,
        node: &Node,
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
    ) {
        match &node.kind {
            NodeKind::Group {
                position,
                size: _,
                alpha,
                scale_x,
                scale_y,
                rotation,
                z_level,
                children,
            } => {
                let transform =
                    positional_transform(position, *scale_x, *scale_y, *rotation, parent_transform);
                let alpha = parent_alpha * *alpha as f32;
                // Clone to avoid holding a borrow on node.kind while calling self methods.
                let children = children.clone();
                self.render_children(&children, pixmap, transform, alpha);
            }
            NodeKind::Rect {
                position,
                size,
                style,
                z_level
            } => {
                let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
                let Some(rect) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32)
                else {
                    return;
                };
                let path = PathBuilder::from_rect(rect);
                fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
            }
            NodeKind::Ellipse {
                position,
                size,
                style,
                z_level
            } => {
                let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
                let Some(oval) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32)
                else {
                    return;
                };
                let Some(path) = PathBuilder::from_oval(oval) else {
                    return;
                };
                fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
            }
            NodeKind::Path { style, children, z_level } => {
                if let Some(path) = build_path(children) {
                    fill_and_stroke(&path, style, pixmap, parent_transform, parent_alpha);
                }
            }
            NodeKind::Text { position, lines, z_level } => {
                let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
                let lines = lines.clone();
                self.render_text_lines(&lines, pixmap, transform, parent_alpha);
            }
        }
    }

    fn render_text_lines(
        &mut self,
        lines: &[TextChild],
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
    ) {
        let mut y_cursor = 0.0f32;
        for line in lines {
            let mut spans: Vec<&TextSpan> = Vec::new();
            collect_spans(line, &mut spans);
            if spans.is_empty() {
                continue;
            }

            let cached = self.get_or_build_line(&spans);

            for glyph in &cached.glyphs {
                let span = spans[glyph.span_idx];
                let fill_color = span.style.fill_color.as_ref().map(|c| c.to_skia_color());
                let alpha = parent_alpha * span.style.alpha as f32;

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
                if let Some(ref sc) = span.style.stroke_color {
                    let mut color = sc.to_skia_color();
                    color.set_alpha(color.alpha() * alpha);
                    let mut paint = Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;
                    let stroke = Stroke {
                        width: span.style.stroke_width as f32,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&path, &paint, &stroke, parent_transform, None);
                }
            }

            y_cursor += cached.height;
        }
    }

    fn measure_text_lines(&mut self, lines: &[TextChild]) -> (f32, f32) {
        let mut total_width = 0.0f32;
        let mut total_height = 0.0f32;
        for line in lines {
            let mut spans: Vec<&TextSpan> = Vec::new();
            collect_spans(line, &mut spans);
            if spans.is_empty() {
                continue;
            }
            let cached = self.get_or_build_line(&spans);
            total_width = total_width.max(cached.width);
            total_height += cached.height;
        }
        (total_width, total_height)
    }

    /// Find the top-left position of the first glyph belonging to `target_id`
    /// within the text block described by `lines`.
    /// Returns `(x, y)` relative to the text node's origin, or `None` if not found.
    fn find_text_node_pos(&mut self, lines: &[TextChild], target_id: u64) -> Option<(f32, f32)> {
        let mut y_offset = 0.0f32;
        for line in lines {
            let mut tagged: Vec<(bool, &TextSpan)> = Vec::new();
            collect_spans_tagged(line, target_id, false, &mut tagged);

            let spans: Vec<&TextSpan> = tagged.iter().map(|(_, s)| *s).collect();
            if spans.is_empty() {
                continue;
            }

            let cached = self.get_or_build_line(&spans);

            // Find which span index corresponds to the target.
            let first_target_idx = tagged.iter().position(|(is_target, _)| *is_target);
            if let Some(target_span_idx) = first_target_idx {
                // Find the first cached glyph that belongs to the target span.
                if let Some(glyph) = cached.glyphs.iter().find(|g| g.span_idx == target_span_idx) {
                    return Some((glyph.x, y_offset));
                }
            }

            y_offset += cached.height;
        }
        None
    }

    fn get_or_build_line(&mut self, spans: &[&TextSpan]) -> Arc<CachedLine> {
        let key = make_line_key(spans);
        if let Some(cached) = glyph_cache::cache_get(&key) {
            return cached;
        }
        self.build_cached_line(spans, key)
    }

    /// Run parley layout for `spans`, extract glyph outlines into `CachedGlyph`s,
    /// store in the global cache, and return the entry.
    fn build_cached_line(
        &mut self,
        spans: &[&TextSpan],
        key: glyph_cache::LineKey,
    ) -> Arc<CachedLine> {
        let (full_text, ranges) = build_span_text(spans);
        if full_text.is_empty() {
            // Don't cache empty lines — they're trivial and have no spans to key on.
            return Arc::new(CachedLine {
                width: 0.0,
                height: 0.0,
                glyphs: vec![],
            });
        }

        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, &full_text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(DEFAULT_FONT_SIZE));
        for (range, span_idx) in &ranges {
            let span = spans[*span_idx];
            // Brush carries the span index so we can look it up per-glyph below.
            builder.push(StyleProperty::Brush(*span_idx), range.clone());
            builder.push(
                StyleProperty::FontSize(span.font_size as f32),
                range.clone(),
            );
            builder.push(
                StyleProperty::FontStack(FontStack::Source((span.font_family.as_str()).into())),
                range.clone(),
            );
            if span.italic {
                builder.push(
                    StyleProperty::FontStyle(parley::FontStyle::Italic),
                    range.clone(),
                );
            }
        }
        let mut layout = builder.build(&full_text);
        layout.break_all_lines(None);
        layout.align(None, Alignment::Start, AlignmentOptions::default());

        let width = layout.width();
        let height = layout.height();
        let mut glyphs = Vec::new();

        for layout_line in layout.lines() {
            for item in layout_line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let normalized_coords: Vec<NormalizedCoord> = run
                    .normalized_coords()
                    .iter()
                    .map(|c| NormalizedCoord::from_bits(*c))
                    .collect();

                let font_ref = ReadFontsRef::from_index(font.data.as_ref(), font.index).unwrap();
                let outlines = font_ref.outline_glyphs();

                let mut run_x = glyph_run.offset();
                // baseline without y_cursor — y_cursor is added at render time via y_offset
                let baseline = glyph_run.baseline();

                for glyph in glyph_run.glyphs() {
                    // Per-glyph span lookup: a single run may span multiple style ranges.
                    let span_idx = layout
                        .styles()
                        .get(glyph.style_index())
                        .map(|s| s.brush)
                        .unwrap_or(0)
                        .min(spans.len().saturating_sub(1));

                    let gx = run_x + glyph.x;
                    let gy = baseline - glyph.y;
                    run_x += glyph.advance;

                    let glyph_id = GlyphId::from(glyph.id as u16);
                    let Some(outline) = outlines.get(glyph_id) else {
                        continue;
                    };

                    let settings = DrawSettings::unhinted(
                        SkrifaSize::new(font_size),
                        LocationRef::new(&normalized_coords),
                    );
                    let mut pen = GlyphPen::new(gx, gy);
                    let _ = outline.draw(settings, &mut pen);
                    glyphs.push(glyph_cache::CachedGlyph {
                        span_idx,
                        x: gx,
                        path: VectorPath { verbs: pen.verbs },
                    });
                }
            }
        }

        glyph_cache::cache_store(
            key,
            CachedLine {
                width,
                height,
                glyphs,
            },
        )
    }
}

/// Render a scene using the per-thread `Renderer` (initialised once per thread).
pub fn render_scene(scene: &Scene, scale: f32) -> Pixmap {
    RENDERER.with(|r| r.borrow_mut().render_scene(scene, scale))
}

/// Render `scene` fitted into `target_w × target_h`, preserving aspect ratio.
/// The scene is scaled to fill as much of the target as possible; any remaining
/// area is filled with black (letterbox / pillarbox).
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

/// Measure the natural (unwrapped) dimensions of a text block.
/// Returns `(width, height)` in logical pixels (scale = 1).
pub fn measure_text(lines: &[TextChild]) -> (f32, f32) {
    RENDERER.with(|r| r.borrow_mut().measure_text_lines(lines))
}

/// Find the position `(x, y)` of the first glyph belonging to the node with
/// `target_id` within the text block.  `y` is the top of the line that contains
/// the target; `x` is the left edge of its first glyph.
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
        NodeKind::Group {
            position,
            size,
            alpha: _,
            scale_x,
            scale_y,
            rotation,
            children,
            z_level,
        } => {
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
            children
                .iter()
                .find_map(|cmd| search_path_cmd(cmd, node_id, parent))
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
        .map(|cmd| {
            let pos = match cmd {
                PathCommand::Move { position, .. } => position,
                PathCommand::Line { position, .. } => position,
                PathCommand::Cubic { position, .. } => position,
            };
            Point::from_xy(pos.x as f32, pos.y as f32)
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

fn positional_transform(
    position: &Position,
    scale_x: f64,
    scale_y: f64,
    rotation: f64,
    parent: Transform,
) -> Transform {
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
        }
    }
    pb.finish()
}

const DEFAULT_FONT_SIZE: f32 = 16.0;

/// Concatenates span texts separated by U+200C (ZERO WIDTH NON-JOINER).
/// The ZWNJ tells HarfBuzz not to form ligatures across span boundaries.
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
fn collect_spans_tagged<'a>(
    child: &'a TextChild,
    target_id: u64,
    in_target: bool,
    out: &mut Vec<(bool, &'a TextSpan)>,
) {
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

/// Build the normalized cache key for a slice of spans.
/// Excludes node IDs and colors so identical text from different node IDs shares one entry.
fn make_line_key(spans: &[&TextSpan]) -> glyph_cache::LineKey {
    spans
        .iter()
        .map(|s| glyph_cache::SpanKey {
            text: s.text.clone(),
            font_family: s.font_family.clone(),
            font_size_bits: (s.font_size as f32).to_bits(),
            italic: s.italic,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
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

/// Collects glyph outline verbs into a `VectorPath`, positioned at (x, y) in screen space.
struct GlyphPen {
    x: f32,
    y: f32,
    verbs: Vec<PathVerb>,
}

impl GlyphPen {
    fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            verbs: Vec::new(),
        }
    }
}

impl OutlinePen for GlyphPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.verbs.push(PathVerb::MoveTo(self.x + x, self.y - y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.verbs.push(PathVerb::LineTo(self.x + x, self.y - y));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.verbs.push(PathVerb::QuadTo(
            self.x + cx0,
            self.y - cy0,
            self.x + x,
            self.y - y,
        ));
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.verbs.push(PathVerb::CubicTo(
            self.x + cx0,
            self.y - cy0,
            self.x + cx1,
            self.y - cy1,
            self.x + x,
            self.y - y,
        ));
    }
    fn close(&mut self) {
        self.verbs.push(PathVerb::Close);
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
        let stroke = Stroke {
            width: style.stroke_width as f32,
            ..Default::default()
        };
        pixmap.stroke_path(path, &paint, &stroke, transform, None);
    }
}
