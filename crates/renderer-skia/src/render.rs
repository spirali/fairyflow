use renderer_core::glyph_cache::{PathVerb, VectorPath};
use renderer_core::image_cache::{self, CachedImageKind, RawPixmap};
use renderer_core::path_utils::{build_cropped_path_verbs, build_rounded_rect_verbs, offset_verbs};
use renderer_core::resources::Resources;
use renderer_core::text_decorations::{DecorationKind, DecorationRect, decoration_rects};
use renderer_core::text_layout::{build_span_text, collect_spans};
use renderer_core::transform::{
    AffineTransform, gradient_line_endpoints, node_z_level,
    positional_transform as core_positional_transform,
};
use renderer_core::{
    Color, ImageLayer, Node, NodeKind, Paint as RcPaint, PathCommand, Position, Scene, Size, Style,
    TextChild, TextSpan,
};
use renderer_core::{NodeBox, highlight};
use resvg::usvg;
use std::sync::Arc;
use tiny_skia::{
    FillRule, FilterQuality, GradientStop, LinearGradient, Mask, Paint, PathBuilder, Pixmap,
    PixmapPaint, Point, Rect, Shader, SpreadMode, Stroke, Transform,
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
        let box_center = Position::new(scene.width * 0.5, scene.height * 0.5);
        let content_transform = skia_from_affine(
            renderer_core::camera_transform(&scene.camera, box_center)
                .concat(affine_from_skia(Transform::from_scale(scale, scale))),
        );
        self.render_children(&scene.children, &mut pixmap, content_transform, 1.0);
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
                let transform = transform_nodebox(node_box, parent_transform);
                let box_center =
                    Position::new(node_box.size.width * 0.5, node_box.size.height * 0.5);
                let content_transform = skia_from_affine(
                    renderer_core::camera_transform(camera, box_center)
                        .concat(affine_from_skia(transform)),
                );
                let alpha = parent_alpha * *alpha as f32;
                let children = children.clone();

                let needs_clip = *clip_enabled
                    || *clip_x > 0.0
                    || *clip_y > 0.0
                    || *clip_w < 1.0
                    || *clip_h < 1.0;
                if !needs_clip {
                    self.render_children(&children, pixmap, content_transform, alpha);
                } else {
                    let w = pixmap.width();
                    let h = pixmap.height();
                    let mut offscreen = Pixmap::new(w, h).expect("offscreen pixmap");
                    self.render_children(&children, &mut offscreen, content_transform, alpha);

                    let lw = node_box.size.width as f32;
                    let lh = node_box.size.height as f32;
                    if let Some(clip_rect) = Rect::from_xywh(
                        *clip_x as f32 * lw,
                        *clip_y as f32 * lh,
                        *clip_w as f32 * lw,
                        *clip_h as f32 * lh,
                    ) {
                        // Pre-transform to screen space so mask.fill_path is called
                        // with identity — avoids tiny-skia's degenerate-path warning
                        // when the transform squashes one axis to nearly zero.
                        if let Some(screen_clip) =
                            PathBuilder::from_rect(clip_rect).transform(transform)
                        {
                            let b = screen_clip.bounds();
                            // SCALAR_NEARLY_ZERO = 1/4096; mirror tiny-skia's own check
                            if b.width() > (1.0 / 4096.0)
                                && b.height() > (1.0 / 4096.0)
                                && let Some(mut mask) = Mask::new(w, h)
                            {
                                mask.fill_path(
                                    &screen_clip,
                                    FillRule::Winding,
                                    true,
                                    Transform::identity(),
                                );
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
            }
            NodeKind::Rect {
                node_box,
                style,
                radius,
            } => {
                let transform = transform_nodebox(node_box, parent_transform);
                let w = node_box.size.width as f32;
                let h = node_box.size.height as f32;
                let path = if *radius > 0.0 {
                    verbs_to_skia_path(&build_rounded_rect_verbs(w, h, *radius as f32))
                } else {
                    Rect::from_xywh(0.0, 0.0, w, h).map(PathBuilder::from_rect)
                };
                let Some(path) = path else {
                    return;
                };
                fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
            }
            NodeKind::Ellipse { node_box, style } => {
                let transform = transform_nodebox(node_box, parent_transform);
                let Some(oval) = Rect::from_xywh(
                    0.0,
                    0.0,
                    node_box.size.width as f32,
                    node_box.size.height as f32,
                ) else {
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
                z_level: _,
                crop_start,
                crop_end,
                ..
            } => {
                if let Some(path) = build_cropped_path(children, *crop_start, *crop_end) {
                    fill_and_stroke(&path, style, pixmap, parent_transform, parent_alpha);
                }
            }
            NodeKind::Text {
                node_box,
                keep_aspect,
                wrap,
                text_align,
                lines,
                sh_language,
                sh_theme,
                uncommon,
                ..
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
                let box_transform = transform_nodebox(node_box, parent_transform);
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
                let sh = sh_language.as_ref().map(|lang| {
                    let theme = sh_theme
                        .as_ref()
                        .map(|s| s.as_str())
                        .unwrap_or("InspiredGitHub");
                    (lang.as_str(), theme)
                });
                let reveal = uncommon.as_ref().map(|u| u.reveal as f32).unwrap_or(1.0);
                self.render_text_lines(
                    lines,
                    &laid_out.lines,
                    pixmap,
                    content_transform,
                    parent_alpha,
                    sh,
                    (sx, sy),
                    reveal,
                );
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
                let layers = layers.clone();
                let hidden_layers = hidden_layers.clone();
                render_image(
                    &ImageSpec {
                        path,
                        size: node_box.size,
                        keep_aspect: *keep_aspect,
                        layers: &layers,
                        hidden_layers: &hidden_layers,
                    },
                    pixmap,
                    parent_transform,
                    node_box,
                    effective_alpha,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_text_lines(
        &self,
        lines: &[TextChild],
        cached_lines: &[Arc<renderer_core::glyph_cache::CachedLine>],
        pixmap: &mut Pixmap,
        parent_transform: Transform,
        parent_alpha: f32,
        sh: Option<(&str, &str)>,
        fit_scale: (f32, f32),
        reveal: f32,
    ) {
        // Typewriter reveal: a hard per-glyph cutoff over the
        // already-laid-out glyphs, in reading order across all lines — never
        // a reflow. At `reveal >= 1.0` every glyph's index is below the
        // cutoff, so this is a no-op for the common (never called `type_on`)
        // case.
        let total_glyphs: usize = cached_lines.iter().map(|l| l.glyphs.len()).sum();
        let reveal_count = (reveal as f64 * total_glyphs as f64).floor() as usize;
        let mut glyph_index = 0usize;

        // A placeable run's `override_offset` is a delta in the block's final
        // (fit-scaled) coordinate space — the same space `.at()` queries use —
        // but glyph paths are built in raw, pre-fit-scale space, with the
        // uniform `fit_scale` applied once via `parent_transform`. A uniform
        // scale means offsets cancel in a delta, so dividing by `fit_scale` is
        // the exact (and only) conversion needed; guard near-zero the same way
        // `text_fit_scale` guards a degenerate natural extent. `override_transform`
        // (rotate/scale/pivot) is already raw — rotation/scale are
        // resolution-independent ratios, not distances — so it composes
        // directly, but *after* the offset's conversion, never before (concat's
        // cross terms would otherwise mix final- and raw-space quantities once
        // rotation is involved).
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
                let should_draw = glyph_index < reveal_count;
                glyph_index += 1;
                if !should_draw {
                    continue;
                }

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

                        // Gradients aren't supported on text fill — degrade to the
                        // first stop as a solid color (`Paint::solid_or_first_stop`).
                        let fallback = span.text_style.fill_color.value().solid_or_first_stop();
                        sh_colors
                            .color_at(byte_in_full)
                            .map(|c| color_to_skia(&c))
                            .or_else(|| {
                                if !fallback.is_transparent() {
                                    Some(color_to_skia(fallback))
                                } else {
                                    None
                                }
                            })
                    } else {
                        let c = span.text_style.fill_color.value().solid_or_first_stop();
                        if !c.is_transparent() {
                            Some(color_to_skia(c))
                        } else {
                            None
                        }
                    }
                } else {
                    let c = span.text_style.fill_color.value().solid_or_first_stop();
                    if !c.is_transparent() {
                        Some(color_to_skia(c))
                    } else {
                        None
                    }
                };

                let local = raw_local_transform(span)
                    .concat(AffineTransform::from_translate(0.0, y_cursor));
                let Some(path) = vector_path_to_skia(&glyph.path, &local) else {
                    continue;
                };

                if let Some(mut color) = fill_color {
                    let b = path.bounds();
                    if b.width() > (1.0 / 4096.0) && b.height() > (1.0 / 4096.0) {
                        color.set_alpha(color.alpha() * alpha);
                        let mut paint = Paint::default();
                        paint.set_color(color);
                        paint.anti_alias = true;
                        pixmap.fill_path(&path, &paint, FillRule::Winding, parent_transform, None);
                    }
                }
                if !span.text_style.stroke_color.value().is_transparent() {
                    let sc = span.text_style.stroke_color.value();
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

            let local = |span: &TextSpan| -> AffineTransform {
                raw_local_transform(span).concat(AffineTransform::from_translate(0.0, y_cursor))
            };
            for kind in [DecorationKind::Underline, DecorationKind::Strike] {
                for rect in decoration_rects(cached, &spans, kind) {
                    let span = spans[rect.span_idx];
                    let alpha = parent_alpha * *span.text_style.alpha.value() as f32;
                    draw_decoration_rect(&rect, &local(span), alpha, pixmap, parent_transform);
                }
            }

            y_cursor += cached.height;
        }
    }
}

/// Paint one underline/strike rect — reuses the exact same row-local ->
/// screen transform pipeline glyph painting already goes through
/// (`vector_path_to_skia`/`local`), so a decoration correctly follows the
/// same per-run rotate/scale/pivot/position override and `y_cursor` row
/// stacking a glyph would. `rect`'s x/y/width/thickness are already in that
/// same raw, row-local coordinate space (see `DecorationRect`'s own docs).
fn draw_decoration_rect(
    rect: &DecorationRect,
    local: &AffineTransform,
    alpha: f32,
    pixmap: &mut Pixmap,
    parent_transform: Transform,
) {
    let verbs = offset_verbs(
        &build_rounded_rect_verbs(rect.width, rect.thickness, 0.0),
        rect.x,
        rect.y,
    );
    let vp = VectorPath { verbs };
    let Some(path) = vector_path_to_skia(&vp, local) else {
        return;
    };
    let mut color = color_to_skia(&rect.color);
    color.set_alpha(color.alpha() * alpha);
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    pixmap.fill_path(&path, &paint, FillRule::Winding, parent_transform, None);
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

/// Packs a `Pixmap` into `buffer` as `0x00RRGGBB` u32 values (softbuffer-compatible
/// format). `buffer` must be at least `pixmap.width() * pixmap.height()` long.
pub fn pack_pixmap_to_buffer(pixmap: &Pixmap, buffer: &mut [u32]) {
    for (dst, src) in buffer.iter_mut().zip(pixmap.pixels()) {
        *dst = ((src.red() as u32) << 16) | ((src.green() as u32) << 8) | (src.blue() as u32);
    }
}

/// Renders `scene` fitted (letterboxed) into `width × height` and writes the
/// result into `buffer` as `0x00RRGGBB` u32 values (softbuffer-compatible format).
pub fn render_scene_to_buffer(scene: &Scene, width: u32, height: u32, buffer: &mut [u32]) {
    let pixmap = render_scene_fitted(scene, width, height);
    pack_pixmap_to_buffer(&pixmap, buffer);
}

/// Renders `main` fitted into the top `width × (height - strip_height)` and `strip`
/// fitted into a `width × strip_height` band at the bottom, composited into one
/// `width × height` buffer (softbuffer-compatible `0x00RRGGBB` u32 values). Used by
/// the standalone player's speaker-notes overlay — `strip` is a small synthetic
/// `Scene` built from note text, reusing the same text-rendering pipeline as any
/// other scene rather than needing separate UI-chrome font rasterization.
pub fn render_scene_with_strip_to_buffer(
    main: &Scene,
    strip: &Scene,
    strip_height: u32,
    width: u32,
    height: u32,
    buffer: &mut [u32],
) {
    let main_h = height.saturating_sub(strip_height);
    let main_pixmap = render_scene_fitted(main, width, main_h);
    let strip_pixmap = render_scene_fitted(strip, width, strip_height);

    let mut canvas = Pixmap::new(width, height).expect("invalid target resolution");
    canvas.draw_pixmap(
        0,
        0,
        main_pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    canvas.draw_pixmap(
        0,
        main_h as i32,
        strip_pixmap.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    pack_pixmap_to_buffer(&canvas, buffer);
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

fn transform_nodebox(node_box: &NodeBox, parent: Transform) -> Transform {
    positional_transform(
        node_box.position,
        Size {
            width: node_box.scale_x,
            height: node_box.scale_y,
        },
        node_box.rotation,
        node_box.pivot_x as f32,
        node_box.pivot_y as f32,
        parent,
    )
}

fn positional_transform(
    position: Position,
    scale: Size,
    rotation: f64,
    pivot_x: f32,
    pivot_y: f32,
    parent: Transform,
) -> Transform {
    let t = core_positional_transform(
        position,
        scale,
        rotation,
        pivot_x,
        pivot_y,
        affine_from_skia(parent),
    );
    skia_from_affine(t)
}

fn affine_from_skia(t: Transform) -> AffineTransform {
    AffineTransform {
        a: t.sx,
        b: t.ky,
        c: t.kx,
        d: t.sy,
        e: t.tx,
        f: t.ty,
    }
}

fn skia_from_affine(t: AffineTransform) -> Transform {
    Transform::from_row(t.a, t.b, t.c, t.d, t.e, t.f)
}

/// An image layer override's `position` is a translate-only nudge on top of
/// wherever its content already sits in the source SVG/ORA composite — unlike
/// Rect/Group/Image, a layer's content is not anchored at its own local
/// (0, 0), so rotation/scale/pivot (which assume that) are not applied here;
/// doing so can swing content arbitrarily far from view. Deferred until layer
/// content has a real local bounding box to rotate/scale/pivot around (same
/// category of gap as Path's lack of a box).
fn image_layer_translate(override_: Option<&ImageLayer>, node_transform: Transform) -> Transform {
    let Some(ov) = override_ else {
        return node_transform;
    };
    node_transform.post_translate(ov.node_box.position.x as f32, ov.node_box.position.y as f32)
}

fn fill_and_stroke(
    path: &tiny_skia::Path,
    style: &Style,
    pixmap: &mut Pixmap,
    transform: Transform,
    parent_alpha: f32,
) {
    let alpha = parent_alpha * style.alpha as f32;
    if !style.fill_color.is_transparent() {
        let b = path.bounds();
        if b.width() > (1.0 / 4096.0) && b.height() > (1.0 / 4096.0) {
            let shader = match &style.fill_color {
                RcPaint::Solid(c) => {
                    let mut color = color_to_skia(c);
                    color.set_alpha(color.alpha() * alpha);
                    Shader::SolidColor(color)
                }
                RcPaint::LinearGradient { stops, angle } => {
                    let bounds = (b.x(), b.y(), b.width(), b.height());
                    let (start, end) = gradient_line_endpoints(*angle, bounds);
                    let grad_stops: Vec<GradientStop> = stops
                        .iter()
                        .map(|(offset, c)| {
                            let mut sc = color_to_skia(c);
                            sc.set_alpha(sc.alpha() * alpha);
                            GradientStop::new(*offset as f32, sc)
                        })
                        .collect();
                    LinearGradient::new(
                        Point::from_xy(start.0, start.1),
                        Point::from_xy(end.0, end.1),
                        grad_stops,
                        SpreadMode::Pad,
                        Transform::identity(),
                    )
                    .unwrap_or(Shader::SolidColor(tiny_skia::Color::TRANSPARENT))
                }
            };
            let paint = Paint {
                shader,
                anti_alias: true,
                ..Default::default()
            };
            pixmap.fill_path(path, &paint, FillRule::Winding, transform, None);
        }
    }
    if !style.stroke_color.is_transparent() {
        let mut color = color_to_skia(&style.stroke_color);
        color.set_alpha(color.alpha() * alpha);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke {
            width: style.stroke_width as f32,
            dash: style.dash.and_then(|(on, off)| {
                tiny_skia::StrokeDash::new(vec![on as f32, off as f32], style.dash_offset as f32)
            }),
            ..Default::default()
        };
        pixmap.stroke_path(path, &paint, &stroke, transform, None);
    }
}

/// Convert a `VectorPath` to a tiny-skia `Path`, mapping every point through
/// `local` — carries the per-line stacking cursor (a pure translation) and,
/// for an overridden run, its raw-space position/rotate/scale/pivot delta.
/// Applying an affine map to a Bezier curve's control points is equivalent to
/// applying it to the curve itself, so transforming each verb's raw
/// coordinates directly (rather than the assembled path) is exact, not an
/// approximation.
fn vector_path_to_skia(vp: &VectorPath, local: &AffineTransform) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for verb in &vp.verbs {
        match verb {
            PathVerb::MoveTo(x, y) => {
                let (x, y) = local.apply(*x, *y);
                pb.move_to(x, y)
            }
            PathVerb::LineTo(x, y) => {
                let (x, y) = local.apply(*x, *y);
                pb.line_to(x, y)
            }
            PathVerb::QuadTo(cx, cy, x, y) => {
                let (cx, cy) = local.apply(*cx, *cy);
                let (x, y) = local.apply(*x, *y);
                pb.quad_to(cx, cy, x, y)
            }
            PathVerb::CubicTo(cx0, cy0, cx1, cy1, x, y) => {
                let (cx0, cy0) = local.apply(*cx0, *cy0);
                let (cx1, cy1) = local.apply(*cx1, *cy1);
                let (x, y) = local.apply(*x, *y);
                pb.cubic_to(cx0, cy0, cx1, cy1, x, y)
            }
            PathVerb::Close => pb.close(),
        }
    }
    pb.finish()
}

// ── Path building ─────────────────────────────────────────────────────────────

fn verbs_to_skia_path(verbs: &[PathVerb]) -> Option<tiny_skia::Path> {
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

fn build_cropped_path(
    commands: &[PathCommand],
    crop_start: f64,
    crop_end: f64,
) -> Option<tiny_skia::Path> {
    verbs_to_skia_path(&build_cropped_path_verbs(commands, crop_start, crop_end))
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

// ── Image rendering ───────────────────────────────────────────────────────────

struct ImageSpec<'a> {
    path: &'a str,
    size: Size,
    keep_aspect: bool,
    layers: &'a [ImageLayer],
    hidden_layers: &'a [Arc<String>],
}

/// How a source image is scaled and offset within its destination rect.
struct ImagePlacement {
    sx: f32,
    sy: f32,
    offset_x: f32,
    offset_y: f32,
    dest_w: f32,
    dest_h: f32,
}

fn render_raster_pixmap(
    src: &RawPixmap,
    placement: &ImagePlacement,
    node_transform: Transform,
    pixmap: &mut Pixmap,
    effective_alpha: f32,
) {
    let Some(src_pixmap) = raw_to_pixmap(src) else {
        return;
    };
    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        ..Default::default()
    };
    if (effective_alpha - 1.0).abs() < 1e-6 {
        let transform = Transform::from_scale(placement.sx, placement.sy)
            .post_translate(placement.offset_x, placement.offset_y)
            .post_concat(node_transform);
        pixmap.draw_pixmap(0, 0, src_pixmap.as_ref(), &paint, transform, None);
    } else {
        let w_u32 = placement.dest_w.ceil() as u32;
        let h_u32 = placement.dest_h.ceil() as u32;
        let Some(mut img_pixmap) = Pixmap::new(w_u32.max(1), h_u32.max(1)) else {
            return;
        };
        let inner_transform = Transform::from_scale(placement.sx, placement.sy)
            .post_translate(placement.offset_x, placement.offset_y);
        img_pixmap.draw_pixmap(0, 0, src_pixmap.as_ref(), &paint, inner_transform, None);
        let composite_paint = PixmapPaint {
            opacity: effective_alpha.clamp(0.0, 1.0),
            ..Default::default()
        };
        pixmap.draw_pixmap(
            0,
            0,
            img_pixmap.as_ref(),
            &composite_paint,
            node_transform,
            None,
        );
    }
}

fn render_svg_tree(
    tree: &usvg::Tree,
    placement: &ImagePlacement,
    node_transform: Transform,
    pixmap: &mut Pixmap,
    effective_alpha: f32,
) {
    if (effective_alpha - 1.0).abs() < 1e-6 {
        let svg_transform = Transform::from_scale(placement.sx, placement.sy)
            .post_translate(placement.offset_x, placement.offset_y)
            .post_concat(node_transform);
        resvg::render(tree, svg_transform, &mut pixmap.as_mut());
    } else {
        let w_u32 = placement.dest_w.ceil() as u32;
        let h_u32 = placement.dest_h.ceil() as u32;
        let Some(mut img_pixmap) = Pixmap::new(w_u32.max(1), h_u32.max(1)) else {
            return;
        };
        let svg_transform = Transform::from_scale(placement.sx, placement.sy)
            .post_translate(placement.offset_x, placement.offset_y);
        resvg::render(tree, svg_transform, &mut img_pixmap.as_mut());
        let paint = PixmapPaint {
            opacity: effective_alpha.clamp(0.0, 1.0),
            ..Default::default()
        };
        pixmap.draw_pixmap(0, 0, img_pixmap.as_ref(), &paint, node_transform, None);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_image(
    spec: &ImageSpec<'_>,
    pixmap: &mut Pixmap,
    parent_transform: Transform,
    node_box: &NodeBox,
    effective_alpha: f32,
) {
    let Some(cached) = image_cache::load_image(spec.path) else {
        return;
    };
    let dest_w = spec.size.width as f32;
    let dest_h = spec.size.height as f32;
    if dest_w <= 0.0 || dest_h <= 0.0 || cached.width <= 0.0 || cached.height <= 0.0 {
        return;
    }

    let placement = if spec.keep_aspect {
        let s = (dest_w / cached.width).min(dest_h / cached.height);
        let actual_w = cached.width * s;
        let actual_h = cached.height * s;
        ImagePlacement {
            sx: s,
            sy: s,
            offset_x: (dest_w - actual_w) / 2.0,
            offset_y: (dest_h - actual_h) / 2.0,
            dest_w,
            dest_h,
        }
    } else {
        ImagePlacement {
            sx: dest_w / cached.width,
            sy: dest_h / cached.height,
            offset_x: 0.0,
            offset_y: 0.0,
            dest_w,
            dest_h,
        }
    };

    let node_transform = transform_nodebox(node_box, parent_transform);

    match &cached.kind {
        CachedImageKind::Svg { tree, .. } => {
            if spec.layers.is_empty() && spec.hidden_layers.is_empty() {
                render_svg_tree(tree, &placement, node_transform, pixmap, effective_alpha);
            } else {
                let all_labels = cached.image_layers.as_ref();
                if all_labels.is_none_or(|labels| labels.is_empty()) {
                    render_svg_tree(tree, &placement, node_transform, pixmap, effective_alpha);
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
                        let layer_tree = match &layer_cached.kind {
                            CachedImageKind::Svg { tree, .. } => tree,
                            _ => continue,
                        };
                        let layer_transform = image_layer_translate(override_, node_transform);
                        render_svg_tree(
                            layer_tree,
                            &placement,
                            layer_transform,
                            pixmap,
                            layer_alpha,
                        );
                    }
                }
            }
        }
        CachedImageKind::Raster { pixmap: src } => {
            render_raster_pixmap(src, &placement, node_transform, pixmap, effective_alpha);
        }
        CachedImageKind::Ora { layers: ora_layers } => {
            let all_labels = &cached.image_layers;
            if spec.layers.is_empty() && spec.hidden_layers.is_empty() {
                for layer_data in ora_layers.iter() {
                    let layer_placement = ImagePlacement {
                        offset_x: placement.offset_x + layer_data.x as f32 * placement.sx,
                        offset_y: placement.offset_y + layer_data.y as f32 * placement.sy,
                        ..placement
                    };
                    render_raster_pixmap(
                        &layer_data.pixmap,
                        &layer_placement,
                        node_transform,
                        pixmap,
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
                    let layer_placement = ImagePlacement {
                        offset_x: placement.offset_x + layer_data.x as f32 * placement.sx,
                        offset_y: placement.offset_y + layer_data.y as f32 * placement.sy,
                        ..placement
                    };
                    let layer_transform = image_layer_translate(override_, node_transform);
                    render_raster_pixmap(
                        &layer_data.pixmap,
                        &layer_placement,
                        layer_transform,
                        pixmap,
                        layer_alpha,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renderer_core::{Camera, Inheritable, NodeBox, TextAlign, TextNodeUncommon, TextStyle};

    fn count_non_background_pixels(pixmap: &Pixmap) -> usize {
        pixmap
            .pixels()
            .iter()
            .filter(|p| !(p.red() == 255 && p.green() == 255 && p.blue() == 255))
            .count()
    }

    fn text_style_for_test() -> TextStyle {
        TextStyle {
            fill_color: Inheritable::Own(RcPaint::Solid(Color::from_rgba8(0, 0, 0, 255))),
            stroke_color: Inheritable::Own(Color::from_rgba8(0, 0, 0, 0)),
            stroke_width: Inheritable::Own(0.0),
            alpha: Inheritable::Own(1.0),
            font_family: Inheritable::Own(Arc::new("sans-serif".to_string())),
            font_size: Inheritable::Own(24.0),
            font_weight: Inheritable::Own(400.0),
            italic: Inheritable::Own(false),
            underline: Inheritable::Own(0.0),
            strike: Inheritable::Own(0.0),
        }
    }

    fn text_scene(reveal: f64) -> Scene {
        let uncommon = TextNodeUncommon {
            reveal,
            underline_color: None,
            underline_width: None,
            underline_offset: None,
            strike_color: None,
            strike_width: None,
            strike_offset: None,
        }
        .into_optional_box();
        let node_box = NodeBox {
            position: Position { x: 0.0, y: 0.0 },
            size: Size {
                width: 200.0,
                height: 40.0,
            },
            z_level: Inheritable::Own(0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            pivot_x: 0.0,
            pivot_y: 0.0,
        };
        Scene {
            width: 200.0,
            height: 40.0,
            fill_color: Color::from_rgba8(255, 255, 255, 255),
            camera: Camera {
                camera_zoom: 1.0,
                camera_x: 100.0,
                camera_y: 20.0,
            },
            children: vec![Node {
                id: 0,
                kind: NodeKind::Text {
                    node_box,
                    keep_aspect: false,
                    wrap: None,
                    text_align: TextAlign::Left,
                    text_style: text_style_for_test(),
                    sh_language: None,
                    sh_theme: None,
                    uncommon,
                    lines: vec![TextChild::Span(TextSpan {
                        id: 1,
                        text: Arc::new("Hello fairyflow".to_string()),
                        text_style: text_style_for_test(),
                        underline_color: None,
                        underline_width: None,
                        underline_offset: None,
                        strike_color: None,
                        strike_width: None,
                        strike_offset: None,
                        override_offset: None,
                        override_transform: None,
                    })],
                },
            }],
        }
    }

    #[test]
    fn type_on_reveal_zero_draws_nothing() {
        Resources::init();
        let pixmap = render_scene(&text_scene(0.0), 1.0);
        assert_eq!(count_non_background_pixels(&pixmap), 0);
    }

    #[test]
    fn type_on_reveal_one_draws_the_full_text() {
        Resources::init();
        let pixmap = render_scene(&text_scene(1.0), 1.0);
        assert!(count_non_background_pixels(&pixmap) > 0);
    }

    #[test]
    fn type_on_partial_reveal_draws_strictly_fewer_pixels_than_full() {
        Resources::init();
        let half = count_non_background_pixels(&render_scene(&text_scene(0.5), 1.0));
        let full = count_non_background_pixels(&render_scene(&text_scene(1.0), 1.0));
        assert!(half > 0, "expected some glyphs visible at reveal=0.5");
        assert!(
            half < full,
            "expected reveal=0.5 ({half} px) to draw fewer pixels than reveal=1.0 ({full} px)"
        );
    }

    #[test]
    fn gradient_fill_produces_different_colors_across_the_axis() {
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
                    (0.0, renderer_core::Color::from_rgba8(255, 0, 0, 255)),
                    (1.0, renderer_core::Color::from_rgba8(0, 0, 255, 255)),
                ],
                angle: 90.0, // left-to-right, per gradient_line_endpoints' convention
            },
            stroke_color: renderer_core::Color::from_rgba8(0, 0, 0, 0),
            stroke_width: 0.0,
            alpha: 1.0,
            dash: None,
            dash_offset: 0.0,
        };
        let scene = Scene {
            width: 100.0,
            height: 40.0,
            fill_color: renderer_core::Color::from_rgba8(255, 255, 255, 255),
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
        let pixmap = render_scene(&scene, 1.0);
        let left = pixmap.pixel(2, 20).unwrap();
        let right = pixmap.pixel(97, 20).unwrap();
        assert!(
            left.red() > right.red() && right.blue() > left.blue(),
            "expected left (red stop) to right (blue stop) to actually vary: \
             left={left:?} right={right:?}"
        );
    }
}
