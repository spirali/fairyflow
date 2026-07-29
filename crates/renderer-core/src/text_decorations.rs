//! `underline()`/`strike()` decoration geometry. Pure,
//! backend-independent: computed fresh per render call from data already in
//! scope during normal glyph painting (`cached.glyphs` + the `spans` slice +
//! the per-span font metrics `text_layout.rs` captures for free while a
//! span's font is already resolved) — no glyph-cache shape change beyond
//! that metrics capture. Both `renderer-skia` and `renderer-pdf` call
//! [`decoration_rects`] identically and only duplicate the small
//! backend-specific "turn a rect into a filled path" step, the same split
//! `raw_local_transform` already establishes for glyph painting.

use crate::glyph_cache::{CachedGlyph, CachedLine};
use crate::{Color, TextSpan};

/// Which decoration to compute rects for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationKind {
    Underline,
    Strike,
}

/// One visible (already progress-clipped) decoration rect, in the same
/// local, pre-transform coordinate space `CachedGlyph` positions use (0 at
/// the `CachedLine`'s own top-left). Callers compose it with the same
/// per-run transform + `y_cursor` translate glyph paths already go through
/// — `span_idx` is provided so a caller can look up that span (for alpha,
/// `override_offset`/`override_transform`, etc.), the same index
/// `CachedGlyph::span_idx` already uses.
#[derive(Debug, Clone)]
pub struct DecorationRect {
    pub span_idx: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub thickness: f32,
    pub color: Color,
}

/// One contiguous run of glyphs sharing (span_idx, row) within a `CachedLine`,
/// in reading order.
struct RunSegment {
    span_idx: usize,
    /// This row's baseline, in the same space as `CachedGlyph::y`/`x`.
    baseline_y: f32,
    x_start: f32,
    x_end: f32,
}

/// Group `glyphs` (already in visual/reading order — see `text_layout.rs`)
/// into contiguous same-`(span_idx, row)` runs. A run's end x is the next
/// glyph's start x, which is exact for every boundary except the very last
/// glyph on a row (no next glyph to bound it): that one falls back to its
/// own ink bounds via the glyph's path. Two consequences of that fallback,
/// both acceptable for a decoration line rather than a text-selection
/// highlight: a run ending mid-row on a normal glyph may undershoot the true
/// advance-based edge by a small amount (ink bounds vs. advance width), and
/// a run whose last character is invisible (e.g. trailing whitespace at the
/// very end of a wrapped line) contributes no extra width there.
fn glyph_run_segments(glyphs: &[CachedGlyph]) -> Vec<RunSegment> {
    let mut segments: Vec<RunSegment> = Vec::new();
    for (i, glyph) in glyphs.iter().enumerate() {
        let same_run = segments.last().is_some_and(|s: &RunSegment| {
            s.span_idx == glyph.span_idx && s.baseline_y == glyph.baseline_y
        });
        if same_run {
            let seg = segments.last_mut().unwrap();
            seg.x_end = glyph.x;
        } else {
            segments.push(RunSegment {
                span_idx: glyph.span_idx,
                baseline_y: glyph.baseline_y,
                x_start: glyph.x,
                x_end: glyph.x,
            });
        }
        let is_last_on_row = glyphs
            .get(i + 1)
            .is_none_or(|next| next.baseline_y != glyph.baseline_y);
        if is_last_on_row {
            let seg = segments.last_mut().unwrap();
            let ink_width = crate::path_utils::verbs_bounds(&glyph.path.verbs)
                .map(|(min_x, _, max_x, _)| (max_x - min_x).max(0.0))
                .unwrap_or(0.0);
            seg.x_end = glyph.x + ink_width;
        }
    }
    segments
}

