use krilla::document::Document;
use krilla::geom::{Path, PathBuilder, Rect as KRect, Size as KSize, Transform as KTransform};
use krilla::image::Image;
use krilla::paint::{Fill, FillRule, Stroke};
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla_svg::{SurfaceExt, SvgSettings};
use renderer_core::glyph_cache::PathVerb;
use renderer_core::highlight;
use renderer_core::image_cache::{self, CachedImageKind, RawPixmap};
use renderer_core::path_utils::build_cropped_path_verbs;
use renderer_core::resources::Resources;
use renderer_core::text_layout::{build_span_text, collect_spans, get_or_build_line};
use renderer_core::transform::{node_z_level, positional_transform, AffineTransform};
use renderer_core::{Color, ImageLayer, Node, NodeKind, Scene, Style, TextChild, TextSpan};
use std::collections::HashMap;
use std::sync::Arc;

// ── Public entry point ────────────────────────────────────────────────────────

/// Render a slice of scenes into a single multi-page PDF.
/// Each scene becomes one page.  Image data is deduplicated across pages.
pub fn render_to_pdf(frames: &[&Scene]) -> anyhow::Result<Vec<u8>> {
    let mut doc = Document::new();
    let mut renderer = PdfRenderer::new();
    for scene in frames {
        renderer.render_scene(&mut doc, scene)?;
    }
    doc.finish()
        .map_err(|e| anyhow::anyhow!("PDF serialization error: {e:?}"))
}

// ── Internal renderer ─────────────────────────────────────────────────────────

struct PdfRenderer {
    /// Raster images indexed by image-cache path (or "path\0layer" for SVG layers).
    /// Cloning an `Image` is cheap — it holds an `Arc` internally, and krilla embeds
    /// identical images only once in the PDF.
    image_store: HashMap<String, Image>,
}

impl PdfRenderer {
    fn new() -> Self {
        Self {
            image_store: HashMap::new(),
        }
    }

