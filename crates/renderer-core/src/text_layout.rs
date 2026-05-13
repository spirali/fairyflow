use crate::glyph_cache::{self, CachedLine, PathVerb, VectorPath};
use crate::resources::Resources;
use crate::{TextChild, TextSpan};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontStack, FontWeight, LayoutContext,
    PositionedLayoutItem, StyleProperty,
};
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{LocationRef, NormalizedCoord, Size as SkrifaSize},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef as ReadFontsRef,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

const DEFAULT_FONT_SIZE: f32 = 16.0;

pub struct TextLayoutEngine {
    font_cx: FontContext,
    layout_cx: LayoutContext<usize>,
}

impl TextLayoutEngine {
    pub fn new(resources: &Resources) -> TextLayoutEngine {
        TextLayoutEngine {
            font_cx: resources.font_cx(),
            layout_cx: LayoutContext::new(),
        }
    }

    pub fn get_or_build_line(&mut self, spans: &[&TextSpan]) -> Arc<CachedLine> {
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
            builder.push(StyleProperty::Brush(*span_idx), range.clone());
            builder.push(
                StyleProperty::FontSize(*span.text_style.font_size.value() as f32),
                range.clone(),
            );
            builder.push(
                StyleProperty::FontStack(FontStack::Source(
                    (span.text_style.font_family.value().as_str()).into(),
                )),
                range.clone(),
            );
            builder.push(
                StyleProperty::FontWeight(FontWeight::new(
                    *span.text_style.font_weight.value() as f32
                )),
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
            // Per underlying Run: how many glyphs have already been consumed by
            // earlier GlyphRuns that share the same font run.  Parley can split
            // one font run into multiple GlyphRuns at style-brush boundaries, so
            // run.visual_clusters() returns ALL clusters for that font run.  We
            // must only process the slice belonging to the current GlyphRun.
            let mut run_glyph_offset: HashMap<usize, usize> = HashMap::new();

            for item in layout_line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let run = glyph_run.run();
                let run_idx = run.index();
                let font = run.font();
                let font_size = run.font_size();
                let normalized_coords: Vec<NormalizedCoord> = run
                    .normalized_coords()
                    .iter()
                    .map(|c| NormalizedCoord::from_bits(*c))
                    .collect();

                let font_ref = ReadFontsRef::from_index(font.data.as_ref(), font.index).unwrap();
                let outlines = font_ref.outline_glyphs();

                // Build a flat table: for each glyph index in this font run,
                // what is the cluster's byte offset in full_text?
                let cluster_bytes: Vec<u32> = run
                    .visual_clusters()
                    .flat_map(|cluster| {
                        let byte = cluster.text_range().start as u32;
                        let count = cluster.glyphs().count();
                        std::iter::repeat(byte).take(count)
                    })
                    .collect();

                let glyph_start = *run_glyph_offset.entry(run_idx).or_insert(0);
                let mut run_x = glyph_run.offset();
                let baseline = glyph_run.baseline();
                let mut local_glyph_count = 0usize;

                for glyph in glyph_run.glyphs() {
                    let cluster_byte = cluster_bytes
                        .get(glyph_start + local_glyph_count)
                        .copied()
                        .unwrap_or(0);
                    local_glyph_count += 1;

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

                *run_glyph_offset.get_mut(&run_idx).unwrap() += local_glyph_count;
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

    pub fn measure_text_lines(&mut self, lines: &[TextChild]) -> (f32, f32) {
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
    pub fn find_text_node_pos(
        &mut self,
        lines: &[TextChild],
        target_id: u64,
    ) -> Option<(f32, f32)> {
        let mut y_offset = 0.0f32;
        for line in lines {
            let mut tagged: Vec<(bool, &TextSpan)> = Vec::new();
            collect_spans_tagged(line, target_id, false, &mut tagged);

            let spans: Vec<&TextSpan> = tagged.iter().map(|(_, s)| *s).collect();
            if spans.is_empty() {
                continue;
            }

            let cached = self.get_or_build_line(&spans);

            let first_target_idx = tagged.iter().position(|(is_target, _)| *is_target);
            if let Some(target_span_idx) = first_target_idx
                && let Some(glyph) = cached.glyphs.iter().find(|g| g.span_idx == target_span_idx)
            {
                return Some((glyph.x, y_offset));
            }

            y_offset += cached.height;
        }
        None
    }
}

thread_local! {
    static LAYOUT_ENGINE: RefCell<TextLayoutEngine> =
        RefCell::new(TextLayoutEngine::new(Resources::get()));
}

/// Look up or build the cached line for `spans` using the thread-local engine.
pub fn get_or_build_line(spans: &[&TextSpan]) -> Arc<CachedLine> {
    LAYOUT_ENGINE.with(|e| e.borrow_mut().get_or_build_line(spans))
}

/// Measure the natural (unwrapped) dimensions of a text block.
/// Returns `(width, height)` in logical pixels (scale = 1).
pub fn measure_text(lines: &[TextChild]) -> (f32, f32) {
    LAYOUT_ENGINE.with(|e| e.borrow_mut().measure_text_lines(lines))
}

/// Find the position `(x, y)` of the first glyph belonging to the node with
/// `target_id` within the text block.
pub fn measure_text_node_pos(lines: &[TextChild], target_id: u64) -> Option<(f32, f32)> {
    LAYOUT_ENGINE.with(|e| e.borrow_mut().find_text_node_pos(lines, target_id))
}

/// Concatenates span texts into a single string, returning byte ranges per span.
///
/// A ZWNJ (U+200C) is inserted between adjacent spans to prevent the OpenType
/// shaper from forming ligatures across span boundaries (e.g. an "fi" ligature
/// spanning two differently-coloured spans).  The ZWNJ bytes are included in
/// the preceding span's byte range so that syntax-highlight offset arithmetic
/// in the renderer stays consistent.
pub fn build_span_text(spans: &[&TextSpan]) -> (String, Vec<(std::ops::Range<usize>, usize)>) {
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

pub fn collect_spans<'a>(child: &'a TextChild, out: &mut Vec<&'a TextSpan>) {
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
pub fn collect_spans_tagged<'a>(
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
pub fn make_line_key(spans: &[&TextSpan]) -> glyph_cache::LineKey {
    spans
        .iter()
        .map(|s| glyph_cache::SpanKey {
            text: s.text.clone(),
            font_family: s.text_style.font_family.value().clone(),
            font_size_bits: (*s.text_style.font_size.value() as f32).to_bits(),
            font_weight_bits: (*s.text_style.font_weight.value() as f32).to_bits(),
            italic: *s.text_style.italic.value(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
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