/// Compute the visible decoration rects for one `CachedLine`, given the
/// resolved spans it was built from. Only spans with a resolved progress
/// `> 0` for `kind` (`span.text_style.underline`/`.strike`) produce
/// anything. Progress is resolved **per span_idx independently** — the
/// total length a progress fraction sweeps across is that one span's own
/// segments on this line (covering the "wrapped across several visual rows"
/// case the proposal calls out), not pooled across sibling spans under a
/// `TextGroup`. `color`/`width`/`offset` fall back to the span's own fill
/// and this line's captured font metrics (`cached.decorations`) when the
/// span didn't set its own.
pub fn decoration_rects(
    cached: &CachedLine,
    spans: &[&TextSpan],
    kind: DecorationKind,
) -> Vec<DecorationRect> {
    let segments = glyph_run_segments(&cached.glyphs);
    let mut rects = Vec::new();

    let mut i = 0;
    while i < segments.len() {
        let span_idx = segments[i].span_idx;
        // All of this span's segments on this line — spans never
        // interleave (each glyph belongs to exactly one span_idx run at a
        // time), but the *same* span_idx can recur non-contiguously if
        // wrapping put it on more than one row, so gather by value, not by
        // a single contiguous slice.
        let span_segments: Vec<&RunSegment> =
            segments.iter().filter(|s| s.span_idx == span_idx).collect();
        // Skip every segment index belonging to this span in the outer
        // loop — they're all consumed by span_segments above.
        i += segments[i..]
            .iter()
            .take_while(|s| s.span_idx == span_idx)
            .count();

        let Some(span) = spans.get(span_idx) else {
            continue;
        };
        let progress = match kind {
            DecorationKind::Underline => *span.text_style.underline.value(),
            DecorationKind::Strike => *span.text_style.strike.value(),
        };
        if progress <= 0.0 {
            continue;
        }
        let progress = progress.min(1.0) as f32;

        let metrics = cached.decorations.get(span_idx).copied().flatten();
        let (default_offset, default_thickness) = match kind {
            DecorationKind::Underline => (
                metrics.map(|m| -m.underline_offset).unwrap_or(1.0),
                metrics.map(|m| m.underline_thickness).unwrap_or(1.0),
            ),
            DecorationKind::Strike => (
                metrics.map(|m| m.strikeout_offset).unwrap_or(1.0),
                metrics.map(|m| m.strikeout_thickness).unwrap_or(1.0),
            ),
        };
        let (explicit_color, explicit_width, explicit_offset) = match kind {
            DecorationKind::Underline => (
                span.underline_color.as_ref(),
                span.underline_width,
                span.underline_offset,
            ),
            DecorationKind::Strike => (
                span.strike_color.as_ref(),
                span.strike_width,
                span.strike_offset,
            ),
        };
        let color = explicit_color
            .map(|p| p.solid_or_first_stop().clone())
            .unwrap_or_else(|| {
                span.text_style
                    .fill_color
                    .value()
                    .solid_or_first_stop()
                    .clone()
            });
        let thickness = explicit_width
            .map(|w| w as f32)
            .unwrap_or(default_thickness);
        let offset = explicit_offset.map(|o| o as f32).unwrap_or(default_offset);

        let total: f32 = span_segments.iter().map(|s| s.x_end - s.x_start).sum();
        if total <= 0.0 {
            continue;
        }
        let cutoff = progress * total;
        let mut drawn = 0.0f32;
        for seg in span_segments {
            let seg_len = seg.x_end - seg.x_start;
            if drawn >= cutoff {
                break;
            }
            let visible_len = (cutoff - drawn).min(seg_len);
            if visible_len > 0.0 {
                let sign = match kind {
                    DecorationKind::Underline => 1.0,
                    DecorationKind::Strike => -1.0,
                };
                rects.push(DecorationRect {
                    span_idx,
                    x: seg.x_start,
                    y: seg.baseline_y + sign * offset - thickness / 2.0,
                    width: visible_len,
                    thickness,
                    color: color.clone(),
                });
            }
            drawn += seg_len;
        }
    }

    rects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_layout::TextLayoutEngine;
    use crate::text_layout::tests::{init_test_resources, span};
    use crate::{Inheritable, Resources, TextAlign, TextChild};

    fn span_with_progress(id: u64, text: &str, underline: f64, strike: f64) -> TextSpan {
        let mut sp = span(id, text);
        sp.text_style.underline = Inheritable::Own(underline);
        sp.text_style.strike = Inheritable::Own(strike);
        sp
    }

    #[test]
    fn no_decoration_when_progress_is_zero() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let sp = span_with_progress(1, "plain text", 0.0, 0.0);
        let lines = [TextChild::Span(sp.clone())];
        let laid_out = engine.layout_text(&lines, None, TextAlign::Left);
        let spans = vec![&sp];
        assert!(decoration_rects(&laid_out.lines[0], &spans, DecorationKind::Underline).is_empty());
        assert!(decoration_rects(&laid_out.lines[0], &spans, DecorationKind::Strike).is_empty());
    }

    #[test]
    fn full_progress_covers_the_whole_run() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let sp = span_with_progress(1, "Hello underline", 1.0, 0.0);
        let lines = [TextChild::Span(sp.clone())];
        let laid_out = engine.layout_text(&lines, None, TextAlign::Left);
        let cached = &laid_out.lines[0];
        let spans = vec![&sp];
        let rects = decoration_rects(cached, &spans, DecorationKind::Underline);
        assert_eq!(rects.len(), 1, "one contiguous run -> one rect");
        let rect = &rects[0];
        assert_eq!(rect.x, 0.0, "run starts at the line's own left edge");
        // The last glyph's end-x falls back to ink bounds rather than true
        // advance width (documented approximation) -- it can only ever
        // *undershoot* the measured line width, by up to roughly one
        // glyph's side-bearing (a proportionally larger slice of a short
        // run like this one).
        assert!(
            rect.width <= cached.width,
            "rect width {} must never exceed the line width {}",
            rect.width,
            cached.width
        );
        assert!(
            rect.width > cached.width * 0.85,
            "rect width {} should still approximate the line width {} reasonably closely",
            rect.width,
            cached.width
        );
        assert!(rect.thickness > 0.0);
        // underline sits *below* the baseline (larger y = further down).
        let first_glyph_baseline = cached.glyphs[0].baseline_y;
        assert!(rect.y > first_glyph_baseline);
    }

    #[test]
    fn strike_sits_above_the_baseline() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let sp = span_with_progress(1, "crossed out", 0.0, 1.0);
        let lines = [TextChild::Span(sp.clone())];
        let laid_out = engine.layout_text(&lines, None, TextAlign::Left);
        let cached = &laid_out.lines[0];
        let spans = vec![&sp];
        let rects = decoration_rects(cached, &spans, DecorationKind::Strike);
        assert_eq!(rects.len(), 1);
        let first_glyph_baseline = cached.glyphs[0].baseline_y;
        assert!(rects[0].y + rects[0].thickness < first_glyph_baseline);
    }

    #[test]
    fn partial_progress_covers_strictly_less_than_full_progress() {
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let full = span_with_progress(1, "sweeping in", 1.0, 0.0);
        let half = span_with_progress(1, "sweeping in", 0.5, 0.0);

        let full_lines = [TextChild::Span(full.clone())];
        let full_laid_out = engine.layout_text(&full_lines, None, TextAlign::Left);
        let full_spans = vec![&full];
        let full_rects = decoration_rects(
            &full_laid_out.lines[0],
            &full_spans,
            DecorationKind::Underline,
        );
        let full_width: f32 = full_rects.iter().map(|r| r.width).sum();

        let half_lines = [TextChild::Span(half.clone())];
        let half_laid_out = engine.layout_text(&half_lines, None, TextAlign::Left);
        let half_spans = vec![&half];
        let half_rects = decoration_rects(
            &half_laid_out.lines[0],
            &half_spans,
            DecorationKind::Underline,
        );
        let half_width: f32 = half_rects.iter().map(|r| r.width).sum();

        assert!(half_width > 0.0);
        assert!(
            half_width < full_width,
            "half progress ({half_width}) should draw strictly less than full progress ({full_width})"
        );
        // Not just "less than" -- roughly half, within the ink-bounds slop
        // the last-glyph fallback introduces.
        assert!(
            (half_width - full_width / 2.0).abs() < 3.0,
            "half_width {half_width} should be close to half of full_width {full_width}"
        );
    }

    #[test]
    fn wrapped_run_sweeps_continuously_across_rows() {
        // A single long span forced to wrap across two visual rows within
        // one CachedLine -- the exact case the proposal calls out ("a run
        // wrapped over three lines draws in continuously").
        init_test_resources();
        let mut engine = TextLayoutEngine::new(Resources::get());
        let sp = span_with_progress(1, "one two three four five six seven", 1.0, 0.0);
        let lines = [TextChild::Span(sp.clone())];
        // Narrow wrap width forces multiple visual rows.
        let laid_out = engine.layout_text(&lines, Some(60.0), TextAlign::Left);
        let cached = &laid_out.lines[0];
        let distinct_rows = cached
            .glyphs
            .iter()
            .map(|g| g.baseline_y.to_bits())
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert!(
            distinct_rows > 1,
            "expected the span to wrap onto multiple rows"
        );

        let spans = vec![&sp];
        let rects = decoration_rects(cached, &spans, DecorationKind::Underline);
        assert_eq!(
            rects.len(),
            distinct_rows,
            "one continuous rect per row the run touches"
        );
        // Every row must be at least partially covered at full progress --
        // a bug that stopped accumulating after the first row would leave
        // later rows with a zero-width (i.e. absent) rect.
        let covered_rows = rects
            .iter()
            .map(|r| r.y.to_bits())
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(covered_rows, distinct_rows);
    }
}
