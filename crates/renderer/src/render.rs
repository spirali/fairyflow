use crate::glyph_cache::{self, CachedLine, PathVerb, VectorPath};
use crate::highlight;
use crate::image_cache;
use crate::resources::Resources;
use crate::scene::{Node, NodeKind, PathCommand, Position, Scene, Size, Style, TextChild, TextSpan};
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
    FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Point, Rect, Stroke, Transform,
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
        &mut self,
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
                z_level,
                children,
            } => {
                let pivot_x_abs = (pivot_x * size.width) as f32;
                let pivot_y_abs = (pivot_y * size.height) as f32;
                let transform =
                    positional_transform(position, *scale_x, *scale_y, *rotation, pivot_x_abs, pivot_y_abs, parent_transform);
                let alpha = parent_alpha * *alpha as f32;
                // Clone to avoid holding a borrow on node.kind while calling self methods.
                let children = children.clone();

                let needs_clip = *clip_x > 0.0 || *clip_y > 0.0 || *clip_w < 1.0 || *clip_h < 1.0;
                if !needs_clip {
                    self.render_children(&children, pixmap, transform, alpha);
                } else {
                    // Render children into an offscreen pixmap of the same dimensions.
                    let w = pixmap.width();
                    let h = pixmap.height();
                    let mut offscreen = Pixmap::new(w, h).expect("offscreen pixmap");
                    self.render_children(&children, &mut offscreen, transform, alpha);

                    // Build the clip rectangle in group-local space and transform it to
                    // screen space to create a mask.
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
            NodeKind::Rect {
                position,
                size,
                style,
                z_level,
            } => {
                let transform = positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent_transform);
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
                z_level,
            } => {
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
            NodeKind::Path {
                style,
                children,
                z_level,
                crop_start,
                crop_end,
            } => {
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
                z_level,
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
        &mut self,
        lines: &[TextChild],
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
        sh: Option<(&str, &str)>,
    ) {
        // Pre-compute SH colors for the entire text block in one pass so that
        // the syntect parser state carries correctly across TextChild boundaries.
        //
        // span_starts[line_idx][span_idx] = byte offset of that span's text in
        // the full concatenated program text (no ZWNJ separators).
        let sh_ctx: Option<(Vec<Vec<usize>>, highlight::SyntaxColors)> = sh.map(|(lang, theme)| {
            let mut full_text = String::new();
            // Insert '\n' between consecutive TextChild entries so syntect sees
            // them as separate lines and recognises tokens that span child
            // boundaries correctly (e.g. "print" split across "pr"/"in"/"t(…)").
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

            let cached = self.get_or_build_line(&spans);

            // For SH: build the ZWNJ-text span ranges so we can map
            // glyph.cluster (offset in ZWNJ text) → offset in original text.
            let zwnj_ranges: Vec<(std::ops::Range<usize>, usize)> = if sh_ctx.is_some() {
                let (_, r) = build_span_text(&spans);
                r
            } else {
                Vec::new()
            };

            for glyph in &cached.glyphs {
                let span = spans[glyph.span_idx];
                let alpha = parent_alpha * *span.text_style.alpha.value() as f32;

                // Resolve fill color: SH overrides when the span's fill_color is Inherited.
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
                            .map(|c| c.to_skia_color())
                            .or_else(|| {
                                span.text_style.fill_color.value().as_ref().map(|c| c.to_skia_color())
                            })
                    } else {
                        span.text_style.fill_color.value().as_ref().map(|c| c.to_skia_color())
                    }
                } else {
                    span.text_style.fill_color.value().as_ref().map(|c| c.to_skia_color())
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
                    let mut color = sc.to_skia_color();
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
                StyleProperty::FontSize(*span.text_style.font_size.value() as f32),
                range.clone(),
            );
            builder.push(
                StyleProperty::FontStack(FontStack::Source((span.text_style.font_family.value().as_str()).into())),
                range.clone(),
            );
            if *span.text_style.italic.value() {
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

                // Iterate clusters to obtain per-cluster byte offsets in the
                // ZWNJ text, then iterate the glyphs within each cluster.
                for cluster in run.visual_clusters() {
                    let cluster_byte = cluster.text_range().start as u32;
                    for glyph in cluster.glyphs() {
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
                            cluster: cluster_byte,
                        });
                    }
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

/// Renders `scene` fitted (letterboxed) into `width × height` and writes the
/// result into `buffer` as `0x00RRGGBB` u32 values (softbuffer-compatible format).
/// `buffer` must have exactly `width * height` elements.
pub fn render_scene_to_buffer(scene: &Scene, width: u32, height: u32, buffer: &mut [u32]) {
    let pixmap = render_scene_fitted(scene, width, height);
    for (dst, src) in buffer.iter_mut().zip(pixmap.pixels()) {
        *dst = ((src.red() as u32) << 16)
            | ((src.green() as u32) << 8)
            | (src.blue() as u32);
    }
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
            pivot_x,
            pivot_y,
            children,
            z_level,
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

/// Split a cubic Bézier (absolute coords) at parameter t using de Casteljau.
/// Returns (left, right) where each is (start, c1, c2, end).
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

/// Return the subsegment of a cubic from t1 to t2 (both in [0, 1]).
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

/// Build a path from `commands`, cropped to the arc-length range [crop_start, crop_end]
/// where 0.0 = path start, 1.0 = path end.
fn build_cropped_path(commands: &[PathCommand], crop_start: f64, crop_end: f64) -> Option<tiny_skia::Path> {
    if crop_start <= 0.0 && crop_end >= 1.0 {
        return build_path(commands);
    }

    // Segment kinds with absolute coordinates.
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

    // First pass: extract drawable segments with their arc lengths.
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

    // Second pass: build the cropped path.
    let mut pb = PathBuilder::new();
    let mut accumulated = 0.0f32;
    // Track the last emitted endpoint to detect continuity.
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
            font_family: s.text_style.font_family.value().clone(),
            font_size_bits: (*s.text_style.font_size.value() as f32).to_bits(),
            italic: *s.text_style.italic.value(),
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

/// Dispatch: load any supported image format (SVG, PNG, JPEG, ORA) from disk.
fn load_image(path: &str) -> Option<Arc<image_cache::CachedImage>> {
    if let Some(cached) = image_cache::cache_get(path) {
        return Some(cached);
    }
    let data = std::fs::read(path).ok()?;
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "svg" | "svgz" => load_svg_from_data(path, data),
        "png" | "jpg" | "jpeg" => load_raster_from_data(path, data),
        "ora" => load_ora_from_data(path, data),
        _ => None,
    }
}

fn load_svg_from_data(path: &str, data: Vec<u8>) -> Option<Arc<image_cache::CachedImage>> {
    let opt = usvg::Options {
        fontdb: Resources::get().fontdb(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(&data, &opt).ok()?;
    let svg_size = tree.size();
    let image_layers = Arc::new(svg_layer_labels(&data));
    let cached = image_cache::CachedImage {
        kind: image_cache::CachedImageKind::Svg { tree, raw_data: data },
        width: svg_size.width(),
        height: svg_size.height(),
        image_layers: Some(image_layers),
    };
    Some(image_cache::cache_store(path.to_string(), cached))
}

fn load_raster_from_data(path: &str, data: Vec<u8>) -> Option<Arc<image_cache::CachedImage>> {
    let img = image::load_from_memory(&data).ok()?;
    let width = img.width() as f32;
    let height = img.height() as f32;
    let pixmap = image_to_pixmap(img)?;
    let cached = image_cache::CachedImage {
        kind: image_cache::CachedImageKind::Raster { pixmap },
        width,
        height,
        image_layers: None,
    };
    Some(image_cache::cache_store(path.to_string(), cached))
}

fn load_ora_from_data(path: &str, data: Vec<u8>) -> Option<Arc<image_cache::CachedImage>> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;

    let stack_xml = {
        let mut file = archive.by_name("stack.xml").ok()?;
        let mut buf = String::new();
        file.read_to_string(&mut buf).ok()?;
        buf
    };
    let root = xmltree::Element::parse(std::io::Cursor::new(stack_xml.as_bytes())).ok()?;
    let width: f32 = root.attributes.get("w")?.parse().ok()?;
    let height: f32 = root.attributes.get("h")?.parse().ok()?;

    let stack_elem = root.children.iter().find_map(|child| {
        if let xmltree::XMLNode::Element(elem) = child {
            if elem.name == "stack" { Some(elem) } else { None }
        } else {
            None
        }
    })?;

    // Collect layer metadata in top-to-bottom stack.xml order.
    let mut layers_info: Vec<(String, String, i32, i32)> = Vec::new();
    for child in &stack_elem.children {
        let xmltree::XMLNode::Element(elem) = child else { continue };
        if elem.name != "layer" {
            continue;
        }
        let name = elem.attributes.get("name").cloned().unwrap_or_default();
        let src = elem.attributes.get("src").cloned().unwrap_or_default();
        let x: i32 = elem.attributes.get("x").and_then(|v| v.parse().ok()).unwrap_or(0);
        let y: i32 = elem.attributes.get("y").and_then(|v| v.parse().ok()).unwrap_or(0);
        layers_info.push((name, src, x, y));
    }

    // Store layer names bottom-to-top (consistent with SVG document order).
    let image_layers: Vec<String> = layers_info.iter().rev().map(|(n, ..)| n.clone()).collect();

    // Decode layer PNGs and store in bottom-to-top render order.
    let mut ora_layers: Vec<image_cache::OraLayer> = Vec::new();
    for (name, src, x, y) in layers_info.iter().rev() {
        let png_data = {
            let Ok(mut file) = archive.by_name(src) else { continue };
            let mut buf = Vec::new();
            if file.read_to_end(&mut buf).is_err() {
                continue;
            }
            buf
        };
        let Ok(img) = image::load_from_memory(&png_data) else { continue };
        let Some(pixmap) = image_to_pixmap(img) else { continue };
        ora_layers.push(image_cache::OraLayer { name: name.clone(), pixmap, x: *x, y: *y });
    }

    let cached = image_cache::CachedImage {
        kind: image_cache::CachedImageKind::Ora { layers: ora_layers },
        width,
        height,
        image_layers: Some(Arc::new(image_layers)),
    };
    Some(image_cache::cache_store(path.to_string(), cached))
}

/// Convert a decoded `DynamicImage` to a `Pixmap` with premultiplied alpha.
fn image_to_pixmap(img: image::DynamicImage) -> Option<Pixmap> {
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let size = tiny_skia::IntSize::from_wh(width, height)?;
    let data: Vec<u8> = rgba
        .pixels()
        .flat_map(|p| {
            let [r, g, b, a] = p.0;
            let pm = |c: u8| (c as u32 * a as u32 / 255) as u8;
            [pm(r), pm(g), pm(b), a]
        })
        .collect();
    Pixmap::from_vec(data, size)
}

/// The Inkscape namespace URI used for layer metadata attributes.
const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";

/// Return the `inkscape:label` values of all direct-child `<g>` layer elements
/// in the SVG, in document order.  Elements without a label are skipped.
fn svg_layer_labels(data: &[u8]) -> Vec<String> {
    let Ok(root) = xmltree::Element::parse(std::io::Cursor::new(data)) else {
        return Vec::new();
    };
    root.children
        .iter()
        .filter_map(|child| {
            let xmltree::XMLNode::Element(elem) = child else { return None };
            if elem.name != "g" {
                return None;
            }
            inkscape_label(elem).map(str::to_owned)
        })
        .collect()
}

/// Return a modified copy of the SVG bytes where every direct-child `<g>`
/// element that carries an `inkscape:label` attribute whose value does **not**
/// equal `target_label` is hidden by setting `display:none` in its `style`.
///
/// Namespace-awareness: xmltree (backed by xml-rs) stores every attribute
/// under its **local name** as the HashMap key, regardless of what prefix the
/// document uses to bind a namespace URI.  An attribute written as
/// `inkscape:label`, `ink:label`, or `ns0:label` (all binding the Inkscape
/// URI) will all appear in `element.attributes` under the key `"label"`.
/// That means prefix variations are handled automatically.
///
/// To avoid accidentally matching an unrelated `label` attribute in a
/// different namespace, we additionally require that the Inkscape namespace
/// URI is declared somewhere in scope on the element.  xml-rs propagates all
/// in-scope namespace bindings (including those declared on ancestors) into
/// every `StartElement` event's namespace map, so a binding declared on the
/// root `<svg>` will be present in `elem.namespaces` for every descendant.
fn svg_show_only_layer(data: &[u8], target_label: &str) -> Vec<u8> {
    let Ok(mut root) = xmltree::Element::parse(std::io::Cursor::new(data)) else {
        return data.to_vec();
    };

    for child in &mut root.children {
        // Rust 2024: iterating `&mut Vec<XMLNode>` yields `&mut XMLNode`;
        // the binding pattern implicitly reborrrows without `ref mut`.
        let xmltree::XMLNode::Element(elem) = child else { continue };
        if elem.name != "g" {
            continue;
        }
        let Some(label) = inkscape_label(elem) else { continue };
        if label == target_label {
            continue;
        }
        // Hide this layer by overwriting its `style` attribute.
        let current = elem.attributes.get("style").cloned().unwrap_or_default();
        elem.attributes.insert("style".to_string(), css_display_none(&current));
    }

    let mut output = Vec::new();
    root.write(&mut output).ok();
    output
}

/// Return the value of the `inkscape:label` attribute on `elem`, or `None` if
/// the element has no such attribute or the Inkscape namespace is not in scope.
///
/// Because xmltree keys attributes by local name, the lookup is simply
/// `attributes["label"]`.
fn inkscape_label(elem: &xmltree::Element) -> Option<&str> {
    elem.attributes.get("label").map(String::as_str)
}

/// Return a CSS `style` string identical to `style` but with `display:none`
/// set.  An existing `display` property is replaced; other properties are kept.
fn css_display_none(style: &str) -> String {
    let mut parts: Vec<&str> = style
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("display"))
        .collect();
    parts.push("display:none");
    parts.join(";")
}

/// Load (or create from cache) a single-layer view of the SVG at `path`.
/// The layer is identified by its `inkscape:label` attribute value.
fn load_svg_layer(path: &str, layer_label: &str) -> Option<Arc<image_cache::CachedImage>> {
    // Use a composite cache key that won't collide with plain path keys
    // (file paths never contain the null byte).
    let cache_key = format!("{}\0{}", path, layer_label);
    if let Some(cached) = image_cache::cache_get(&cache_key) {
        return Some(cached);
    }

    // Ensure the base image is loaded and extract its raw SVG bytes.
    let base = load_image(path)?;
    let raw_data = match &base.kind {
        image_cache::CachedImageKind::Svg { raw_data, .. } => raw_data.clone(),
        _ => return None,
    };

    let modified = svg_show_only_layer(&raw_data, layer_label);

    let opt = usvg::Options {
        fontdb: Resources::get().fontdb(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(&modified, &opt).ok()?;
    let svg_size = tree.size();
    let cached = image_cache::CachedImage {
        kind: image_cache::CachedImageKind::Svg { tree, raw_data: modified },
        width: svg_size.width(),
        height: svg_size.height(),
        image_layers: None,
    };
    Some(image_cache::cache_store(cache_key, cached))
}

/// Render a raster `Pixmap` into the destination `pixmap`.
///
/// `sx`/`sy` scale the source; `offset_x`/`offset_y` position the scaled image
/// within the node box (letterbox margins or ORA layer offsets).
fn render_raster_pixmap(
    src: &Pixmap,
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
    let mut paint = PixmapPaint::default();
    paint.quality = tiny_skia::FilterQuality::Bilinear;
    if (effective_alpha - 1.0).abs() < 1e-6 {
        let transform = Transform::from_scale(sx, sy)
            .post_translate(offset_x, offset_y)
            .post_concat(node_transform);
        pixmap.draw_pixmap(0, 0, src.as_ref(), &paint, transform, None);
    } else {
        let w_u32 = dest_w.ceil() as u32;
        let h_u32 = dest_h.ceil() as u32;
        let Some(mut img_pixmap) = Pixmap::new(w_u32.max(1), h_u32.max(1)) else { return };
        let inner_transform = Transform::from_scale(sx, sy).post_translate(offset_x, offset_y);
        img_pixmap.draw_pixmap(0, 0, src.as_ref(), &paint, inner_transform, None);
        let mut composite_paint = PixmapPaint::default();
        composite_paint.opacity = effective_alpha.clamp(0.0, 1.0);
        pixmap.draw_pixmap(0, 0, img_pixmap.as_ref(), &composite_paint, node_transform, None);
    }
}

/// Render an image node (SVG, PNG, JPEG, or ORA) into `pixmap`.
///
/// For SVG with alpha = 1.0 the tree is rendered directly at full vector
/// quality; for alpha < 1.0 an intermediate pixmap is used.  PNG/JPEG are
/// drawn as scaled raster images.  ORA images are composited layer by layer.
///
/// When `layers` / `hidden_layers` is non-empty, each named layer is handled
/// individually (overrides applied or layer skipped).
fn render_image(
    path: &str,
    size: &Size,
    keep_aspect: bool,
    layers: &[crate::scene::ImageLayer],
    hidden_layers: &[Arc<String>],
    pixmap: &mut Pixmap,
    parent_transform: Transform,
    position: &Position,
    effective_alpha: f32,
) {
    let Some(cached) = load_image(path) else { return };
    let dest_w = size.width as f32;
    let dest_h = size.height as f32;
    if dest_w <= 0.0 || dest_h <= 0.0 || cached.width <= 0.0 || cached.height <= 0.0 {
        return;
    }

    // When keep_aspect is true, scale uniformly so the image fits within
    // dest_w × dest_h, then centre it inside the node box.
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
        image_cache::CachedImageKind::Svg { tree, .. } => {
            if layers.is_empty() && hidden_layers.is_empty() {
                render_svg_tree(tree, sx, sy, offset_x, offset_y, node_transform, pixmap, effective_alpha, dest_w, dest_h);
            } else {
                let all_labels = cached.image_layers.as_ref();
                if all_labels.is_none_or(|labels| labels.is_empty()) {
                    // No named layers — render the whole SVG.
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
                        let Some(layer_cached) = load_svg_layer(path, label) else { continue };
                        let layer_tree = match &layer_cached.kind {
                            image_cache::CachedImageKind::Svg { tree, .. } => tree,
                            _ => continue,
                        };
                        let layer_transform = node_transform.post_translate(lx, ly);
                        render_svg_tree(layer_tree, sx, sy, offset_x, offset_y, layer_transform, pixmap, layer_alpha, dest_w, dest_h);
                    }
                }
            }
        }
        image_cache::CachedImageKind::Raster { pixmap: src } => {
            // PNG/JPEG: no layer support.
            render_raster_pixmap(src, sx, sy, offset_x, offset_y, node_transform, pixmap, effective_alpha, dest_w, dest_h);
        }
        image_cache::CachedImageKind::Ora { layers: ora_layers } => {
            // ORA: composite layers in bottom-to-top order.
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

/// Low-level helper: render a `usvg::Tree` into `pixmap` using the given scale
/// and centering offsets.
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
        // Render directly into the main pixmap for full vector quality.
        let svg_transform = Transform::from_scale(sx, sy)
            .post_translate(offset_x, offset_y)
            .post_concat(node_transform);
        resvg::render(tree, svg_transform, &mut pixmap.as_mut());
    } else {
        // Render to an intermediate pixmap so we can apply opacity.
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

/// Return the natural (intrinsic) pixel size of an image, or `None` if the
/// file cannot be read or parsed.  Results are cached for the lifetime of the
/// current request.
pub fn measure_image(path: &str) -> Option<(f32, f32)> {
    let cached = load_image(path)?;
    Some((cached.width, cached.height))
}

/// Return all layer names for the image at `path`, in document order.
/// For SVG: `inkscape:label` layer names (bottom-to-top).
/// For ORA: layer names from `stack.xml` (bottom-to-top).
/// For JPEG/PNG: always empty.
pub fn svg_image_layers(path: &str) -> Arc<Vec<String>> {
    load_image(path).map(|c| c.image_layers.clone().unwrap_or_default()).unwrap_or_default()
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
