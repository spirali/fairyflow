use crate::glyph_cache::{self, CachedLine, PathVerb, VectorPath};
use crate::resources::Resources;
use crate::{TextAlign, TextChild, TextGroup, TextSpan};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontStack, FontWeight, LayoutContext,
    PositionedLayoutItem, StyleProperty,
    style::{FontFeature, FontSettings},
};
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{LocationRef, NormalizedCoord, Size as SkrifaSize},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef as ReadFontsRef,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

const DEFAULT_FONT_SIZE: f32 = 16.0;

/// An unregistered OpenType feature tag, pushed on every other span so that
/// parley's shaper ends its item at each span boundary (it breaks a run when
/// `font_features` differs between adjacent styles — see parley's
/// `shape::shape_text`). No font implements `zzzy`, so the shaper drops it:
/// the split is the only effect. This is what keeps a ligature from forming
/// across two spans (an "fi" spanning two differently-coloured runs would
/// otherwise be one glyph with one brush); spans are shaped independently,
/// so kerning/ligatures *within* a span are untouched.
const RUN_SPLIT_FEATURE: [FontFeature; 1] = [FontFeature {
    tag: u32::from_be_bytes(*b"zzzy"),
    value: 1,
}];

fn to_parley_alignment(align: TextAlign) -> Alignment {
    match align {
        TextAlign::Left => Alignment::Left,
        TextAlign::Center => Alignment::Center,
        TextAlign::Right => Alignment::Right,
        TextAlign::Justify => Alignment::Justify,
    }
}

/// A `Text` block's fully laid-out lines, ready to render or measure.
/// `lines` has one entry per logical `TextChild` in the input, in order.
pub struct LaidOutText {
    /// The block's intrinsic width — the widest (possibly wrapped) line.
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Arc<CachedLine>>,
}

