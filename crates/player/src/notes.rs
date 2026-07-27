use renderer_skia::{
    Camera, Color, Inheritable, Node, NodeBox, NodeKind, Paint, Position, Scene, Size, TextAlign,
    TextChild, TextSpan, TextStyle, measure_text,
};
use std::sync::Arc;

/// Speaker notes attach to the segment starting at their cue (or scene
/// start) up to *and including* the next cue (or scene end) — same rule the
/// web presenter panel uses (`web/src/notes.ts`). A cue only arms a lazy
/// one-frame advance — content authored after `cue()` doesn't visually
/// change until the frame after it, and the player pauses playback AT the
/// cue frame — so while paused there, the notes still showing should be the
/// ones for the segment that just played, not the one that hasn't started
/// yet. Only once the frame moves past the cue does the next segment's
/// notes take over.
///
/// Segment membership is tracked as an explicit ordinal (count of distinct
/// cue frames placed before the note), not inferred from frame number: a
/// note recorded right after `cue()` lands on the cue's own frame, so a
/// note from the *previous* segment can share that exact frame number and
/// would be indistinguishable by a frame-range check alone. `cues` need not
/// be pre-sorted.
pub fn notes_for_segment<'a>(
    cues: &[u32],
    notes: &'a [(u32, u32, String)],
    frame: u32,
) -> Vec<&'a str> {
    let current_segment = cues.iter().filter(|&&cf| cf < frame).count() as u32;
    notes
        .iter()
        .filter(|(_, seg, _)| *seg == current_segment)
        .map(|(_, _, text)| text.as_str())
        .collect()
}

const PADDING: f64 = 16.0;
const FONT_SIZE: f64 = 20.0;

fn notes_text_style() -> TextStyle {
    TextStyle {
        fill_color: Inheritable::Own(Paint::Solid(Color::from_rgba8(235, 235, 235, 255))),
        stroke_color: Inheritable::Own(Color::from_rgba8(0, 0, 0, 0)),
        stroke_width: Inheritable::Own(0.0),
        alpha: Inheritable::Own(1.0),
        font_family: Inheritable::Own(Arc::new("sans-serif".to_string())),
        font_size: Inheritable::Own(FONT_SIZE),
        font_weight: Inheritable::Own(400.0),
        italic: Inheritable::Own(false),
        underline: Inheritable::Own(0.0),
        strike: Inheritable::Own(0.0),
    }
}

