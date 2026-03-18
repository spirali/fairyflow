use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::scene::{NodeKind, Scene, SceneNode, Style};

pub fn render_scene(scene: &Scene, scale: f32) -> Pixmap {
    let width = (scene.width as f32 * scale).round() as u32;
    let height = (scene.height as f32 * scale).round() as u32;
    let mut pixmap = Pixmap::new(width.max(1), height.max(1)).expect("invalid scene dimensions");
    pixmap.fill(parse_color(&scene.fill_color));
    render_children(&scene.children, &mut pixmap, Transform::from_scale(scale, scale), 1.0);
    pixmap
}

fn render_children(nodes: &[SceneNode], pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
    for node in nodes {
        render_node(node, pixmap, parent_transform, parent_alpha);
    }
}

fn render_node(node: &SceneNode, pixmap: &mut Pixmap, parent_transform: Transform, parent_alpha: f32) {
    let transform = local_transform(node, parent_transform);
    let alpha = parent_alpha * node.alpha as f32;
    match &node.kind {
        NodeKind::Node { children } => render_children(children, pixmap, transform, alpha),
        NodeKind::Rect { style } => {
            let Some(rect) = Rect::from_xywh(0.0, 0.0, node.width as f32, node.height as f32) else { return };
            let path = PathBuilder::from_rect(rect);
            fill_and_stroke(&path, style, pixmap, transform, alpha);
        }
        NodeKind::Ellipse { style } => {
            let Some(oval) = Rect::from_xywh(0.0, 0.0, node.width as f32, node.height as f32) else { return };
            let Some(path) = PathBuilder::from_oval(oval) else { return };
            fill_and_stroke(&path, style, pixmap, transform, alpha);
        }
    }
}

/// Builds the accumulated transform for a node: parent → translate → scale → rotate.
fn local_transform(node: &SceneNode, parent: Transform) -> Transform {
    Transform::from_translate(node.x as f32, node.y as f32)
        .post_scale(node.scale as f32, node.scale as f32)
        .post_rotate(node.rotation as f32)
        .post_concat(parent)
}

fn fill_and_stroke(path: &tiny_skia::Path, style: &Style, pixmap: &mut Pixmap, transform: Transform, alpha: f32) {
    if let Some(ref fc) = style.fill_color {
        let mut color = parse_color(fc);
        color.set_alpha(color.alpha() * alpha);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        pixmap.fill_path(path, &paint, FillRule::Winding, transform, None);
    }
    if let Some(ref sc) = style.stroke_color {
        let mut color = parse_color(sc);
        color.set_alpha(color.alpha() * alpha);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let stroke = Stroke { width: style.stroke_width as f32, ..Default::default() };
        pixmap.stroke_path(path, &paint, &stroke, transform, None);
    }
}

fn parse_color(s: &str) -> tiny_skia::Color {
    let c = csscolorparser::parse(s)
        .unwrap_or_else(|_| csscolorparser::Color::from([0.0, 0.0, 0.0, 1.0]));
    tiny_skia::Color::from_rgba(c.r as f32, c.g as f32, c.b as f32, c.a as f32)
        .unwrap_or(tiny_skia::Color::BLACK)
}