    fn render_scene(&mut self, doc: &mut Document, scene: &Scene) -> anyhow::Result<()> {
        let settings = PageSettings::from_wh(scene.width as f32, scene.height as f32)
            .ok_or_else(|| anyhow::anyhow!("invalid scene dimensions"))?;
        let mut page = doc.start_page_with(settings);
        let mut surface = page.surface();

        // Background fill.
        if let Some(bg) = KRect::from_xywh(0.0, 0.0, scene.width as f32, scene.height as f32) {
            let mut pb = PathBuilder::new();
            pb.push_rect(bg);
            if let Some(path) = pb.finish() {
                surface.set_fill(Some(color_fill(&scene.fill_color, 1.0)));
                surface.set_stroke(None);
                surface.draw_path(&path);
            }
        }

        // Z-sorted children.
        let mut order: Vec<usize> = (0..scene.children.len()).collect();
        order.sort_by(|&a, &b| {
            node_z_level(&scene.children[a])
                .partial_cmp(&node_z_level(&scene.children[b]))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for i in order {
            self.render_node(&mut surface, &scene.children[i], AffineTransform::identity(), 1.0);
        }

        surface.finish();
        page.finish();
        Ok(())
    }

    fn render_node(
        &mut self,
        surface: &mut krilla::surface::Surface,
        node: &Node,
        parent_transform: AffineTransform,
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
                let pivot_x_abs = (*pivot_x * size.width) as f32;
                let pivot_y_abs = (*pivot_y * size.height) as f32;
                let transform = positional_transform(
                    position,
                    *scale_x,
                    *scale_y,
                    *rotation,
                    pivot_x_abs,
                    pivot_y_abs,
                    parent_transform,
                );
                let effective_alpha = parent_alpha * *alpha as f32;
                let needs_clip = *clip_x > 0.0 || *clip_y > 0.0 || *clip_w < 1.0 || *clip_h < 1.0;

                surface.push_transform(&to_krilla_transform(transform));
                let mut extra_pops = 0u32;

                if needs_clip {
                    let lw = size.width as f32;
                    let lh = size.height as f32;
                    if let Some(clip_rect) = KRect::from_xywh(
                        *clip_x as f32 * lw,
                        *clip_y as f32 * lh,
                        *clip_w as f32 * lw,
                        *clip_h as f32 * lh,
                    ) {
                        let mut pb = PathBuilder::new();
                        pb.push_rect(clip_rect);
                        if let Some(clip_path) = pb.finish() {
                            surface.push_clip_path(&clip_path, &FillRule::NonZero);
                            extra_pops += 1;
                        }
                    }
                }

                // Z-sort children.
                let mut order: Vec<usize> = (0..children.len()).collect();
                order.sort_by(|&a, &b| {
                    node_z_level(&children[a])
                        .partial_cmp(&node_z_level(&children[b]))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for i in order {
                    self.render_node(surface, &children[i], AffineTransform::identity(), effective_alpha);
                }

                for _ in 0..extra_pops {
                    surface.pop();
                }
                surface.pop(); // transform
            }

            NodeKind::Rect {
                position,
                size,
                style,
                z_level: _,
            } => {
                let transform = positional_transform(
                    position,
                    1.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    parent_transform,
                );
                surface.push_transform(&to_krilla_transform(transform));
                if let Some(rect) =
                    KRect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32)
                {
                    let mut pb = PathBuilder::new();
                    pb.push_rect(rect);
                    if let Some(path) = pb.finish() {
                        fill_and_stroke(surface, &path, style, parent_alpha);
                    }
                }
                surface.pop();
            }

            NodeKind::Ellipse {
                position,
                size,
                style,
                z_level: _,
            } => {
                let transform = positional_transform(
                    position,
                    1.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    parent_transform,
                );
                surface.push_transform(&to_krilla_transform(transform));
                if let Some(path) = build_ellipse_path(size.width as f32, size.height as f32) {
                    fill_and_stroke(surface, &path, style, parent_alpha);
                }
                surface.pop();
            }

            NodeKind::Path {
                style,
                children,
                z_level: _,
                crop_start,
                crop_end,
            } => {
                let verbs = build_cropped_path_verbs(children, *crop_start, *crop_end);
                if let Some(path) = verbs_to_krilla_path(&verbs) {
                    surface.push_transform(&to_krilla_transform(parent_transform));
                    fill_and_stroke(surface, &path, style, parent_alpha);
                    surface.pop();
                }
            }

            NodeKind::Text {
                position,
                text_style: _,
                sh_language,
                sh_theme,
                lines,
                z_level: _,
            } => {
                let transform = positional_transform(
                    position,
                    1.0,
                    1.0,
                    0.0,
                    0.0,
                    0.0,
                    parent_transform,
                );
                surface.push_transform(&to_krilla_transform(transform));
                render_text_lines(surface, lines, parent_alpha, sh_language.as_ref().map(|s| s.as_str()), sh_theme.as_ref().map(|s| s.as_str()));
                surface.pop();
            }

            NodeKind::Image {
                position,
                size,
                alpha,
                z_level: _,
                path,
                keep_aspect,
                layers,
                hidden_layers,
                ..
            } => {
                let effective_alpha = parent_alpha * *alpha as f32;
                let base_transform =
                    positional_transform(position, 1.0, 1.0, 0.0, 0.0, 0.0, parent_transform);
                self.render_image(
                    surface,
                    path,
                    size.width as f32,
                    size.height as f32,
                    *keep_aspect,
                    layers,
                    hidden_layers,
                    base_transform,
                    effective_alpha,
                );
            }
        }
    }

    // ── Image rendering ───────────────────────────────────────────────────────

    fn render_image(
        &mut self,
        surface: &mut krilla::surface::Surface,
        path: &str,
        dest_w: f32,
        dest_h: f32,
        keep_aspect: bool,
        layers: &[ImageLayer],
        hidden_layers: &[Arc<String>],
        base_transform: AffineTransform,
        effective_alpha: f32,
    ) {
        let Some(cached) = image_cache::load_image(path) else {
            return;
        };
        if dest_w <= 0.0 || dest_h <= 0.0 || cached.width <= 0.0 || cached.height <= 0.0 {
            return;
        }

        let (sx, sy, offset_x, offset_y) = if keep_aspect {
            let s = (dest_w / cached.width).min(dest_h / cached.height);
            let actual_w = cached.width * s;
            let actual_h = cached.height * s;
            (
                s,
                s,
                (dest_w - actual_w) / 2.0,
                (dest_h - actual_h) / 2.0,
            )
        } else {
            (
                dest_w / cached.width,
                dest_h / cached.height,
                0.0,
                0.0,
            )
        };

        match &cached.kind {
            CachedImageKind::Svg { raw_data, .. } => {
                if layers.is_empty() && hidden_layers.is_empty() {
                    self.draw_svg_bytes(
                        surface,
                        raw_data,
                        sx,
                        sy,
                        offset_x,
                        offset_y,
                        base_transform,
                        effective_alpha,
                        cached.width,
                        cached.height,
                    );
                } else {
                    let all_labels = cached.image_layers.as_ref();
                    if all_labels.is_none_or(|l| l.is_empty()) {
                        self.draw_svg_bytes(
                            surface,
                            raw_data,
                            sx,
                            sy,
                            offset_x,
                            offset_y,
                            base_transform,
                            effective_alpha,
                            cached.width,
                            cached.height,
                        );
                    } else {
                        for label in all_labels.unwrap().iter() {
                            if hidden_layers.iter().any(|h| **h == *label) {
                                continue;
                            }
                            let override_ =
                                layers.iter().find(|l| l.layer_name.as_str() == label.as_str());
                            let layer_alpha = override_
                                .map(|ov| effective_alpha * ov.alpha as f32)
                                .unwrap_or(effective_alpha);
                            if layer_alpha <= 0.0 {
                                continue;
                            }
                            let (lx, ly) = override_
                                .map(|ov| (ov.position.x as f32, ov.position.y as f32))
                                .unwrap_or((0.0, 0.0));
                            let Some(layer_cached) =
                                image_cache::load_svg_layer(path, label)
                            else {
                                continue;
                            };
                            let layer_raw = match &layer_cached.kind {
                                CachedImageKind::Svg { raw_data, .. } => raw_data,
                                _ => continue,
                            };
                            // Per-layer offset added on top of the main offset.
                            let layer_transform = AffineTransform::from_translate(lx, ly)
                                .concat(base_transform);
                            self.draw_svg_bytes(
                                surface,
                                layer_raw,
                                sx,
                                sy,
                                offset_x,
                                offset_y,
                                layer_transform,
                                layer_alpha,
                                cached.width,
                                cached.height,
                            );
                        }
                    }
                }
            }

            CachedImageKind::Raster { pixmap } => {
                self.draw_raster(
                    surface, path, pixmap, sx, sy, offset_x, offset_y, base_transform,
                    effective_alpha,
                );
            }

            CachedImageKind::Ora { layers: ora_layers } => {
                let all_labels = &cached.image_layers;
                if layers.is_empty() && hidden_layers.is_empty() {
                    for layer_data in ora_layers.iter() {
                        let layer_key = format!("{}\0ora\0{}", path, layer_data.name);
                        let layer_ox = offset_x + layer_data.x as f32 * sx;
                        let layer_oy = offset_y + layer_data.y as f32 * sy;
                        self.draw_raster_pixmap(
                            surface,
                            &layer_key,
                            &layer_data.pixmap,
                            sx,
                            sy,
                            layer_ox,
                            layer_oy,
                            base_transform,
                            effective_alpha,
                        );
                    }
                } else if let Some(all_labels) = all_labels {
                    for label in all_labels.iter() {
                        if hidden_layers.iter().any(|h| **h == *label) {
                            continue;
                        }
                        let Some(layer_data) =
                            ora_layers.iter().find(|l| l.name == label.as_str())
                        else {
                            continue;
                        };
                        let override_ =
                            layers.iter().find(|l| l.layer_name.as_str() == label.as_str());
                        let layer_alpha = override_
                            .map(|ov| effective_alpha * ov.alpha as f32)
                            .unwrap_or(effective_alpha);
                        if layer_alpha <= 0.0 {
                            continue;
                        }
                        let (lx, ly) = override_
                            .map(|ov| (ov.position.x as f32, ov.position.y as f32))
                            .unwrap_or((0.0, 0.0));
                        let layer_key = format!("{}\0ora\0{}", path, layer_data.name);
                        let layer_ox = offset_x + layer_data.x as f32 * sx + lx;
                        let layer_oy = offset_y + layer_data.y as f32 * sy + ly;
                        self.draw_raster_pixmap(
                            surface,
                            &layer_key,
                            &layer_data.pixmap,
                            sx,
                            sy,
                            layer_ox,
                            layer_oy,
                            base_transform,
                            layer_alpha,
                        );
                    }
                }
            }
        }
    }

    fn draw_svg_bytes(
        &mut self,
        surface: &mut krilla::surface::Surface,
        raw_data: &[u8],
        sx: f32,
        sy: f32,
        offset_x: f32,
        offset_y: f32,
        base_transform: AffineTransform,
        effective_alpha: f32,
        src_w: f32,
        src_h: f32,
    ) {
        // Re-parse raw SVG bytes with the shared font database.
        let tree = match usvg::Tree::from_data(raw_data, &usvg::Options {
            fontdb: Resources::get().fontdb(),
            ..Default::default()
        }) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("SVG re-parse for PDF failed: {e}");
                return;
            }
        };

