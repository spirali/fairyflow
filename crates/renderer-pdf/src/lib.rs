use krilla::document::Document;
use krilla::geom::{Path, PathBuilder, Rect as KRect, Size as KSize, Transform as KTransform};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, FillRule, Stroke};
use krilla_svg::{SurfaceExt, SvgSettings};
use renderer_core::glyph_cache::PathVerb;
use renderer_core::highlight;
use renderer_core::image_cache::{self, CachedImageKind, RawPixmap};
use renderer_core::path_utils::{build_cropped_path_verbs, build_rounded_rect_verbs, verbs_bounds};
use renderer_core::resources::Resources;
use renderer_core::text_layout::{build_span_text, collect_spans};
use renderer_core::transform::{
    AffineTransform, camera_transform, gradient_line_endpoints, node_z_level, positional_transform,
    transform_node_box,
};
use renderer_core::{
    Color, ImageLayer, Node, NodeKind, Paint as RcPaint, Position, Scene, Size, Style, TextChild,
    TextSpan,
};
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

/// `(sx, sy, offset_x, offset_y)` to scale + center a `Text` node's laid-out
/// lines (natural extent `nat_w × nat_h`) to fit an explicit `size(w=, h=)`
/// box `dest_w × dest_h` — same fit math as `ImagePlacement` below, kept as a
/// separate small function since `Image`'s placement also folds in
/// destination-pixmap sizing that `Text` doesn't need. Degenerate (empty)
/// text keeps scale 1, matching the untouched-default case.
fn text_fit_scale(
    dest_w: f32,
    dest_h: f32,
    nat_w: f32,
    nat_h: f32,
    keep_aspect: bool,
) -> (f32, f32, f32, f32) {
    if nat_w <= 0.0 || nat_h <= 0.0 {
        return (1.0, 1.0, 0.0, 0.0);
    }
    if keep_aspect {
        let s = (dest_w / nat_w).min(dest_h / nat_h);
        (s, s, (dest_w - nat_w * s) / 2.0, (dest_h - nat_h * s) / 2.0)
    } else {
        (dest_w / nat_w, dest_h / nat_h, 0.0, 0.0)
    }
}

// ── Image helper structs ──────────────────────────────────────────────────────

struct ImageSpec<'a> {
    path: &'a str,
    dest_w: f32,
    dest_h: f32,
    keep_aspect: bool,
    layers: &'a [ImageLayer],
    hidden_layers: &'a [Arc<String>],
}

struct ImagePlacement {
    sx: f32,
    sy: f32,
    offset_x: f32,
    offset_y: f32,
    dest_w: f32,
    dest_h: f32,
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

        // Scene-level camera (innermost, applied before children paint).
        let box_center = Position::new(scene.width * 0.5, scene.height * 0.5);
        surface.push_transform(&to_krilla_transform(camera_transform(
            &scene.camera,
            box_center,
        )));

