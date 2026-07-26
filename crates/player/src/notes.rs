use renderer_skia::{
    Camera, Color, Inheritable, Node, NodeBox, NodeKind, Paint, Position, Scene, Size, TextAlign,
    TextChild, TextSpan, TextStyle,
};
use std::sync::Arc;

/// Speaker notes attach to the segment starting at their frame (previous cue,
/// or scene start) up to the next cue (or scene end) — same rule the web
/// presenter panel uses (`web/src/notes.ts`). `cues` need not be pre-sorted.
pub fn notes_for_segment<'a>(
    cues: &[u32],
    notes: &'a [(u32, String)],
    frame: u32,
    frame_count: u32,
) -> Vec<&'a str> {
    let mut segment_start = 0u32;
    let mut segment_end = frame_count;
    for &cf in cues {
        if cf <= frame {
            segment_start = segment_start.max(cf);
        } else {
            segment_end = segment_end.min(cf);
        }
    }
    notes
        .iter()
        .filter(|(nf, _)| *nf >= segment_start && *nf < segment_end)
        .map(|(_, text)| text.as_str())
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

    let text_node = Node {
        id: 0,
        kind: NodeKind::Text {
            node_box: NodeBox {
                position: Position {
                    x: PADDING,
                    y: PADDING,
                },
                size: Size {
                    width: (w - 2.0 * PADDING).max(1.0),
                    height: (h - 2.0 * PADDING).max(1.0),
                },
                z_level: Inheritable::Own(0.0),
                scale_x: 1.0,
                scale_y: 1.0,
                rotation: 0.0,
                pivot_x: 0.0,
                pivot_y: 0.0,
            },
            keep_aspect: false,
            wrap: Some((w - 2.0 * PADDING).max(1.0)),
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
        // proposal §5.1 worked example: cue at 0 and 1, notes at 0 and 1.
        let cues = [1u32];
        let notes = [(0u32, "first".to_string()), (1u32, "second".to_string())];
        assert_eq!(notes_for_segment(&cues, &notes, 0, 2), vec!["first"]);
        assert_eq!(notes_for_segment(&cues, &notes, 1, 2), vec!["second"]);
    }

    #[test]
    fn notes_for_segment_empty_when_no_notes_in_range() {
        let cues = [5u32];
        let notes = [(0u32, "early".to_string())];
        assert!(notes_for_segment(&cues, &notes, 5, 10).is_empty());
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
}