        let draw_w = src_w * sx;
        let draw_h = src_h * sy;
        let Some(size) = KSize::from_wh(draw_w, draw_h) else {
            return;
        };

        let final_transform =
            AffineTransform::from_translate(offset_x, offset_y).concat(base_transform);
        surface.push_transform(&to_krilla_transform(final_transform));

        if (effective_alpha - 1.0).abs() > 1e-4 {
            surface.push_opacity(
                NormalizedF32::new(effective_alpha.clamp(0.0, 1.0))
                    .unwrap_or(NormalizedF32::ONE),
            );
        }

        surface.draw_svg(&tree, size, SvgSettings::default());

        if (effective_alpha - 1.0).abs() > 1e-4 {
            surface.pop(); // opacity
        }
        surface.pop(); // transform
    }

    fn draw_raster(
        &mut self,
        surface: &mut krilla::surface::Surface,
        path: &str,
        pixmap: &RawPixmap,
        sx: f32,
        sy: f32,
        offset_x: f32,
        offset_y: f32,
        base_transform: AffineTransform,
        effective_alpha: f32,
    ) {
        self.draw_raster_pixmap(
            surface,
            path,
            pixmap,
            sx,
            sy,
            offset_x,
            offset_y,
            base_transform,
            effective_alpha,
        );
    }

    fn draw_raster_pixmap(
        &mut self,
        surface: &mut krilla::surface::Surface,
        cache_key: &str,
        pixmap: &RawPixmap,
        sx: f32,
        sy: f32,
        offset_x: f32,
        offset_y: f32,
        base_transform: AffineTransform,
        effective_alpha: f32,
    ) {
        // Get or create the krilla Image, deduplicating by cache key.
        let image = self
            .image_store
            .entry(cache_key.to_string())
            .or_insert_with(|| {
                let straight = unpremultiply(&pixmap.data);
                Image::from_rgba8(straight, pixmap.width, pixmap.height)
            })
            .clone();

        let draw_w = pixmap.width as f32 * sx;
        let draw_h = pixmap.height as f32 * sy;
        let Some(size) = KSize::from_wh(draw_w, draw_h) else {
            return;
        };

        let final_transform =
            AffineTransform::from_translate(offset_x, offset_y).concat(base_transform);
        surface.push_transform(&to_krilla_transform(final_transform));

        if (effective_alpha - 1.0).abs() > 1e-4 {
            surface.push_opacity(
                NormalizedF32::new(effective_alpha.clamp(0.0, 1.0))
                    .unwrap_or(NormalizedF32::ONE),
            );
        }

        surface.draw_image(image, size);

        if (effective_alpha - 1.0).abs() > 1e-4 {
            surface.pop(); // opacity
        }
        surface.pop(); // transform
    }
}