/// Effective alignment for one logical line: its own `TextGroup.text_align`
/// override, or the block's default. A bare `TextSpan` line (no `TextGroup`
/// wrapper) has no override and always uses the block default.
fn line_align(line: &TextChild, default_align: TextAlign) -> TextAlign {
    match line {
        TextChild::Group(TextGroup { text_align, .. }) => text_align.unwrap_or(default_align),
        TextChild::Span(_) => default_align,
    }
}

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

    /// Lay out a whole `Text` block.
    ///
    /// Fast path: if every line resolves to `Left` alignment (the default —
    /// this is the common case), skip straight to the align+extract pass
    /// with a constant `alignment_width` of `0.0`. This is sound because
    /// parley's `align()` is a documented no-op for `Left` regardless of the
    /// width passed to it (verified against parley 0.7.0's
    /// `layout/alignment.rs`: the `Left` branch of `align_impl` is literally
    /// "do nothing"), and `align()` never touches `Layout::width()` (a
    /// separate field) — so each line's own cached `.width` is always its
    /// true natural width, giving us `block_width` for free instead of
    /// needing a separate measure pass first.
    ///
    /// Slow path: `Center`/`Right`/`Justify` alignment genuinely need the
    /// cross-line reference width (the widest line, possibly after wrap)
    /// *before* any individual line can be positioned, so those still pay a
    /// dedicated measure pass ahead of the align+extract pass.
    pub fn layout_text(
        &mut self,
        lines: &[TextChild],
        wrap: Option<f32>,
        default_align: TextAlign,
    ) -> LaidOutText {
        let all_left = lines
            .iter()
            .all(|line| line_align(line, default_align) == TextAlign::Left);

        if all_left {
            let mut block_width = 0.0f32;
            let mut total_height = 0.0f32;
            let mut cached_lines = Vec::with_capacity(lines.len());
            for line in lines {
                let mut spans: Vec<&TextSpan> = Vec::new();
                collect_spans(line, &mut spans);
                let cached = if spans.is_empty() {
                    Arc::new(CachedLine {
                        width: 0.0,
                        height: 0.0,
                        glyphs: vec![],
                        decorations: vec![],
                    })
                } else {
                    let key = make_line_key(&spans, wrap, TextAlign::Left, 0.0);
                    if let Some(cached) = glyph_cache::cache_get(&key) {
                        cached
                    } else {
                        self.build_cached_line(&spans, wrap, TextAlign::Left, 0.0, key)
                    }
                };
                block_width = block_width.max(cached.width);
                total_height += cached.height;
                cached_lines.push(cached);
            }
            return LaidOutText {
                width: block_width,
                height: total_height,
                lines: cached_lines,
            };
        }

        let mut block_width = 0.0f32;
        for line in lines {
            let mut spans: Vec<&TextSpan> = Vec::new();
            collect_spans(line, &mut spans);
            if spans.is_empty() {
                continue;
            }
            block_width = block_width.max(self.measure_natural_width(&spans, wrap));
        }

        let mut total_height = 0.0f32;
        let mut cached_lines = Vec::with_capacity(lines.len());
        for line in lines {
            let mut spans: Vec<&TextSpan> = Vec::new();
            collect_spans(line, &mut spans);
            let cached = if spans.is_empty() {
                Arc::new(CachedLine {
                    width: 0.0,
                    height: 0.0,
                    glyphs: vec![],
                    decorations: vec![],
                })
            } else {
                let align = line_align(line, default_align);
                let alignment_width = match align {
                    TextAlign::Justify => wrap.unwrap_or(block_width),
                    _ => block_width,
                };
                let key = make_line_key(&spans, wrap, align, alignment_width);
                if let Some(cached) = glyph_cache::cache_get(&key) {
                    cached
                } else {
                    self.build_cached_line(&spans, wrap, align, alignment_width, key)
                }
            };
            total_height += cached.height;
            cached_lines.push(cached);
        }

        LaidOutText {
            width: block_width,
            height: total_height,
            lines: cached_lines,
        }
    }

    /// The "measure pass": build + break (no alignment, no glyph extraction)
    /// and read the natural (possibly wrapped) width of one line.
    fn measure_natural_width(&mut self, spans: &[&TextSpan], wrap: Option<f32>) -> f32 {
        let (full_text, ranges) = build_span_text(spans);
        if full_text.is_empty() {
            return 0.0;
        }
        let mut layout = self.build_layout(&full_text, spans, &ranges);
        layout.break_all_lines(wrap);
        layout.width()
    }

    /// Shared parley builder setup for one line's spans — factored out since
    /// both the measure pass and the align+extract pass need it.
    fn build_layout(
        &mut self,
        full_text: &str,
        spans: &[&TextSpan],
        ranges: &[(Range<usize>, usize)],
    ) -> parley::Layout<usize> {
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, full_text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(DEFAULT_FONT_SIZE));
        for (range, span_idx) in ranges {
            let span = spans[*span_idx];
            builder.push(StyleProperty::Brush(*span_idx), range.clone());
            if span_idx % 2 == 1 {
                builder.push(
                    StyleProperty::FontFeatures(FontSettings::List(std::borrow::Cow::Borrowed(
                        &RUN_SPLIT_FEATURE,
                    ))),
                    range.clone(),
                );
            }
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
        builder.build(full_text)
    }

    /// Run parley layout for `spans`, extract glyph outlines into `CachedGlyph`s,
    /// store in the global cache, and return the entry.
    fn build_cached_line(
        &mut self,
        spans: &[&TextSpan],
        wrap: Option<f32>,
        align: TextAlign,
        alignment_width: f32,
        key: glyph_cache::LineKey,
    ) -> Arc<CachedLine> {
        let (full_text, ranges) = build_span_text(spans);
        if full_text.is_empty() {
            return Arc::new(CachedLine {
                width: 0.0,
                height: 0.0,
                glyphs: vec![],
                decorations: vec![],
            });
        }

        let mut layout = self.build_layout(&full_text, spans, &ranges);
        layout.break_all_lines(wrap);
        layout.align(
            Some(alignment_width),
            to_parley_alignment(align),
            AlignmentOptions::default(),
        );

        let width = layout.width();
        let height = layout.height();
        let mut glyphs = Vec::new();
        let mut decorations: Vec<Option<glyph_cache::DecorationMetrics>> = vec![None; spans.len()];

        for layout_line in layout.lines() {
            // Top of this visual row within the (possibly multi-row, once
            // wrapped) layout — 0 for the first row, matching the pre-wrap
            // behavior where a `CachedLine` was always exactly one row.
            // Distinct from a glyph's own baseline (`gy` below), which also
            // includes that row's ascent.
            let row_top = layout_line.metrics().min_coord;

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

                // A GlyphRun is inherently one physical font (font/script
                // changes are exactly what split parley's runs), so every
                // span_idx whose glyphs land in this run shares these
                // metrics — computed once per run, not per glyph.
                let run_metrics = font_ref.metrics(
                    SkrifaSize::new(font_size),
                    LocationRef::new(&normalized_coords),
                );
                let run_decoration_metrics = match (run_metrics.underline, run_metrics.strikeout) {
                    (Some(u), Some(s)) => Some(glyph_cache::DecorationMetrics {
                        underline_offset: u.offset,
                        underline_thickness: u.thickness,
                        strikeout_offset: s.offset,
                        strikeout_thickness: s.thickness,
                    }),
                    _ => None,
                };

                // Build a flat table: for each glyph index in this font run,
                // what is the cluster's byte offset in full_text?
                let cluster_bytes: Vec<u32> = run
                    .visual_clusters()
                    .flat_map(|cluster| {
                        let byte = cluster.text_range().start as u32;
                        let count = cluster.glyphs().count();
                        std::iter::repeat_n(byte, count)
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

                    if decorations[span_idx].is_none() {
                        decorations[span_idx] = run_decoration_metrics;
                    }

                    let gx = run_x + glyph.x;
                    let gy = baseline - glyph.y;
                    let advance = glyph.advance;
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
                        y: row_top,
                        baseline_y: baseline,
                        path: VectorPath { verbs: pen.verbs },
                        advance,
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
                decorations,
            },
        )
    }

    /// Find the top-left position of the first glyph belonging to `target_id`
    /// within the text block described by `lines`. `wrap`/`default_align` must
    /// match what the block is actually rendered with, or the returned
    /// position won't match the glyphs on screen.
    pub fn find_text_node_pos(
        &mut self,
        lines: &[TextChild],
        target_id: u64,
        wrap: Option<f32>,
        default_align: TextAlign,
    ) -> Option<(f32, f32)> {
        let laid_out = self.layout_text(lines, wrap, default_align);
        let mut y_offset = 0.0f32;
        for (line, cached) in lines.iter().zip(laid_out.lines.iter()) {
            let mut tagged: Vec<(bool, &TextSpan)> = Vec::new();
            collect_spans_tagged(line, target_id, false, &mut tagged);
            if tagged.is_empty() {
                continue;
            }

            let first_target_idx = tagged.iter().position(|(is_target, _)| *is_target);
            if let Some(target_span_idx) = first_target_idx
                && let Some(glyph) = cached.glyphs.iter().find(|g| g.span_idx == target_span_idx)
            {
                return Some((glyph.x, y_offset + glyph.y));
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

/// Lay out a whole `Text` block using the thread-local engine.
pub fn layout_text(
    lines: &[TextChild],
    wrap: Option<f32>,
    default_align: TextAlign,
) -> LaidOutText {
    LAYOUT_ENGINE.with(|e| e.borrow_mut().layout_text(lines, wrap, default_align))
}

/// Measure the natural (post-wrap) dimensions of a text block.
/// Returns `(width, height)` in logical pixels (scale = 1).
pub fn measure_text(
    lines: &[TextChild],
    wrap: Option<f32>,
    default_align: TextAlign,
) -> (f32, f32) {
    let laid_out = layout_text(lines, wrap, default_align);
    (laid_out.width, laid_out.height)
}

/// Find the position `(x, y)` of the first glyph belonging to the node with
/// `target_id` within the text block.
pub fn measure_text_node_pos(
    lines: &[TextChild],
    target_id: u64,
    wrap: Option<f32>,
    default_align: TextAlign,
) -> Option<(f32, f32)> {
    LAYOUT_ENGINE.with(|e| {
        e.borrow_mut()
            .find_text_node_pos(lines, target_id, wrap, default_align)
    })
}

/// Concatenates span texts into a single string, returning byte ranges per span.
///
/// Nothing is inserted between spans: cross-span ligatures are prevented by
/// `RUN_SPLIT_FEATURE` instead, which breaks the shaper's run at each boundary
/// without adding a character to the shaped text.
pub fn build_span_text(spans: &[&TextSpan]) -> (String, Vec<(std::ops::Range<usize>, usize)>) {
    let mut full_text = String::new();
    let mut ranges = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        let start = full_text.len();
        full_text.push_str(&span.text);
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

/// Build the normalized cache key for a built line: the spans plus the
/// wrap/alignment parameters used to lay them out.
pub fn make_line_key(
    spans: &[&TextSpan],
    wrap: Option<f32>,
    align: TextAlign,
    alignment_width: f32,
) -> glyph_cache::LineKey {
    glyph_cache::LineKey {
        spans: spans
            .iter()
            .map(|s| glyph_cache::SpanKey {
                text: s.text.clone(),
                font_family: s.text_style.font_family.value().clone(),
                font_size_bits: (*s.text_style.font_size.value() as f32).to_bits(),
                font_weight_bits: (*s.text_style.font_weight.value() as f32).to_bits(),
                italic: *s.text_style.italic.value(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        wrap_bits: wrap.map(f32::to_bits),
        align,
        alignment_width_bits: alignment_width.to_bits(),
    }
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{Color, Inheritable, Paint, TextStyle};

    pub(crate) fn init_test_resources() {
        Resources::init();
        let resources = Resources::get();
        let fonts_dir = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/assets/fonts"
        ));
        resources.load_font_directories(&[fonts_dir]);
        let mut aliases = HashMap::new();
        aliases.insert("sans-serif".to_string(), "DejaVu Sans".to_string());
        aliases.insert("monospace".to_string(), "DejaVu Sans Mono".to_string());
        resources.set_font_aliases(&aliases);
    }

    pub(crate) fn test_style() -> TextStyle {
        TextStyle {
            fill_color: Inheritable::Own(Paint::Solid(Color::from_rgba8(0, 0, 0, 255))),
            stroke_color: Inheritable::Own(Color::from_rgba8(0, 0, 0, 0)),
            stroke_width: Inheritable::Own(0.0),
            alpha: Inheritable::Own(1.0),
            font_family: Inheritable::Own(Arc::new("sans-serif".to_string())),
            font_size: Inheritable::Own(16.0),
            font_weight: Inheritable::Own(400.0),
            italic: Inheritable::Own(false),
            underline: Inheritable::Own(0.0),
            strike: Inheritable::Own(0.0),
        }
    }

    pub(crate) fn span(id: u64, text: &str) -> TextSpan {
        TextSpan {
            id,
            text: Arc::new(text.to_string()),
            text_style: test_style(),
            underline_color: None,
            underline_width: None,
            underline_offset: None,
            strike_color: None,
            strike_width: None,
            strike_offset: None,
            override_offset: None,
            override_transform: None,
        }
    }

    pub(crate) fn group(
        id: u64,
        children: Vec<TextChild>,
        text_align: Option<TextAlign>,
    ) -> TextGroup {
        TextGroup {
            id,
            text_style: test_style(),
            underline_color: None,
            underline_width: None,
            underline_offset: None,
            strike_color: None,
            strike_width: None,
            strike_offset: None,
            text_align,
            children,
        }
    }

    #[test]
    fn left_alignment_all_lines_start_at_zero_offset() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let lines = [
            TextChild::Span(span(1, "a")),
            TextChild::Span(span(2, "aaaaaaaaaa")),
        ];
        let laid_out = engine.layout_text(&lines, None, TextAlign::Left);
        assert_eq!(laid_out.lines.len(), 2);
        for line in &laid_out.lines {
            let first = line.glyphs.first().expect("line has glyphs");
            assert!(
                first.x.abs() < 0.01,
                "left-aligned line should start at x=0, got {}",
                first.x
            );
        }
    }

    #[test]
    fn center_and_right_alignment_use_the_cross_line_block_width() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let short_span = span(1, "a");
        let long_span = span(2, "aaaaaaaaaa");
        let short_width = engine.measure_natural_width(&[&short_span], None);
        let long_width = engine.measure_natural_width(&[&long_span], None);
        assert!(
            long_width > short_width,
            "sanity: repeating the character should make a wider line"
        );

        let lines = [TextChild::Span(short_span), TextChild::Span(long_span)];

        // `Text("a\naaaaaaaaaa").text_align("center")` centers the short
        // line against the long line's width, not its own.
        let centered = engine.layout_text(&lines, None, TextAlign::Center);
        assert!((centered.width - long_width).abs() < 0.5);
        let center_offset = centered.lines[0].glyphs[0].x;
        assert!((center_offset - (centered.width - short_width) / 2.0).abs() < 0.5);

        let right = engine.layout_text(&lines, None, TextAlign::Right);
        let right_offset = right.lines[0].glyphs[0].x;
        assert!((right_offset - (right.width - short_width)).abs() < 0.5);

        assert!(
            right_offset > center_offset,
            "right-aligning should push the short line further than centering it"
        );
    }

    #[test]
    fn justify_on_a_lone_line_degrades_to_left() {
        // Parley: "Justify each line by spacing out content, except for the
        // last line." A block with only one (therefore always-last) visual
        // row has nothing to justify against, wrap or no wrap.
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let lines = [TextChild::Span(span(1, "short line"))];

        let no_wrap = engine.layout_text(&lines, None, TextAlign::Justify);
        assert!(no_wrap.lines[0].glyphs[0].x.abs() < 0.5);

        let with_wrap = engine.layout_text(&lines, Some(1000.0), TextAlign::Justify);
        assert!(with_wrap.lines[0].glyphs[0].x.abs() < 0.5);
    }

    #[test]
    fn find_text_node_pos_accounts_for_wrapped_row_offset() {
        // Regression test for the multi-row bug: a span that lands on the
        // second (or later) visual row of a `wrap()`-forced line must get
        // that row's offset added, not just the top of the logical line
        // (which pre-fix was always what `find_text_node_pos` returned).
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let word1 = span(1, "wwwwwwwwww ");
        let target_id = 2;
        let word2 = span(target_id, "wwwwwwwwww");
        let word1_width = engine.measure_natural_width(&[&word1], None);
        let wrap = word1_width + 5.0;

        let the_group = group(
            3,
            vec![TextChild::Span(word1), TextChild::Span(word2)],
            None,
        );
        let lines = [TextChild::Group(the_group)];

        let laid_out = engine.layout_text(&lines, Some(wrap), TextAlign::Left);
        assert_eq!(laid_out.lines.len(), 1);
        let block_height = laid_out.lines[0].height;
        assert!(block_height > 0.0);

        let (_, y) = engine
            .find_text_node_pos(&lines, target_id, Some(wrap), TextAlign::Left)
            .expect("target span should be found");
        assert!(
            y > 0.0,
            "target span wrapped to row 2, so its y must be > 0 \
             (pre-fix, this always returned 0)"
        );
        assert!(
            y < block_height,
            "row offset must stay within the block's total height"
        );
    }

    fn mono_span(id: u64, text: &str) -> TextSpan {
        let mut span = span(id, text);
        span.text_style.font_family = Inheritable::Own(Arc::new("monospace".to_string()));
        span
    }

    #[test]
    fn a_span_boundary_leaves_the_glyphs_untouched() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let joined = [TextChild::Span(mono_span(1, "888"))];
        let split = [TextChild::Group(group(
            2,
            vec![
                TextChild::Span(mono_span(3, "88")),
                TextChild::Span(mono_span(4, "8")),
            ],
            None,
        ))];

        let one = engine.layout_text(&joined, None, TextAlign::Left);
        let two = engine.layout_text(&split, None, TextAlign::Left);

        // DejaVu Sans Mono has no U+200C, so a separator character between the
        // spans costs a glyph of its own *and* drags the digit in front of it
        // into a fallback face along with it (the two form one cluster) — the
        // middle digit of `stext("88<s></s>8")` came out slightly bigger than
        // its neighbours.
        assert_eq!(one.width, two.width, "a span boundary changed the width");
        let (a, b) = (&one.lines[0].glyphs, &two.lines[0].glyphs);
        assert_eq!(a.len(), b.len(), "a span boundary added a glyph");
        for (i, (ga, gb)) in a.iter().zip(b).enumerate() {
            assert_eq!(ga.x, gb.x, "glyph {i} moved");
            assert_eq!(ga.advance, gb.advance, "glyph {i} changed advance");
            assert_eq!(ga.path.verbs, gb.path.verbs, "glyph {i} changed outline");
        }
    }

    #[test]
    fn a_ligature_never_forms_across_a_span_boundary() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        // DejaVu Sans shapes "fi" into a single ligature glyph. Split over two
        // spans it has to stay two glyphs — one ligature carrying two spans'
        // brushes could only ever be painted in one of their colours. This is
        // what `RUN_SPLIT_FEATURE` buys, and the reason the spans can't simply
        // be concatenated.
        let joined = [TextChild::Span(span(1, "fi"))];
        let split = [TextChild::Group(group(
            2,
            vec![TextChild::Span(span(3, "f")), TextChild::Span(span(4, "i"))],
            None,
        ))];

        let one = engine.layout_text(&joined, None, TextAlign::Left);
        let two = engine.layout_text(&split, None, TextAlign::Left);
        assert_eq!(one.lines[0].glyphs.len(), 1, "expected an fi ligature");
        assert_eq!(
            two.lines[0].glyphs.len(),
            2,
            "expected one glyph per span: a ligature crossing the boundary would \
             give 1, a separator character inserted between them 3"
        );
    }
}