/// Builds a small self-contained `Scene` rendering `paragraphs` as stacked
/// text, reusing the normal text-rendering pipeline (`render_scene_fitted`)
/// rather than needing separate UI-chrome font rasterization. Sized exactly
/// `width × height` — callers composite it into a strip via
/// `renderer_skia::render_scene_with_strip_to_buffer`.
pub fn build_notes_scene(paragraphs: &[&str], width: u32, height: u32) -> Scene {
    let (w, h) = (width as f64, height as f64);
    let lines: Vec<TextChild> = paragraphs
        .iter()
        .enumerate()
        .map(|(i, text)| {
            TextChild::Span(TextSpan {
                id: i as u64 + 1,
                text: Arc::new(text.to_string()),
                text_style: notes_text_style(),
                underline_color: None,
                underline_width: None,
                underline_offset: None,
                strike_color: None,
                strike_width: None,
                strike_offset: None,
                override_offset: None,
                override_transform: None,
            })
        })
        .collect();

    // Text nodes are fit-scaled from their natural (post-wrap) size into
    // node_box.size (see renderer-skia's text_fit_scale) — sizing the box to
    // the whole strip instead of the actual text would stretch a few short
    // words up to fill it. Measure first, like the engine's own
    // auto_width/auto_height do, so the fit-scale stays a ~1.0 no-op.
    let wrap_width = (w - 2.0 * PADDING).max(1.0);
    let (nat_w, nat_h) = measure_text(&lines, Some(wrap_width as f32), TextAlign::Left);

    let text_node = Node {
        id: 0,
        kind: NodeKind::Text {
            node_box: NodeBox {
                position: Position {
                    x: PADDING,
                    y: PADDING,
                },
                size: Size {
                    width: (nat_w as f64).max(1.0),
                    height: (nat_h as f64).max(1.0),
                },
                z_level: Inheritable::Own(0.0),
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                pivot_x: 0.0,
                pivot_y: 0.0,
            },
            keep_aspect: true,
            wrap: Some(wrap_width),
            text_align: TextAlign::Left,
            text_style: notes_text_style(),
            underline_color: None,
            underline_width: None,
            underline_offset: None,
            strike_color: None,
            strike_width: None,
            strike_offset: None,
            sh_language: None,
            sh_theme: None,
            reveal: 1.0,
            lines,
        },
    };

    Scene {
        width: w,
        height: h,
        fill_color: Color::from_rgba8(20, 20, 20, 235),
        camera: Camera {
            camera_zoom: 1.0,
            camera_x: w / 2.0,
            camera_y: h / 2.0,
        },
        children: vec![text_node],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_for_segment_matches_worked_example() {
        // Worked example: cue at 1, notes at 0 (segment 0, before the cue)
        // and 1 (segment 1, after the cue). The cue frame itself still
        // shows segment 0 (the player pauses there, still displaying what
        // was authored before the cue) — segment 1 only takes over once
        // the frame moves past the cue.
        let cues = [1u32];
        let notes = [
            (0u32, 0u32, "first".to_string()),
            (1u32, 1u32, "second".to_string()),
        ];
        assert_eq!(notes_for_segment(&cues, &notes, 0), vec!["first"]);
        assert_eq!(notes_for_segment(&cues, &notes, 1), vec!["first"]);
        assert_eq!(notes_for_segment(&cues, &notes, 2), vec!["second"]);
    }

    #[test]
    fn notes_for_segment_empty_when_no_notes_in_range() {
        let cues = [5u32];
        let notes = [(0u32, 0u32, "early".to_string())];
        // Frame 5 is still segment 0 (paused at the cue, pre-cue segment
        // still showing) — query past the cue to land in the empty segment.
        assert!(notes_for_segment(&cues, &notes, 6).is_empty());
    }

    #[test]
    fn notes_for_segment_disambiguates_same_frame_notes_by_ordinal() {
        // Regression test for the reported bug: a note before cue() and the
        // notes right after it can all land on frame 0 (scene1.ffpy's exact
        // shape) — segment_ordinal, not frame number, must separate them.
        // Also the literal scene1.ffpy repro end-to-end: paused at the cue
        // (frame 0) shows the intro note; advancing past it (frame 1)
        // reveals the punchline notes.
        let cues = [0u32];
        let notes = [
            (0u32, 0u32, "Hello".to_string()),
            (0u32, 1u32, "Baf".to_string()),
            (0u32, 1u32, "Baf2".to_string()),
        ];
        assert_eq!(notes_for_segment(&cues, &notes, 0), vec!["Hello"]);
        assert_eq!(notes_for_segment(&cues, &notes, 1), vec!["Baf", "Baf2"]);
    }

    #[test]
    #[ignore = "manual visual check only, not part of CI — writes a PNG to /tmp"]
    fn dump_notes_scene_preview_png() {
        renderer_skia::Resources::init();
        let scene = build_notes_scene(
            &[
                "This is the intro segment. Say hello and introduce the topic before moving on to the demo.",
                "Second paragraph: mention the animation timing.",
            ],
            500,
            140,
        );
        let pixmap = renderer_skia::render_scene(&scene, 1.0);
        pixmap.save_png("/tmp/notes_preview.png").unwrap();
    }

    #[test]
    fn build_notes_scene_renders_non_trivial_pixels() {
        renderer_skia::Resources::init();
        let scene = build_notes_scene(&["A speaker note.", "Second paragraph."], 400, 120);
        let pixmap = renderer_skia::render_scene(&scene, 1.0);
        // Background is a distinct dark gray; text is light — confirm the
        // pixmap isn't just a flat fill (i.e. something actually drew text).
        let distinct_colors: std::collections::HashSet<_> = pixmap
            .pixels()
            .iter()
            .map(|p| (p.red(), p.green(), p.blue(), p.alpha()))
            .collect();
        assert!(
            distinct_colors.len() > 2,
            "expected text glyphs to introduce more than background+transparent colors"
        );
    }

    #[test]
    fn build_notes_scene_sizes_text_box_to_content_not_the_strip() {
        // Regression test: node_box.size must track the text's own natural
        // extent, not the (much larger) strip box — otherwise Text's fit-scale
        // stretches a couple of short words up to fill the whole strip
        // (anisotropically, since keep_aspect was false), producing giant,
        // distorted, overlapping glyphs.
        renderer_skia::Resources::init();
        let paragraphs = ["Baf", "Baf2"];
        let strip_w = 1250;
        let strip_h = 130;
        let scene = build_notes_scene(&paragraphs, strip_w, strip_h);
        let NodeKind::Text { node_box, .. } = &scene.children[0].kind else {
            panic!("expected a Text node");
        };

        let wrap_width = strip_w as f64 - 2.0 * PADDING;
        let lines: Vec<TextChild> = paragraphs
            .iter()
            .enumerate()
            .map(|(i, text)| {
                TextChild::Span(TextSpan {
                    id: i as u64 + 1,
                    text: Arc::new(text.to_string()),
                    text_style: notes_text_style(),
                    underline_color: None,
                    underline_width: None,
                    underline_offset: None,
                    strike_color: None,
                    strike_width: None,
                    strike_offset: None,
                    override_offset: None,
                    override_transform: None,
                })
            })
            .collect();
        let (nat_w, nat_h) = measure_text(&lines, Some(wrap_width as f32), TextAlign::Left);

        assert!(
            (node_box.size.width - nat_w as f64).abs() < 0.01,
            "node_box width {} should match the text's natural width {nat_w}, not the strip",
            node_box.size.width
        );
        assert!(
            (node_box.size.height - nat_h as f64).abs() < 0.01,
            "node_box height {} should match the text's natural height {nat_h}, not the strip",
            node_box.size.height
        );
        // Sanity check this is actually a meaningfully smaller box than the
        // strip — otherwise the assertions above wouldn't distinguish the fix
        // from the old (buggy) behavior.
        assert!(node_box.size.width < 200.0, "two short words shouldn't need 200px");
        assert!(node_box.size.height < 100.0, "two short lines shouldn't need 100px");
    }
}