        // Z-sorted children.
        let mut order: Vec<usize> = (0..scene.children.len()).collect();
        order.sort_by(|&a, &b| {
            node_z_level(&scene.children[a])
                .partial_cmp(&node_z_level(&scene.children[b]))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for i in order {
            self.render_node(
                &mut surface,
                &scene.children[i],
                AffineTransform::identity(),
                1.0,
            );
        }

        surface.pop(); // scene camera

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
                node_box,
                alpha,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
                clip_enabled,
                camera,
                children,
            } => {
                let transform = transform_node_box(&node_box, parent_transform);
                let effective_alpha = parent_alpha * *alpha as f32;
                let needs_clip = *clip_enabled
                    || *clip_x > 0.0
                    || *clip_y > 0.0
                    || *clip_w < 1.0
                    || *clip_h < 1.0;
                surface.push_transform(&to_krilla_transform(transform));
                let mut extra_pops = 0u32;

                if needs_clip {
                    let lw = node_box.size.width as f32;
                    let lh = node_box.size.height as f32;
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

                // Camera (innermost layer): affects only the children drawn
                // below, never the clip path above.
                let box_center =
                    Position::new(node_box.size.width * 0.5, node_box.size.height * 0.5);
                surface.push_transform(&to_krilla_transform(camera_transform(camera, box_center)));

                // Z-sort children.
                let mut order: Vec<usize> = (0..children.len()).collect();
                order.sort_by(|&a, &b| {
                    node_z_level(&children[a])
                        .partial_cmp(&node_z_level(&children[b]))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for i in order {
                    self.render_node(
                        surface,
                        &children[i],
                        AffineTransform::identity(),
                        effective_alpha,
                    );
                }

                surface.pop(); // camera
                for _ in 0..extra_pops {
                    surface.pop();
                }
                surface.pop(); // transform
            }

            NodeKind::Rect {
                node_box,
                style,
                radius,
            } => {
                let transform = transform_node_box(&node_box, parent_transform);
                surface.push_transform(&to_krilla_transform(transform));
                let w = node_box.size.width as f32;
                let h = node_box.size.height as f32;
                let path = if *radius > 0.0 {
                    verbs_to_krilla_path(&build_rounded_rect_verbs(w, h, *radius as f32))
                } else {
                    KRect::from_xywh(0.0, 0.0, w, h).and_then(|rect| {
                        let mut pb = PathBuilder::new();
                        pb.push_rect(rect);
                        pb.finish()
                    })
                };
                if let Some(path) = path {
                    fill_and_stroke(surface, &path, style, parent_alpha, (0.0, 0.0, w, h));
                }
                surface.pop();
            }

            NodeKind::Ellipse { node_box, style } => {
                let transform = transform_node_box(&node_box, parent_transform);
                surface.push_transform(&to_krilla_transform(transform));
                let (w, h) = (node_box.size.width as f32, node_box.size.height as f32);
                if let Some(path) = build_ellipse_path(w, h) {
                    fill_and_stroke(surface, &path, style, parent_alpha, (0.0, 0.0, w, h));
                }
                surface.pop();
            }

            NodeKind::Path {
                style,
                children,
                z_level: _,
                crop_start,
                crop_end,
                ..
            } => {
                let verbs = build_cropped_path_verbs(children, *crop_start, *crop_end);
                let bounds = verbs_bounds(&verbs).unwrap_or((0.0, 0.0, 0.0, 0.0));
                if let Some(path) = verbs_to_krilla_path(&verbs) {
                    fill_and_stroke(surface, &path, style, parent_alpha, bounds);
                }
            }

            NodeKind::Text {
                node_box,
                keep_aspect,
                wrap,
                text_align,
                text_style: _,
                sh_language,
                sh_theme,
                lines,
            } => {
                let laid_out =
                    renderer_core::layout_text(lines, wrap.map(|w| w as f32), *text_align);
                let (sx, sy, off_x, off_y) = text_fit_scale(
                    node_box.size.width as f32,
                    node_box.size.height as f32,
                    laid_out.width,
                    laid_out.height,
                    *keep_aspect,
                );
                let box_transform = transform_node_box(node_box, parent_transform);
                let content_transform = positional_transform(
                    Position {
                        x: off_x as f64,
                        y: off_y as f64,
                    },
                    Size {
                        width: sx as f64,
                        height: sy as f64,
                    },
                    0.0,
                    0.0,
                    0.0,
                    box_transform,
                );
                surface.push_transform(&to_krilla_transform(content_transform));
                render_text_lines(
                    surface,
                    lines,
                    &laid_out.lines,
                    parent_alpha,
                    sh_language.as_ref().map(|s| s.as_str()),
                    sh_theme.as_ref().map(|s| s.as_str()),
                    (sx, sy),
                );
                surface.pop();
            }

            NodeKind::Image {
                node_box,
                alpha,
                path,
                keep_aspect,
                layers,
                hidden_layers,
                ..
            } => {
                let effective_alpha = parent_alpha * *alpha as f32;
                let base_transform = transform_node_box(node_box, parent_transform);
                self.render_image(
                    surface,
                    &ImageSpec {
                        path,
                        dest_w: node_box.size.width as f32,
                        dest_h: node_box.size.height as f32,
                        keep_aspect: *keep_aspect,
                        layers,
                        hidden_layers,
                    },
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
        spec: &ImageSpec<'_>,
        base_transform: AffineTransform,
        effective_alpha: f32,
    ) {
        let Some(cached) = image_cache::load_image(spec.path) else {
            return;
        };
        if spec.dest_w <= 0.0 || spec.dest_h <= 0.0 || cached.width <= 0.0 || cached.height <= 0.0 {
            return;
        }

        let placement = if spec.keep_aspect {
            let s = (spec.dest_w / cached.width).min(spec.dest_h / cached.height);
            let actual_w = cached.width * s;
            let actual_h = cached.height * s;
            ImagePlacement {
                sx: s,
                sy: s,
                offset_x: (spec.dest_w - actual_w) / 2.0,
                offset_y: (spec.dest_h - actual_h) / 2.0,
                dest_w: spec.dest_w,
                dest_h: spec.dest_h,
            }
        } else {
            ImagePlacement {
                sx: spec.dest_w / cached.width,
                sy: spec.dest_h / cached.height,
                offset_x: 0.0,
                offset_y: 0.0,
                dest_w: spec.dest_w,
                dest_h: spec.dest_h,
            }
        };

        match &cached.kind {
            CachedImageKind::Svg { raw_data, .. } => {
                let svg_placement = ImagePlacement {
                    dest_w: cached.width * placement.sx,
                    dest_h: cached.height * placement.sy,
                    ..placement
                };
                if spec.layers.is_empty() && spec.hidden_layers.is_empty() {
                    self.draw_svg_bytes(
                        surface,
                        raw_data,
                        &svg_placement,
                        base_transform,
                        effective_alpha,
                    );
                } else {
                    let all_labels = cached.image_layers.as_ref();
                    if all_labels.is_none_or(|l| l.is_empty()) {
                        self.draw_svg_bytes(
                            surface,
                            raw_data,
                            &svg_placement,
                            base_transform,
                            effective_alpha,
                        );
                    } else {
                        for label in all_labels.unwrap().iter() {
                            if spec.hidden_layers.iter().any(|h| **h == *label) {
                                continue;
                            }
                            let override_ = spec
                                .layers
                                .iter()
                                .find(|l| l.layer_name.as_str() == label.as_str());
                            let layer_alpha = override_
                                .map(|ov| effective_alpha * ov.alpha as f32)
                                .unwrap_or(effective_alpha);
                            if layer_alpha <= 0.0 {
                                continue;
                            }
                            let Some(layer_cached) = image_cache::load_svg_layer(spec.path, label)
                            else {
                                continue;
                            };
                            let layer_raw = match &layer_cached.kind {
                                CachedImageKind::Svg { raw_data, .. } => raw_data,
                                _ => continue,
                            };
                            let layer_transform = image_layer_translate(override_, base_transform);
                            self.draw_svg_bytes(
                                surface,
                                layer_raw,
                                &svg_placement,
                                layer_transform,
                                layer_alpha,
                            );
                        }
                    }
                }
            }

            CachedImageKind::Raster { pixmap } => {
                let raster_placement = ImagePlacement {
                    dest_w: pixmap.width as f32 * placement.sx,
                    dest_h: pixmap.height as f32 * placement.sy,
                    ..placement
                };
                self.draw_raster(
                    surface,
                    spec.path,
                    pixmap,
                    &raster_placement,
                    base_transform,
                    effective_alpha,
                );
            }

            CachedImageKind::Ora { layers: ora_layers } => {
                let all_labels = &cached.image_layers;
                if spec.layers.is_empty() && spec.hidden_layers.is_empty() {
                    for layer_data in ora_layers.iter() {
                        let layer_key = format!("{}\0ora\0{}", spec.path, layer_data.name);
                        let layer_placement = ImagePlacement {
                            offset_x: placement.offset_x + layer_data.x as f32 * placement.sx,
                            offset_y: placement.offset_y + layer_data.y as f32 * placement.sy,
                            dest_w: layer_data.pixmap.width as f32 * placement.sx,
                            dest_h: layer_data.pixmap.height as f32 * placement.sy,
                            ..placement
                        };
                        self.draw_raster_pixmap(
                            surface,
                            &layer_key,
                            &layer_data.pixmap,
                            &layer_placement,
                            base_transform,
                            effective_alpha,
                        );
                    }
                } else if let Some(all_labels) = all_labels {
                    for label in all_labels.iter() {
                        if spec.hidden_layers.iter().any(|h| **h == *label) {
                            continue;
                        }
                        let Some(layer_data) = ora_layers.iter().find(|l| l.name == label.as_str())
                        else {
                            continue;
                        };
                        let override_ = spec
                            .layers
                            .iter()
                            .find(|l| l.layer_name.as_str() == label.as_str());
                        let layer_alpha = override_
                            .map(|ov| effective_alpha * ov.alpha as f32)
                            .unwrap_or(effective_alpha);
                        if layer_alpha <= 0.0 {
                            continue;
                        }
                        let layer_key = format!("{}\0ora\0{}", spec.path, layer_data.name);
                        let layer_placement = ImagePlacement {
                            offset_x: placement.offset_x + layer_data.x as f32 * placement.sx,
                            offset_y: placement.offset_y + layer_data.y as f32 * placement.sy,
                            dest_w: layer_data.pixmap.width as f32 * placement.sx,
                            dest_h: layer_data.pixmap.height as f32 * placement.sy,
                            ..placement
                        };
                        let layer_transform = image_layer_translate(override_, base_transform);
                        self.draw_raster_pixmap(
                            surface,
                            &layer_key,
                            &layer_data.pixmap,
                            &layer_placement,
                            layer_transform,
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
        placement: &ImagePlacement,
        base_transform: AffineTransform,
        effective_alpha: f32,
    ) {
        // Re-parse raw SVG bytes with the shared font database.
        let tree = match usvg::Tree::from_data(
            raw_data,
            &usvg::Options {
                fontdb: Resources::get().fontdb(),
                ..Default::default()
            },
        ) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("SVG re-parse for PDF failed: {e}");
                return;
            }
        };

        let Some(size) = KSize::from_wh(placement.dest_w, placement.dest_h) else {
            return;
        };

        let final_transform =
            AffineTransform::from_translate(placement.offset_x, placement.offset_y)
                .concat(base_transform);
        surface.push_transform(&to_krilla_transform(final_transform));

        if (effective_alpha - 1.0).abs() > 1e-4 {
            surface.push_opacity(
                NormalizedF32::new(effective_alpha.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE),
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
        placement: &ImagePlacement,
        base_transform: AffineTransform,
        effective_alpha: f32,
    ) {
        self.draw_raster_pixmap(
            surface,
            path,
            pixmap,
            placement,
            base_transform,
            effective_alpha,
        );
    }

    fn draw_raster_pixmap(
        &mut self,
        surface: &mut krilla::surface::Surface,
        cache_key: &str,
        pixmap: &RawPixmap,
        placement: &ImagePlacement,
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

        let Some(size) = KSize::from_wh(placement.dest_w, placement.dest_h) else {
            return;
        };

        let final_transform =
            AffineTransform::from_translate(placement.offset_x, placement.offset_y)
                .concat(base_transform);
        surface.push_transform(&to_krilla_transform(final_transform));

        if (effective_alpha - 1.0).abs() > 1e-4 {
            surface.push_opacity(
                NormalizedF32::new(effective_alpha.clamp(0.0, 1.0)).unwrap_or(NormalizedF32::ONE),
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

#[allow(clippy::too_many_arguments)]
fn render_text_lines(
    surface: &mut krilla::surface::Surface,
    lines: &[TextChild],
    cached_lines: &[Arc<renderer_core::glyph_cache::CachedLine>],
    parent_alpha: f32,
    sh_language: Option<&str>,
    sh_theme: Option<&str>,
    fit_scale: (f32, f32),
) {
    // See renderer-skia's `render_text_lines` for the derivation: a placeable
    // run's `override_offset` is a final-space delta; dividing by the
    // block's uniform fit-scale converts it to the raw glyph-space this
    // function paints in. `override_transform` (rotate/scale/pivot) is
    // already raw and composes *after* that conversion, never before
    // (mixing final- and raw-space quantities via `concat`'s cross terms
    // once rotation is involved would otherwise be wrong).
    let (fit_sx, fit_sy) = fit_scale;
    let raw_local_transform = |span: &TextSpan| -> AffineTransform {
        let raw_dx = span
            .override_offset
            .map(|(dx, _)| {
                if fit_sx.abs() > 1e-6 {
                    dx / fit_sx
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let raw_dy = span
            .override_offset
            .map(|(_, dy)| {
                if fit_sy.abs() > 1e-6 {
                    dy / fit_sy
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        span.override_transform
            .unwrap_or_else(AffineTransform::identity)
            .concat(AffineTransform::from_translate(raw_dx, raw_dy))
    };
    // Build syntax-highlight context if requested.
    let sh_ctx: Option<(Vec<Vec<usize>>, highlight::SyntaxColors)> = sh_language.map(|lang| {
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
    for (line_idx, (line, cached)) in lines.iter().zip(cached_lines.iter()).enumerate() {
        let mut spans: Vec<&TextSpan> = Vec::new();
        collect_spans(line, &mut spans);
        if spans.is_empty() {
            continue;
        }

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
            let fill_color: Option<Color> = if let Some((ref span_starts, ref sh_colors)) = sh_ctx {
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
                    // Gradients aren't supported on text fill — degrade to the
                    // first stop as a solid color (`Paint::solid_or_first_stop`).
                    let fallback = span.text_style.fill_color.value().solid_or_first_stop();
                    sh_colors.color_at(byte_in_full).or_else(|| {
                        if !fallback.is_transparent() {
                            Some(fallback.clone())
                        } else {
                            None
                        }
                    })
                } else {
                    let c = span.text_style.fill_color.value().solid_or_first_stop();
                    if !c.is_transparent() {
                        Some(c.clone())
                    } else {
                        None
                    }
                }
            } else {
                let c = span.text_style.fill_color.value().solid_or_first_stop();
                if !c.is_transparent() {
                    Some(c.clone())
                } else {
                    None
                }
            };

            // Build glyph path shifted by y_cursor plus any placeable-run override.
            let local =
                raw_local_transform(span).concat(AffineTransform::from_translate(0.0, y_cursor));
            let verbs: Vec<PathVerb> = glyph
                .path
                .verbs
                .iter()
                .map(|v| shift_verb(v, &local))
                .collect();
            let Some(path) = verbs_to_krilla_path(&verbs) else {
                continue;
            };

            if let Some(ref fc) = fill_color {
                surface.set_fill(Some(color_fill(fc, alpha)));
                surface.set_stroke(None);
                surface.draw_path(&path);
            }
            if !span.text_style.stroke_color.value().is_transparent() {
                let sc = span.text_style.stroke_color.value();
                surface.set_fill(None);
                surface.set_stroke(Some(color_stroke(
                    sc,
                    *span.text_style.stroke_width.value() as f32,
                    alpha,
                    None,
                    0.0,
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

/// An image layer override's `position` is a translate-only nudge on top of
/// wherever its content already sits in the source SVG/ORA composite — unlike
/// Rect/Group/Image, a layer's content is not anchored at its own local
/// (0, 0), so rotation/scale/pivot (which assume that) are not applied here;
/// doing so can swing content arbitrarily far from view. Deferred until layer
/// content has a real local bounding box to rotate/scale/pivot around (same
/// category of gap as Path's, `api-v2-impl.md` item 2).
fn image_layer_translate(
    override_: Option<&ImageLayer>,
    base_transform: AffineTransform,
) -> AffineTransform {
    let Some(ov) = override_ else {
        return base_transform;
    };
    AffineTransform::from_translate(ov.node_box.position.x as f32, ov.node_box.position.y as f32)
        .concat(base_transform)
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

/// Map a glyph path verb's points through `local` — carries the per-line
/// stacking cursor (a pure translation) and, for an overridden run, its
/// raw-space position/rotate/scale/pivot delta. Applying an affine map to a
/// Bezier curve's control points is equivalent to applying it to the curve
/// itself, so this is exact, not an approximation.
fn shift_verb(v: &PathVerb, local: &AffineTransform) -> PathVerb {
    match *v {
        PathVerb::MoveTo(x, y) => {
            let (x, y) = local.apply(x, y);
            PathVerb::MoveTo(x, y)
        }
        PathVerb::LineTo(x, y) => {
            let (x, y) = local.apply(x, y);
            PathVerb::LineTo(x, y)
        }
        PathVerb::QuadTo(cx, cy, x, y) => {
            let (cx, cy) = local.apply(cx, cy);
            let (x, y) = local.apply(x, y);
            PathVerb::QuadTo(cx, cy, x, y)
        }
        PathVerb::CubicTo(c0x, c0y, c1x, c1y, x, y) => {
            let (c0x, c0y) = local.apply(c0x, c0y);
            let (c1x, c1y) = local.apply(c1x, c1y);
            let (x, y) = local.apply(x, y);
            PathVerb::CubicTo(c0x, c0y, c1x, c1y, x, y)
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
    const K: f32 = 0.552_284_8;
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

/// Solid or gradient fill — `bounds` (local, untransformed space) resolves a
/// gradient's `angle` into concrete line endpoints via the same
/// `gradient_line_endpoints` formula renderer-skia uses, so raster and PDF
/// output agree on gradient placement.
fn paint_fill(paint: &RcPaint, alpha: f32, bounds: (f32, f32, f32, f32)) -> Fill {
    match paint {
        RcPaint::Solid(c) => color_fill(c, alpha),
        RcPaint::LinearGradient { stops, angle } => {
            let (start, end) = gradient_line_endpoints(*angle, bounds);
            let krilla_stops: Vec<krilla::paint::Stop> = stops
                .iter()
                .map(|(offset, c)| {
                    let (r, g, b, a) = c.to_rgba_f32();
                    let opacity = (a * alpha).clamp(0.0, 1.0);
                    krilla::paint::Stop {
                        offset: NormalizedF32::new(*offset as f32).unwrap_or(NormalizedF32::ZERO),
                        color: krilla::color::rgb::Color::new(
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8,
                        )
                        .into(),
                        opacity: NormalizedF32::new(opacity).unwrap_or(NormalizedF32::ONE),
                    }
                })
                .collect();
            Fill {
                paint: krilla::paint::LinearGradient {
                    x1: start.0,
                    y1: start.1,
                    x2: end.0,
                    y2: end.1,
                    transform: to_krilla_transform(AffineTransform::identity()),
                    spread_method: krilla::paint::SpreadMethod::Pad,
                    stops: krilla_stops,
                    anti_alias: true,
                }
                .into(),
                // Per-stop opacity already carries `alpha` — the outer Fill
                // opacity would otherwise apply a second time on top.
                opacity: NormalizedF32::ONE,
                rule: FillRule::NonZero,
            }
        }
    }
}

fn color_stroke(
    c: &Color,
    width: f32,
    alpha: f32,
    dash: Option<(f64, f64)>,
    dash_offset: f32,
) -> Stroke {
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
        dash: dash.map(|(on, off)| krilla::paint::StrokeDash {
            array: vec![on as f32, off as f32],
            offset: dash_offset,
        }),
        ..Default::default()
    }
}

fn fill_and_stroke(
    surface: &mut krilla::surface::Surface,
    path: &Path,
    style: &Style,
    alpha: f32,
    bounds: (f32, f32, f32, f32),
) {
    let effective_alpha = alpha * style.alpha as f32;
    if !style.fill_color.is_transparent() {
        surface.set_fill(Some(paint_fill(&style.fill_color, effective_alpha, bounds)));
        surface.set_stroke(None);
        surface.draw_path(path);
    }
    if !style.stroke_color.is_transparent() {
        surface.set_fill(None);
        surface.set_stroke(Some(color_stroke(
            &style.stroke_color,
            style.stroke_width as f32,
            effective_alpha,
            style.dash,
            style.dash_offset as f32,
        )));
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

#[cfg(test)]
mod tests {
    use super::*;
    use renderer_core::{Camera, Inheritable, NodeBox};

    #[test]
    fn gradient_fill_emits_a_pdf_shading_not_a_flat_color() {
        let node_box = NodeBox {
            position: Position { x: 0.0, y: 0.0 },
            size: Size {
                width: 100.0,
                height: 40.0,
            },
            z_level: Inheritable::Own(0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            pivot_x: 0.0,
            pivot_y: 0.0,
        };
        let style = Style {
            fill_color: RcPaint::LinearGradient {
                stops: vec![
                    (0.0, Color::from_rgba8(255, 0, 0, 255)),
                    (1.0, Color::from_rgba8(0, 0, 255, 255)),
                ],
                angle: 90.0,
            },
            stroke_color: Color::from_rgba8(0, 0, 0, 0),
            stroke_width: 0.0,
            alpha: 1.0,
            dash: None,
            dash_offset: 0.0,
        };
        let scene = Scene {
            width: 100.0,
            height: 40.0,
            fill_color: Color::from_rgba8(255, 255, 255, 255),
            camera: Camera {
                camera_zoom: 1.0,
                camera_x: 50.0,
                camera_y: 20.0,
            },
            children: vec![Node {
                id: 0,
                kind: NodeKind::Rect {
                    node_box,
                    style,
                    radius: 0.0,
                },
            }],
        };
        let pdf_bytes = render_to_pdf(&[&scene]).unwrap();
        // PDF's axial-shading dictionary type (`/ShadingType 2`) — appears as
        // plain (uncompressed) object bytes, so a raw substring check is
        // reliable without a PDF parser/rasterizer dependency. Confirms a
        // real gradient shading was emitted, not a flattened solid fallback.
        assert!(
            pdf_bytes
                .windows(b"/ShadingType".len())
                .any(|w| w == b"/ShadingType"),
            "expected the PDF to contain a /ShadingType object for the gradient fill"
        );
    }
}