// ── Text rendering (mirrors renderer-skia logic) ──────────────────────────────

fn render_text_lines(
    surface: &mut krilla::surface::Surface,
    lines: &[TextChild],
    parent_alpha: f32,
    sh_language: Option<&str>,
    sh_theme: Option<&str>,
) {
    // Build syntax-highlight context if requested.
    let sh_ctx: Option<(Vec<Vec<usize>>, highlight::SyntaxColors)> =
        sh_language.map(|lang| {
            let theme = sh_theme.unwrap_or("InspiredGitHub");
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

            // Resolve fill color (with optional SH override).
            let fill_color: Option<Color> =
                if let Some((ref span_starts, ref sh_colors)) = sh_ctx {
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
                            .or_else(|| span.text_style.fill_color.value().clone())
                    } else {
                        span.text_style.fill_color.value().clone()
                    }
                } else {
                    span.text_style.fill_color.value().clone()
                };

            // Build glyph path shifted by y_cursor.
            let verbs: Vec<PathVerb> = glyph
                .path
                .verbs
                .iter()
                .map(|v| shift_verb_y(v, y_cursor))
                .collect();
            let Some(path) = verbs_to_krilla_path(&verbs) else {
                continue;
            };

            if let Some(ref fc) = fill_color {
                surface.set_fill(Some(color_fill(fc, alpha)));
                surface.set_stroke(None);
                surface.draw_path(&path);
            }
            if let Some(sc) = span.text_style.stroke_color.value() {
                surface.set_fill(None);
                surface.set_stroke(Some(color_stroke(
                    sc,
                    *span.text_style.stroke_width.value() as f32,
                    alpha,
                )));
                surface.draw_path(&path);
            }
        }

        y_cursor += cached.height;
    }
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

fn to_krilla_transform(t: AffineTransform) -> KTransform {
    // AffineTransform { a=sx, b=ky, c=kx, d=sy, e=tx, f=ty }
    KTransform::from_row(t.a, t.b, t.c, t.d, t.e, t.f)
}

