use tiny_skia::{Paint, Pixmap, Rect, Transform};

use crate::scene::{NodeKind, Scene, SceneNode, Style};

pub fn render_scene(scene: &Scene) -> Pixmap {
    let width = scene.width as u32;
    let height = scene.height as u32;
    let mut pixmap = Pixmap::new(width, height).expect("invalid scene dimensions");
    pixmap.fill(parse_color(&scene.fill_color));
    render_children(&scene.children, &mut pixmap, Transform::identity());
    pixmap
}

fn render_children(nodes: &[SceneNode], pixmap: &mut Pixmap, parent_transform: Transform) {
    for node in nodes {
        render_node(node, pixmap, parent_transform);
    }
}

fn render_node(node: &SceneNode, pixmap: &mut Pixmap, parent_transform: Transform) {
    let transform = local_transform(node, parent_transform);
    match &node.kind {
        NodeKind::Node { children } => render_children(children, pixmap, transform),
        NodeKind::Rect { style } => render_rect(node, style, pixmap, transform),
    }
}

/// Builds the accumulated transform for a node: parent → translate → scale → rotate.
fn local_transform(node: &SceneNode, parent: Transform) -> Transform {
    Transform::from_translate(node.x as f32, node.y as f32)
        .post_scale(node.scale as f32, node.scale as f32)
        .post_rotate(node.rotation as f32)
        .post_concat(parent)
}

/// Draws a rect at local origin (0, 0); the transform positions it in the pixmap.
fn render_rect(node: &SceneNode, style: &Style, pixmap: &mut Pixmap, transform: Transform) {
    let Some(rect) = Rect::from_xywh(0.0, 0.0, node.width as f32, node.height as f32) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(parse_color(&style.fill_color));
    paint.anti_alias = true;
    pixmap.fill_rect(rect, &paint, transform, None);
}

fn parse_color(s: &str) -> tiny_skia::Color {
    let c = csscolorparser::parse(s)
        .unwrap_or_else(|_| csscolorparser::Color::from([0.0, 0.0, 0.0, 1.0]));
    tiny_skia::Color::from_rgba(c.r as f32, c.g as f32, c.b as f32, c.a as f32)
        .unwrap_or(tiny_skia::Color::BLACK)
}
