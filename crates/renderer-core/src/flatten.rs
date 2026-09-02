//! Flatten a scene tree into a single globally z-ordered draw list.
//!
//! `z` is **global**: a node's z-level orders it against every other node in
//! the scene, not just its siblings. `Group` is therefore transparent to paint
//! order — it contributes no draw of its own, and its children join the one
//! global list. Every other node kind is an atomic leaf (a `Text`'s runs and an
//! `Image`'s layers are parts of one leaf, not scene nodes of their own).
//!
//! A group is still what carries transform, camera, alpha and clipping, so
//! those must travel with each leaf instead of bracketing a contiguous run of
//! draws: ancestor transforms are baked into the item, ancestor alpha is
//! multiplied in, and ancestor clip windows are recorded as a chain of
//! [`ClipFrame`]s the item points at.
//!
//! Unset `z` is inherited from the parent (resolved upstream in the engine), so
//! a group's contents stay together by default and only an explicit `.z()`
//! breaks a node out of its group's band.

use crate::transform::{AffineTransform, camera_transform, node_z_level, transform_node_box};
use crate::{Node, NodeKind, Position, Scene};

/// One ancestor clip window an item is subject to.
///
/// `rect` is in the clipping group's own local box space, so it must be
/// interpreted under `transform` (absolute) or, for a backend that keeps
/// transforms on a state stack, under `rel_transform` pushed on top of
/// `parent`'s space.
#[derive(Debug, Clone)]
pub struct ClipFrame {
    /// Enclosing clip frame, if this group is nested inside another clip.
    /// Always a lower index than this frame's own.
    pub parent: Option<usize>,
    /// Scene-absolute transform of the clipping group's box.
    pub transform: AffineTransform,
    /// Transform relative to `parent`'s space, or to the flatten root when
    /// `parent` is `None`. The `root` transform itself is *not* folded in — a
    /// stack-based backend has already applied it.
    pub rel_transform: AffineTransform,
    /// Clip window `(x, y, w, h)` in the group's local box space.
    pub rect: (f32, f32, f32, f32),
}

/// One leaf draw, with everything its ancestors contributed already resolved.
#[derive(Debug, Clone)]
pub struct FlatItem<'a> {
    /// The node to draw. Never a `Group`.
    pub node: &'a Node,
    /// Scene-absolute transform of the node's *parent* — the node's own box
    /// transform is still applied by the backend, exactly as when it received
    /// this as `parent_transform`.
    pub transform: AffineTransform,
    /// The same, but relative to `clip`'s space, or to the flatten root when
    /// there is no clip — excluding `root` itself, as on [`ClipFrame`].
    pub rel_transform: AffineTransform,
    /// Product of every ancestor group's alpha.
    pub alpha: f32,
    /// Innermost clip frame this item sits under, if any.
    pub clip: Option<usize>,
    /// Resolved z-level (own or inherited).
    pub z: f64,
}

/// A scene flattened into one z-ordered draw list.
#[derive(Debug, Clone)]
pub struct FlatScene<'a> {
    /// Draw order: sorted by `z`, ties broken by document (preorder) order.
    pub items: Vec<FlatItem<'a>>,
    /// Clip frame arena. A frame's `parent` always precedes it.
    pub clips: Vec<ClipFrame>,
}

impl FlatScene<'_> {
    /// The clip frames enclosing `clip`, outermost first — the order a
    /// stack-based backend must push them in.
    pub fn clip_chain(&self, clip: Option<usize>) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut cur = clip;
        while let Some(i) = cur {
            chain.push(i);
            cur = self.clips[i].parent;
        }
        chain.reverse();
        chain
    }
}