fn verbs_to_krilla_path(verbs: &[PathVerb]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for v in verbs {
        match v {
            PathVerb::MoveTo(x, y) => pb.move_to(*x, *y),
            PathVerb::LineTo(x, y) => pb.line_to(*x, *y),
            PathVerb::QuadTo(cx, cy, x, y) => pb.quad_to(*cx, *cy, *x, *y),
            PathVerb::CubicTo(c0x, c0y, c1x, c1y, x, y) => {
                pb.cubic_to(*c0x, *c0y, *c1x, *c1y, *x, *y)
            }
            PathVerb::Close => pb.close(),
        }
    }
    pb.finish()
}

/// Shift the y-coordinate of a glyph path verb by `dy` (for multi-line text layout).
fn shift_verb_y(v: &PathVerb, dy: f32) -> PathVerb {
    match *v {
        PathVerb::MoveTo(x, y) => PathVerb::MoveTo(x, y + dy),
        PathVerb::LineTo(x, y) => PathVerb::LineTo(x, y + dy),
        PathVerb::QuadTo(cx, cy, x, y) => PathVerb::QuadTo(cx, cy + dy, x, y + dy),
        PathVerb::CubicTo(c0x, c0y, c1x, c1y, x, y) => {
            PathVerb::CubicTo(c0x, c0y + dy, c1x, c1y + dy, x, y + dy)
        }
        PathVerb::Close => PathVerb::Close,
    }
}

/// Build an ellipse path as four cubic Bézier arcs centered in a (w × h) box.
fn build_ellipse_path(w: f32, h: f32) -> Option<Path> {
    let cx = w * 0.5;
    let cy = h * 0.5;
    let rx = w * 0.5;
    let ry = h * 0.5;
    // Bézier approximation constant ≈ 4*(√2-1)/3
    const K: f32 = 0.5522847498;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + rx, cy);
    pb.cubic_to(cx + rx, cy - K * ry, cx + K * rx, cy - ry, cx, cy - ry);
    pb.cubic_to(cx - K * rx, cy - ry, cx - rx, cy - K * ry, cx - rx, cy);
    pb.cubic_to(cx - rx, cy + K * ry, cx - K * rx, cy + ry, cx, cy + ry);
    pb.cubic_to(cx + K * rx, cy + ry, cx + rx, cy + K * ry, cx + rx, cy);
    pb.close();
    pb.finish()
}

// ── Paint helpers ─────────────────────────────────────────────────────────────

fn color_fill(c: &Color, alpha: f32) -> Fill {
    let (r, g, b, a) = c.to_rgba_f32();
    let opacity = (a * alpha).clamp(0.0, 1.0);
    Fill {
        paint: krilla::color::rgb::Color::new(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        )
        .into(),
        opacity: NormalizedF32::new(opacity).unwrap_or(NormalizedF32::ONE),
        rule: FillRule::NonZero,
    }
}

fn color_stroke(c: &Color, width: f32, alpha: f32) -> Stroke {
    let (r, g, b, a) = c.to_rgba_f32();
    let opacity = (a * alpha).clamp(0.0, 1.0);
    Stroke {
        paint: krilla::color::rgb::Color::new(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        )
        .into(),
        opacity: NormalizedF32::new(opacity).unwrap_or(NormalizedF32::ONE),
        width,
        ..Default::default()
    }
}

fn fill_and_stroke(
    surface: &mut krilla::surface::Surface,
    path: &Path,
    style: &Style,
    alpha: f32,
) {
    let effective_alpha = alpha * style.alpha as f32;
    if let Some(ref fc) = style.fill_color {
        surface.set_fill(Some(color_fill(fc, effective_alpha)));
        surface.set_stroke(None);
        surface.draw_path(path);
    }
    if let Some(ref sc) = style.stroke_color {
        surface.set_fill(None);
        surface.set_stroke(Some(color_stroke(sc, style.stroke_width as f32, effective_alpha)));
        surface.draw_path(path);
    }
}

// ── Image helpers ─────────────────────────────────────────────────────────────

/// Convert premultiplied RGBA8888 to straight (un-premultiplied) RGBA8888.
/// `Image::from_rgba8` expects straight alpha.
fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        let [r, g, b, a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let inv = 255.0 / a as f32;
            out.push((r as f32 * inv).min(255.0) as u8);
            out.push((g as f32 * inv).min(255.0) as u8);
            out.push((b as f32 * inv).min(255.0) as u8);
            out.push(a);
        }
    }
    out
}
