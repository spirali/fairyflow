use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::scene::{NodeKind, PathCommand, Position, Scene, SceneNode, Style};

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
    match &node.kind {
        NodeKind::Node { position, size: _, alpha, scale_x, scale_y, rotation, children } => {
            let transform = positional_transform(position, *scale_x, *scale_y, *rotation, parent_transform);
            render_children(children, pixmap, transform, parent_alpha * *alpha as f32);
        }
        NodeKind::Rect { position, size, style } => {
            let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
            let Some(rect) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32) else { return };
            let path = PathBuilder::from_rect(rect);
            fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
        }
        NodeKind::Ellipse { position, size, style } => {
            let transform = positional_transform(position, 1.0, 1.0, 0.0, parent_transform);
            let Some(oval) = Rect::from_xywh(0.0, 0.0, size.width as f32, size.height as f32) else { return };
            let Some(path) = PathBuilder::from_oval(oval) else { return };
            fill_and_stroke(&path, style, pixmap, transform, parent_alpha);
        }
        NodeKind::Path { style, children } => {
            if let Some(path) = build_path(children) {
                fill_and_stroke(&path, style, pixmap, parent_transform, parent_alpha);
            }
        }
    }
}

fn positional_transform(position: &Position, scale_x: f64, scale_y: f64, rotation: f64, parent: Transform) -> Transform {
    // Order: Scale → Rotate → Translate, so the node's position is stable under scale/rotation.
    Transform::from_scale(scale_x as f32, scale_y as f32)
        .post_rotate(rotation as f32)
        .post_translate(position.x as f32, position.y as f32)
        .post_concat(parent)
}

fn build_path(commands: &[PathCommand]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let mut cur = (0.0f32, 0.0f32);
    for cmd in commands {
        match cmd {
            PathCommand::Move { position } => {
                cur = (position.x as f32, position.y as f32);
                pb.move_to(cur.0, cur.1);
            }
            PathCommand::Line { position } => {
                cur = (position.x as f32, position.y as f32);
                pb.line_to(cur.0, cur.1);
            }
            PathCommand::Cubic { position, c1_x, c1_y, c2_x, c2_y } => {
                let end = (position.x as f32, position.y as f32);
                // c1 is relative to the start (cur), c2 is relative to the end point
                let c1 = (cur.0 + *c1_x as f32, cur.1 + *c1_y as f32);
                let c2 = (end.0 + *c2_x as f32, end.1 + *c2_y as f32);
                pb.cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
                cur = end;
            }
        }
    }
    pb.finish()
}

fn fill_and_stroke(path: &tiny_skia::Path, style: &Style, pixmap: &mut Pixmap, transform: Transform, parent_alpha: f32) {
    let alpha = parent_alpha * style.alpha as f32;
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