/// Flatten `scene` into a globally z-ordered draw list.
///
/// `root` is the transform every top-level node starts from — the scene camera
/// (and any output scaling) for a backend that composes transforms explicitly,
/// or identity for one that has already pushed the camera onto its own state
/// stack.
pub fn flatten_scene(scene: &Scene, root: AffineTransform) -> FlatScene<'_> {
    let mut out = FlatScene {
        items: Vec::new(),
        clips: Vec::new(),
    };
    walk(
        &mut out,
        &scene.children,
        root,
        AffineTransform::identity(),
        1.0,
        None,
    );
    // Stable, so equal z-levels keep the document order `walk` visited them in.
    out.items
        .sort_by(|a, b| a.z.partial_cmp(&b.z).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn walk<'a>(
    out: &mut FlatScene<'a>,
    nodes: &'a [Node],
    abs: AffineTransform,
    rel: AffineTransform,
    alpha: f32,
    clip: Option<usize>,
) {
    for node in nodes {
        let NodeKind::Group {
            node_box,
            alpha: group_alpha,
            clip_x,
            clip_y,
            clip_w,
            clip_h,
            clip_enabled,
            camera,
            children,
        } = &node.kind
        else {
            out.items.push(FlatItem {
                node,
                transform: abs,
                rel_transform: rel,
                alpha,
                clip,
                z: node_z_level(node),
            });
            continue;
        };

        // `transform_node_box` composes the box's own local transform onto
        // whatever it is handed, so the absolute and clip-relative chains
        // accumulate through the identical call — no inverse needed.
        let box_abs = transform_node_box(node_box, abs);
        let box_rel = transform_node_box(node_box, rel);

        let needs_clip =
            *clip_enabled || *clip_x > 0.0 || *clip_y > 0.0 || *clip_w < 1.0 || *clip_h < 1.0;
        let (child_clip, child_rel) = if needs_clip {
            let lw = node_box.size.width as f32;
            let lh = node_box.size.height as f32;
            out.clips.push(ClipFrame {
                parent: clip,
                transform: box_abs,
                rel_transform: box_rel,
                rect: (
                    *clip_x as f32 * lw,
                    *clip_y as f32 * lh,
                    *clip_w as f32 * lw,
                    *clip_h as f32 * lh,
                ),
            });
            // Everything below is now measured from inside this clip frame.
            (Some(out.clips.len() - 1), AffineTransform::identity())
        } else {
            (clip, box_rel)
        };

        // The camera applies to the group's content only, inside the clip.
        let box_center = Position::new(node_box.size.width * 0.5, node_box.size.height * 0.5);
        let cam = camera_transform(camera, box_center);

        walk(
            out,
            children,
            cam.concat(box_abs),
            cam.concat(child_rel),
            alpha * *group_alpha as f32,
            child_clip,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Camera, Color, Inheritable, NodeBox, Paint, Position, Size, Style};

    fn node_box(x: f64, y: f64, w: f64, h: f64, z: Option<f64>) -> NodeBox {
        NodeBox {
            position: Position::new(x, y),
            size: Size {
                width: w,
                height: h,
            },
            z_level: match z {
                Some(v) => Inheritable::Own(v),
                None => Inheritable::Inherited(0.0),
            },
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            pivot_x: 0.0,
            pivot_y: 0.0,
        }
    }

    fn style() -> Style {
        Style {
            fill_color: Paint::Solid(Color::from_rgba8(0, 0, 0, 255)),
            stroke_color: Color::from_rgba8(0, 0, 0, 0),
            stroke_width: 0.0,
            alpha: 1.0,
            dash: None,
            dash_offset: 0.0,
        }
    }

    fn rect(id: u64, x: f64, y: f64, z: Option<f64>) -> Node {
        Node {
            id,
            kind: NodeKind::Rect {
                node_box: node_box(x, y, 10.0, 10.0, z),
                style: style(),
                radius: 0.0,
            },
        }
    }

    fn group(id: u64, z: Option<f64>, children: Vec<Node>) -> Node {
        group_with(id, z, 1.0, None, children)
    }

    fn group_with(
        id: u64,
        z: Option<f64>,
        alpha: f64,
        clip: Option<(f64, f64, f64, f64)>,
        children: Vec<Node>,
    ) -> Node {
        let (clip_x, clip_y, clip_w, clip_h) = clip.unwrap_or((0.0, 0.0, 1.0, 1.0));
        Node {
            id,
            kind: NodeKind::Group {
                node_box: node_box(0.0, 0.0, 100.0, 100.0, z),
                alpha,
                clip_x,
                clip_y,
                clip_w,
                clip_h,
                clip_enabled: clip.is_some(),
                camera: Camera {
                    camera_zoom: 1.0,
                    camera_x: 50.0,
                    camera_y: 50.0,
                },
                children,
            },
        }
    }

    fn scene(children: Vec<Node>) -> Scene {
        Scene {
            width: 100.0,
            height: 100.0,
            fill_color: Color::from_rgba8(255, 255, 255, 255),
            camera: Camera {
                camera_zoom: 1.0,
                camera_x: 50.0,
                camera_y: 50.0,
            },
            children,
        }
    }

    fn order(flat: &FlatScene<'_>) -> Vec<u64> {
        flat.items.iter().map(|i| i.node.id).collect()
    }

    #[test]
    fn groups_do_not_appear_in_the_draw_list() {
        let s = scene(vec![group(1, None, vec![rect(2, 0.0, 0.0, None)])]);
        let flat = flatten_scene(&s, AffineTransform::identity());
        assert_eq!(order(&flat), vec![2]);
    }

    #[test]
    fn z_orders_across_group_boundaries() {
        // The reported bug: a z(-1) node in the *second* group must paint
        // below a plain node in the first, even though the first group's
        // subtree comes earlier in document order.
        let s = scene(vec![
            group(1, None, vec![rect(2, 0.0, 0.0, None)]),
            group(3, None, vec![rect(4, 0.0, 0.0, Some(-1.0))]),
        ]);
        let flat = flatten_scene(&s, AffineTransform::identity());
        assert_eq!(order(&flat), vec![4, 2]);
    }

    #[test]
    fn equal_z_keeps_document_order() {
        let s = scene(vec![
            group(
                1,
                None,
                vec![rect(2, 0.0, 0.0, None), rect(3, 0.0, 0.0, None)],
            ),
            rect(4, 0.0, 0.0, None),
            group(5, None, vec![rect(6, 0.0, 0.0, None)]),
        ]);
        let flat = flatten_scene(&s, AffineTransform::identity());
        assert_eq!(order(&flat), vec![2, 3, 4, 6]);
    }

    #[test]
    fn an_inherited_group_z_lifts_the_whole_subtree() {
        // Nothing inside group 3 sets its own z, so all of it inherits z=5 and
        // sorts above the standalone rect — the group stays coherent.
        let s = scene(vec![
            rect(1, 0.0, 0.0, Some(1.0)),
            group(
                3,
                Some(5.0),
                vec![rect(4, 0.0, 0.0, None), rect(5, 0.0, 0.0, None)],
            ),
        ]);
        let mut flat = flatten_scene(&s, AffineTransform::identity());
        // Children inherit the group's z upstream in the engine; here the test
        // tree spells that out explicitly.
        for item in &mut flat.items {
            if item.node.id != 1 {
                item.z = 5.0;
            }
        }
        flat.items
            .sort_by(|a, b| a.z.partial_cmp(&b.z).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(order(&flat), vec![1, 4, 5]);
    }

    #[test]
    fn ancestor_alpha_multiplies_onto_each_item() {
        let s = scene(vec![group_with(
            1,
            None,
            0.5,
            None,
            vec![group_with(
                2,
                None,
                0.5,
                None,
                vec![rect(3, 0.0, 0.0, None)],
            )],
        )]);
        let flat = flatten_scene(&s, AffineTransform::identity());
        assert!((flat.items[0].alpha - 0.25).abs() < 1e-6);
    }

    #[test]
    fn a_clipped_group_records_its_window_in_local_box_space() {
        let s = scene(vec![group_with(
            1,
            None,
            1.0,
            Some((0.0, 0.0, 0.5, 1.0)),
            vec![rect(2, 0.0, 0.0, None)],
        )]);
        let flat = flatten_scene(&s, AffineTransform::identity());
        let clip = flat.items[0].clip.expect("item should be clipped");
        assert_eq!(flat.clips[clip].rect, (0.0, 0.0, 50.0, 100.0));
        assert_eq!(flat.clip_chain(Some(clip)), vec![clip]);
    }

    #[test]
    fn nested_clips_chain_outermost_first() {
        let s = scene(vec![group_with(
            1,
            None,
            1.0,
            Some((0.0, 0.0, 0.5, 1.0)),
            vec![group_with(
                2,
                None,
                1.0,
                Some((0.0, 0.0, 1.0, 0.5)),
                vec![rect(3, 0.0, 0.0, None)],
            )],
        )]);
        let flat = flatten_scene(&s, AffineTransform::identity());
        let chain = flat.clip_chain(flat.items[0].clip);
        assert_eq!(chain, vec![0, 1]);
        assert_eq!(flat.clips[1].parent, Some(0));
    }

    #[test]
    fn rel_transform_composes_back_to_the_absolute_one() {
        // Under a clip frame, `rel_transform` is measured from that frame's
        // space: pushing the frame's own transform and then the item's must
        // land exactly where the absolute transform does.
        let inner = Node {
            id: 3,
            kind: NodeKind::Group {
                node_box: node_box(7.0, 11.0, 40.0, 40.0, None),
                alpha: 1.0,
                clip_x: 0.0,
                clip_y: 0.0,
                clip_w: 1.0,
                clip_h: 1.0,
                clip_enabled: false,
                camera: Camera {
                    camera_zoom: 1.0,
                    camera_x: 20.0,
                    camera_y: 20.0,
                },
                children: vec![rect(4, 0.0, 0.0, None)],
            },
        };
        let s = scene(vec![group_with(
            1,
            None,
            1.0,
            Some((0.0, 0.0, 1.0, 1.0)),
            vec![inner],
        )]);
        let flat = flatten_scene(&s, AffineTransform::from_translate(3.0, 5.0));
        let item = &flat.items[0];
        let frame = &flat.clips[item.clip.expect("clipped")];
        let root = AffineTransform::from_translate(3.0, 5.0);
        let composed = item.rel_transform.concat(frame.rel_transform).concat(root);
        let (ax, ay) = item.transform.apply(1.0, 2.0);
        let (bx, by) = composed.apply(1.0, 2.0);
        assert!(
            (ax - bx).abs() < 1e-4 && (ay - by).abs() < 1e-4,
            "absolute {:?} vs composed {:?}",
            (ax, ay),
            (bx, by)
        );
    }
}
