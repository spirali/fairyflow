use crate::FrameId;
use crate::basictypes::NodeId;
use crate::eval::EvalCtx;
use crate::nodes::{AttrExpr, Camera, Node, NodeDef, NodeKind, SceneDef, Size};
use crate::values::{Color, Expr};
use renderer_core::{
    AffineTransform, Position as RcPosition, Size as RcSize, positional_transform,
};
use serde::Deserialize;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Select which scene(s) to use for rendering / key-frame queries.
#[derive(Debug, Clone, Copy)]
pub enum SceneSelection {
    /// Use the scene at the given index (0-based).
    Single(usize),
    /// Concatenate all scenes end-to-end in document order.
    All,
}

/// Per-scene metadata returned to callers (e.g. the HTTP server).
pub struct SceneInfo {
    pub name: String,
    /// Key frames within this scene (local frame numbers, 0-based).
    pub key_frames: Vec<u32>,
    /// Cue frames within this scene (local frame numbers, 0-based).
    pub cue_frames: Vec<u32>,
    pub frame_count: u32,
    /// Opaque per-node debug info (`--debug`), each tagged with its node's
    /// wire id (array index); empty outside debug runs. The engine never
    /// interprets the contents — see `NodeDef::info`.
    pub info: Vec<serde_json::Value>,
}

// ───────────────────────────── Internal single scene ─────────────────────────

struct SingleScene {
    name: String,
    scene: SceneDef,
    nodes: HashMap<NodeId, Node>,
    info: Vec<serde_json::Value>,
}

// ──────────────────────────────── Public type ─────────────────────────────────

pub struct AnimationDef {
    scenes: Vec<SingleScene>,
    /// `frame_offsets[i]` = absolute start frame of scene `i` in "All" mode.
    frame_offsets: Vec<u32>,
}

impl AnimationDef {
    pub fn collect_images(&self, image_paths: &mut HashSet<Arc<String>>) {
        for scene in &self.scenes {
            for node in scene.nodes.values() {
                node.collect_images(image_paths);
            }
        }
    }
}

// ────────────────────────── Deserialization: wire format v2 ──────────────────

/// Top-level document (`api-v2-impl.md` §A.1): one well-defined shape, no more
/// "single object vs. bare array of objects" polymorphism.
#[derive(Deserialize)]
struct RawDocument {
    #[serde(default)]
    #[allow(dead_code)]
    version: u32,
    scenes: Vec<RawScene>,
}

#[derive(Deserialize)]
struct RawScene {
    name: String,
    width: Expr<f64>,
    height: Expr<f64>,
    background: Expr<Color>,
    #[serde(default)]
    camera_zoom: Option<Expr<f64>>,
    #[serde(default)]
    camera_x: Option<Expr<f64>>,
    #[serde(default)]
    camera_y: Option<Expr<f64>>,
    frames: u32,
    #[serde(default)]
    cues: Vec<u32>,
    #[serde(default)]
    children: Vec<NodeId>,
    #[serde(default)]
    nodes: Vec<NodeDef>,
}

fn check_no_cycles(
    nodes: &HashMap<NodeId, Node>,
    children: &[NodeId],
    visited: &mut HashSet<NodeId>,
    stack: &mut Vec<NodeId>,
) -> anyhow::Result<()> {
    for child_id in children {
        if !visited.insert(*child_id) && visited.contains(child_id) {
            anyhow::bail!("cycle detected; path: {:?}", stack);
        }
        stack.push(*child_id);
        let node = nodes.get(child_id).unwrap();
        check_no_cycles(nodes, node.kind.children(), visited, stack)?;
        stack.pop();
        assert!(visited.remove(child_id));
    }
    Ok(())
}

impl SingleScene {
    fn from_raw(raw: RawScene) -> anyhow::Result<Self> {
        let scene = SceneDef {
            size: Size {
                width: AttrExpr(Some(raw.width)),
                height: AttrExpr(Some(raw.height)),
            },
            fill_color: AttrExpr(Some(raw.background)),
            camera: Camera {
                zoom: AttrExpr(raw.camera_zoom),
                x: AttrExpr(raw.camera_x),
                y: AttrExpr(raw.camera_y),
            },
            frames: raw.frames,
            cues: raw.cues,
            children: raw.children,
        };

        // The engine never interprets `info` — just tag it with the node's
        // wire id (array index) and forward it opaquely (`NodeDef::info`).
        let mut info = Vec::new();
        let raw_nodes = raw
            .nodes
            .into_iter()
            .enumerate()
            .map(|(i, mut def)| {
                if let Some(mut node_info) = def.info.take() {
                    if let serde_json::Value::Object(map) = &mut node_info {
                        map.insert("id".to_string(), serde_json::Value::from(i as u64));
                    }
                    info.push(node_info);
                }
                Node::from_def(i as u32, def)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let mut parents: Vec<(NodeId, NodeId)> = Vec::with_capacity(raw_nodes.len());
        for node in &raw_nodes {
            for child_id in node.kind.children() {
                parents.push((*child_id, node.id));
            }
        }
        let mut nodes: HashMap<NodeId, Node> = raw_nodes.into_iter().map(|n| (n.id, n)).collect();

        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        check_no_cycles(&nodes, &scene.children, &mut visited, &mut stack)?;

        for (node_id, parent_id) in parents {
            nodes.get_mut(&node_id).unwrap().parent = Some(parent_id);
        }

        Ok(SingleScene {
            name: raw.name,
            scene,
            nodes,
            info,
        })
    }

    fn key_frames(&self) -> Vec<FrameId> {
        let mut frames: HashSet<FrameId> = Default::default();
        for node in self.nodes.values() {
            frames.insert(node.start);
            if let Some(end) = node.end {
                frames.insert(end);
            }
        }
        frames.insert(FrameId::new(0));
        let mut result: Vec<FrameId> = frames.into_iter().collect();
        result.sort_unstable();
        result
    }

    fn frame_count(&self) -> u32 {
        self.scene.frames
    }
}

// ─────────────────────────── AnimationDef impl ───────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct NodeBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl AnimationDef {
    fn from_scenes(scenes: Vec<SingleScene>) -> Self {
        let mut frame_offsets = Vec::with_capacity(scenes.len());
        let mut offset = 0u32;
        for s in &scenes {
            frame_offsets.push(offset);
            offset += s.frame_count();
        }
        AnimationDef {
            scenes,
            frame_offsets,
        }
    }

    pub fn build_scene(
        &self,
        frame_id: FrameId,
        selection: SceneSelection,
    ) -> anyhow::Result<renderer_core::Scene> {
        let (s, local_frame) = self.resolve(frame_id, selection);
        let ctx = EvalCtx::new(local_frame, &s.scene, &s.nodes);
        s.scene.eval(&ctx)
    }

    pub fn all_node_bounds(
        &self,
        frame_id: FrameId,
        selection: SceneSelection,
    ) -> anyhow::Result<HashMap<u64, NodeBounds>> {
        let (s, local_frame) = self.resolve(frame_id, selection);
        let ctx = EvalCtx::new(local_frame, &s.scene, &s.nodes);
        let mut map = HashMap::new();
        for &child_id in &s.scene.children {
            collect_world_bounds(
                &s.nodes,
                child_id,
                &ctx,
                AffineTransform::identity(),
                &mut map,
            );
        }
        Ok(map)
    }

    pub fn key_frames(&self, selection: SceneSelection) -> Vec<FrameId> {
        match selection {
            SceneSelection::Single(i) => {
                let i = i.min(self.scenes.len().saturating_sub(1));
                self.scenes[i].key_frames()
            }
            SceneSelection::All => {
                let mut result = Vec::new();
                for (i, s) in self.scenes.iter().enumerate() {
                    let offset = self.frame_offsets[i];
                    for kf in s.key_frames() {
                        result.push(FrameId::new(kf.as_u32() + offset));
                    }
                }
                result.sort_unstable();
                result.dedup();
                result
            }
        }
    }

    /// Per-scene metadata: name, local key frames, and frame count.
    pub fn scene_infos(&self) -> Vec<SceneInfo> {
        self.scenes
            .iter()
            .map(|s| {
                let kf = s.key_frames();
                SceneInfo {
                    name: s.name.clone(),
                    key_frames: kf.into_iter().map(|f| f.as_u32()).collect(),
                    cue_frames: s.scene.cues.clone(),
                    frame_count: s.frame_count(),
                    info: s.info.clone(),
                }
            })
            .collect()
    }

    /// Total frame count for the given selection.
    pub fn frame_count(&self, selection: SceneSelection) -> u32 {
        match selection {
            SceneSelection::Single(i) => {
                let i = i.min(self.scenes.len().saturating_sub(1));
                self.scenes[i].frame_count()
            }
            SceneSelection::All => {
                if self.scenes.is_empty() {
                    return 1;
                }
                let last = self.scenes.len() - 1;
                self.frame_offsets[last] + self.scenes[last].frame_count()
            }
        }
    }

    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    pub fn from_json(s: &str) -> anyhow::Result<Self> {
        let doc: RawDocument = serde_json::from_str(s)?;
        if doc.scenes.is_empty() {
            anyhow::bail!("animation must contain at least one scene");
        }
        let scenes = doc
            .scenes
            .into_iter()
            .enumerate()
            .map(|(i, raw)| {
                SingleScene::from_raw(raw).map_err(|e| anyhow::anyhow!("scene {}: {}", i, e))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(AnimationDef::from_scenes(scenes))
    }

    /// Map an absolute `frame_id` + `selection` to (scene, local frame).
    fn resolve(&self, frame_id: FrameId, selection: SceneSelection) -> (&SingleScene, FrameId) {
        match selection {
            SceneSelection::Single(i) => {
                let i = i.min(self.scenes.len().saturating_sub(1));
                (&self.scenes[i], frame_id)
            }
            SceneSelection::All => {
                let f = frame_id.as_u32();
                let idx = self
                    .frame_offsets
                    .partition_point(|&offset| offset <= f)
                    .saturating_sub(1)
                    .min(self.scenes.len().saturating_sub(1));
                let local = f.saturating_sub(self.frame_offsets[idx]);
                (&self.scenes[idx], FrameId::new(local))
            }
        }
    }
}

// ──────────────────────── World-space bounds helpers ─────────────────────────

fn collect_world_bounds(
    nodes: &HashMap<NodeId, Node>,
    node_id: NodeId,
    ctx: &EvalCtx,
    parent_transform: AffineTransform,
    map: &mut HashMap<u64, NodeBounds>,
) {
    let Some(node) = nodes.get(&node_id) else {
        return;
    };

    let x = node.get_x(ctx).unwrap_or(0.0);
    let y = node.get_y(ctx).unwrap_or(0.0);
    let w = node.get_width(ctx).unwrap_or(0.0);
    let h = node.get_height(ctx).unwrap_or(0.0);

    // `Layer` carries a `NodeBox` (rotation/scale/pivot are wired at the data
    // layer), but the renderer deliberately does not paint them yet - a
    // layer's position is a translate-only nudge, not a real local origin
    // (see api-v2-impl.md). World bounds must match what's actually painted,
    // so `Layer` stays on the flat/translate-only path below rather than
    // joining the generic `node_box()` branch.
    let child_transform = if !matches!(node.kind, NodeKind::Layer { .. })
        && let Some(node_box) = node.kind.node_box()
    {
        let sx = node_box.scale_x.eval_or(ctx, 1.0).unwrap_or(1.0);
        let sy = node_box.scale_y.eval_or(ctx, 1.0).unwrap_or(1.0);
        let rot = node_box.rotation.eval_or(ctx, 0.0).unwrap_or(0.0);
        let pvx = node_box.pivot_x.eval_or(ctx, w * 0.5).unwrap_or(w * 0.5) as f32;
        let pvy = node_box.pivot_y.eval_or(ctx, h * 0.5).unwrap_or(h * 0.5) as f32;
        let pos = RcPosition { x, y };

        // Maps this node's local coordinates → world coordinates
        let t = positional_transform(
            pos,
            RcSize {
                width: sx,
                height: sy,
            },
            rot,
            pvx,
            pvy,
            parent_transform,
        );

        // This node's own AABB: corners (0,0)-(w,h) in its own local space
        map.insert(
            node_id.as_u64(),
            corners_aabb(0.0, 0.0, w as f32, h as f32, t),
        );
        t
    } else {
        // No box (or a Layer): position is in parent-local space; apply
        // parent_transform to corners as-is.
        map.insert(
            node_id.as_u64(),
            corners_aabb(x as f32, y as f32, w as f32, h as f32, parent_transform),
        );
        parent_transform
    };

    for &c in node.kind.children() {
        collect_world_bounds(nodes, c, ctx, child_transform, map);
    }
}

fn corners_aabb(x: f32, y: f32, w: f32, h: f32, t: AffineTransform) -> NodeBounds {
    let pts = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)];
    let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
    let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (cx, cy) in pts {
        let wx = cx * t.a + cy * t.c + t.e;
        let wy = cx * t.b + cy * t.d + t.f;
        min_x = min_x.min(wx);
        max_x = max_x.max(wx);
        min_y = min_y.min(wy);
        max_y = max_y.max(wy);
    }
    NodeBounds {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

// ───────────────────────────────── Tests ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal single-scene JSON using the v2 wire format.
    const SCENE_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "Test", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "rect", "x": 10, "y": 10, "w": 50, "h": 50, "fill": "green"}
     ]}
  ]
}"#;

    #[test]
    fn parse_v2_format() {
        let anim = AnimationDef::from_json(SCENE_JSON).unwrap();
        assert_eq!(anim.scene_count(), 1);
        assert_eq!(anim.scenes[0].name, "Test");
        let kf = anim.key_frames(SceneSelection::All);
        assert!(!kf.is_empty());
    }

    #[test]
    fn multi_scene_frame_offsets() {
        let doc: serde_json::Value = serde_json::from_str(SCENE_JSON).unwrap();
        let scene = doc["scenes"][0].clone();
        let json = serde_json::json!({"version": 2, "scenes": [scene.clone(), scene]}).to_string();
        let anim = AnimationDef::from_json(&json).unwrap();
        assert_eq!(anim.scene_count(), 2);
        // Each scene has only frame 0 → frame_count = 1.
        // Scene 0 offset = 0, scene 1 offset = 1.
        assert_eq!(anim.frame_offsets[0], 0);
        assert_eq!(anim.frame_offsets[1], 1);
        let all_kf = anim.key_frames(SceneSelection::All);
        // Frame 1 = scene-1 frame-0 shifted by offset 1.
        assert!(all_kf.iter().any(|f| f.as_u32() == 1));
    }

    #[test]
    fn build_scene_all() {
        let anim = AnimationDef::from_json(SCENE_JSON).unwrap();
        anim.build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
    }

    #[test]
    fn build_scene_multi_resolves_correctly() {
        let doc: serde_json::Value = serde_json::from_str(SCENE_JSON).unwrap();
        let scene = doc["scenes"][0].clone();
        let json = serde_json::json!({"version": 2, "scenes": [scene.clone(), scene]}).to_string();
        let anim = AnimationDef::from_json(&json).unwrap();
        // Frame 0 → scene 0, local 0
        anim.build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // Frame 1 → scene 1, local 0
        anim.build_scene(FrameId::new(1), SceneSelection::All)
            .unwrap();
        // Single scene selection
        anim.build_scene(FrameId::new(0), SceneSelection::Single(1))
            .unwrap();
    }

    const SCENE_SENTINEL_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "SceneSentinel", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0, 2, 3, 4],
     "nodes": [
       {"kind": "group", "x": 100, "y": 100, "w": 50, "h": 50, "layout": {"kind": "center"},
        "children": [1]},
       {"kind": "rect", "x": 5, "y": 5, "w": 10, "h": 10, "fill": "green"},
       {"kind": "rect",
        "x": ["map_x", 1, -1, 5, 5],
        "y": ["map_y", 1, -1, 5, 5],
        "w": 10, "h": 10, "fill": "red"},
       {"kind": "rect",
        "x": ["map_x", -1, -1, 42, 99],
        "y": ["map_y", -1, -1, 42, 99],
        "w": 10, "h": 10, "fill": "blue"},
       {"kind": "rect",
        "x": ["auto_x", -1], "y": ["auto_y", -1],
        "w": ["auto_w", -1], "h": ["auto_h", -1], "fill": "gold"}
     ]}
  ]
}"#;

    /// Finds a node by id anywhere in the tree (not just direct scene
    /// children) - needed for nodes nested inside a `Group` (e.g. `Row`/
    /// `Column` layout children), unlike the flat probe rects used by most
    /// fixtures in this module.
    fn find_node(nodes: &[renderer_core::Node], id: u64) -> Option<&renderer_core::Node> {
        for node in nodes {
            if node.id == id {
                return Some(node);
            }
            if let renderer_core::NodeKind::Group { children, .. } = &node.kind
                && let Some(found) = find_node(children, id)
            {
                return Some(found);
            }
        }
        None
    }

    fn rect_xy(scene: &renderer_core::Scene, id: u64) -> (f64, f64) {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Rect { node_box, .. } => {
                (node_box.position.x, node_box.position.y)
            }
            other => panic!("expected a rect, got {other:?}"),
        }
    }

    fn rect_wh(scene: &renderer_core::Scene, id: u64) -> (f64, f64) {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Rect { node_box, .. } => {
                (node_box.size.width, node_box.size.height)
            }
            other => panic!("expected a rect, got {other:?}"),
        }
    }

    fn group_wh(scene: &renderer_core::Scene, id: u64) -> (f64, f64) {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Group { node_box, .. } => {
                (node_box.size.width, node_box.size.height)
            }
            other => panic!("expected a group, got {other:?}"),
        }
    }

    /// Rotation math routes through `f64::cos`/`sin`, so e.g. `cos(90°)` lands at
    /// `~6e-17`, not exactly `0.0` - assertions on rotated points need an epsilon.
    fn assert_close(actual: (f64, f64), expected: (f64, f64)) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn node_transform_resolves_scene_sentinel() {
        let anim = AnimationDef::from_json(SCENE_SENTINEL_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // node 2: maps (5, 5) from node 1's local frame (nested in a group at
        // (100, 100)) into the SCENE root frame -> (105, 105).
        assert_eq!(rect_xy(&scene, 2), (105.0, 105.0));
        // node 3: source == target == NodeId::SCENE -> identity fast path.
        assert_eq!(rect_xy(&scene, 3), (42.0, 99.0));
    }

    #[test]
    fn auto_x_y_w_h_resolve_scene_sentinel() {
        let anim = AnimationDef::from_json(SCENE_SENTINEL_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // node 4: auto_x/auto_y/auto_w/auto_h against the -1 sentinel ->
        // the Scene's own default position (0, 0) and its dimensions.
        assert_eq!(rect_xy(&scene, 4), (0.0, 0.0));
        assert_eq!(rect_wh(&scene, 4), (200.0, 200.0));
    }

    #[test]
    fn node_id_rejects_other_negative_values() {
        let json = SCENE_SENTINEL_JSON.replace(r#""map_x", 1, -1"#, r#""map_x", 1, -2"#);
        assert!(AnimationDef::from_json(&json).is_err());
    }

    /// `image`/`layer` have real position/size, so this exercises a
    /// non-degenerate pivot, matching `Group`'s existing (never directly
    /// rotation-tested before this) transform formula.
    const IMAGE_ROTATION_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "ImageRotation", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0, 2],
     "nodes": [
       {"kind": "image", "x": 100, "y": 50, "w": 50, "h": 30, "rotation": 90,
        "children": [1]},
       {"kind": "layer", "layer_name": "l1"},
       {"kind": "rect",
        "x": ["map_x", 0, -1, 40, 10],
        "y": ["map_y", 0, -1, 40, 10],
        "w": 10, "h": 10, "fill": "purple"}
     ]}
  ]
}"#;

    #[test]
    fn image_rotation_composes_into_map_x_map_y() {
        let anim = AnimationDef::from_json(IMAGE_ROTATION_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // image at (100, 50), 50x30, rotated 90 degrees around its center pivot
        // (25, 15): point (40, 10) -> offset from pivot (15, -5) -> rotated ->
        // (5, 15) offset from pivot -> + pivot + pos = (130, 80).
        assert_close(rect_xy(&scene, 2), (130.0, 80.0));
    }

    /// `Rect`/`Ellipse` now carry rotation/scale/pivot wire fields (for a later
    /// renderer step to consume) but deliberately never contribute a transform via
    /// `ancestor_chain`/`node_own_transform` (see their doc comments) - this is a
    /// parse-only smoke test proving the fields round-trip without error, and that
    /// the evaluated `renderer_core::NodeKind::Rect` position/size are completely
    /// unaffected by them today (the deliberate "visual no-op until the renderer
    /// step lands" boundary this implementation step draws).
    const LEAF_TRANSFORM_FIELDS_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "LeafTransformFields", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0, 1],
     "nodes": [
       {"kind": "rect", "x": 0, "y": 0, "w": 10, "h": 10, "fill": "green",
        "rotation": 45, "scale_x": 2, "scale_y": 0.5, "pivot_x": 0.25, "pivot_y": 0.75},
       {"kind": "ellipse", "x": 20, "y": 0, "w": 10, "h": 10, "fill": "blue",
        "rotation": 30, "scale_x": 1.5}
     ]}
  ]
}"#;

    #[test]
    fn leaf_rotation_scale_pivot_fields_parse_and_do_not_affect_output() {
        let anim = AnimationDef::from_json(LEAF_TRANSFORM_FIELDS_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        assert_eq!(rect_xy(&scene, 0), (0.0, 0.0));
        assert_eq!(rect_wh(&scene, 0), (10.0, 10.0));
    }

    /// `aabb_offset`/`get_outer_width`/`get_outer_height` (`layout.rs`) used to
    /// only special-case `Group`; a rotated `Rect` (now a real, painted rotation
    /// since leaf rotation/scale shipped) was laid out by its Row siblings as if
    /// it were never rotated. This regression-tests the generalization to any
    /// `node_box()`-bearing kind.
    const ROTATED_ROW_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "RotatedRow", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group", "x": 0, "y": 0, "w": 40, "h": 30,
        "layout": {"kind": "row", "gap": 0, "align": 0.5, "reserve": true},
        "children": [1, 2]},
       {"kind": "rect", "w": 30, "h": 10, "rotation": 90, "fill": "green"},
       {"kind": "rect", "w": 10, "h": 10, "fill": "blue"}
     ]}
  ]
}"#;

    #[test]
    fn rotated_rect_in_row_offsets_next_sibling_by_outer_width() {
        let anim = AnimationDef::from_json(ROTATED_ROW_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // node 1: 30x10 rect rotated 90 degrees about its default center pivot
        // (15, 5) - its *outer* (rotated) footprint is 10 wide x 30 tall, not
        // its raw 30x10. Its own reported (unrotated) top-left is offset so
        // that footprint starts flush at the row's origin: (-10, 10).
        assert_close(rect_xy(&scene, 1), (-10.0, 10.0));
        // node 2: unrotated 10x10 rect - sits right after node 1's *outer*
        // width (10, gap=0), vertically centered in the row's own 30-tall box.
        assert_close(rect_xy(&scene, 2), (10.0, 10.0));
    }

    const GRID_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "Grid", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group",
        "layout": {"kind": "grid", "cols": 2, "gap_x": 5, "gap_y": 5, "reserve": true},
        "children": [1, 2, 3]},
       {"kind": "rect", "w": 40, "h": 10, "fill": "red"},
       {"kind": "rect", "w": 10, "h": 30, "fill": "green"},
       {"kind": "rect", "w": 20, "h": 20, "fill": "blue"}
     ]}
  ]
}"#;

    #[test]
    fn grid_places_children_row_major_with_col_and_row_sizing() {
        let anim = AnimationDef::from_json(GRID_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // 2 cols; row-major: (row0,col0)=node1 40x10, (row0,col1)=node2 10x30,
        // (row1,col0)=node3 20x20. col_widths=[40,20->max(40,20)=40, 10],
        // row_heights=[max(10,30)=30, 20].
        assert_close(rect_xy(&scene, 1), (0.0, 0.0));
        assert_close(rect_xy(&scene, 2), (45.0, 0.0)); // col0 width (40) + gap_x (5)
        assert_close(rect_xy(&scene, 3), (0.0, 35.0)); // row0 height (30) + gap_y (5)
        // group auto-size: cols sum (40+10) + 1 gap_x (5) = 55; rows sum (30+20) + 1 gap_y (5) = 55.
        assert_close(group_wh(&scene, 0), (55.0, 55.0));
    }

    const GRID_PARTIAL_ROW_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "GridPartial", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group",
        "layout": {"kind": "grid", "cols": 2, "gap_x": 0, "gap_y": 0, "reserve": true},
        "children": [1, 2, 3]},
       {"kind": "rect", "w": 20, "h": 10, "fill": "red"},
       {"kind": "rect", "w": 20, "h": 10, "fill": "green"},
       {"kind": "rect", "w": 20, "h": 10, "fill": "blue"}
     ]}
  ]
}"#;

    #[test]
    fn grid_partial_last_row() {
        let anim = AnimationDef::from_json(GRID_PARTIAL_ROW_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // 3 children, 2 cols -> a partial last row (1 child). Width always
        // reserves the full declared 2 columns; height only reserves the 2
        // rows actually filled, not a phantom 3rd.
        assert_close(group_wh(&scene, 0), (40.0, 20.0));
    }

    const GRID_EMPTY_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "GridEmpty", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group",
        "layout": {"kind": "grid", "cols": 3, "gap_x": 10, "gap_y": 10, "reserve": true},
        "children": []}
     ]}
  ]
}"#;

    #[test]
    fn grid_empty_group_is_zero_sized() {
        let anim = AnimationDef::from_json(GRID_EMPTY_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // No phantom (cols-1)*gap_x / (rows-1)*gap_y for an empty group.
        assert_close(group_wh(&scene, 0), (0.0, 0.0));
    }

    const ROTATED_GRID_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "RotatedGrid", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group",
        "layout": {"kind": "grid", "cols": 2, "gap_x": 0, "gap_y": 0, "reserve": true},
        "children": [1, 2]},
       {"kind": "rect", "w": 30, "h": 10, "rotation": 90, "fill": "green"},
       {"kind": "rect", "w": 10, "h": 10, "fill": "blue"}
     ]}
  ]
}"#;

    #[test]
    fn rotated_rect_in_grid_uses_outer_dimensions_for_position_not_own_size() {
        let anim = AnimationDef::from_json(ROTATED_GRID_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // Same rotated 30x10->90deg rect as the Row regression above, now the
        // sole occupant of grid cell (row0,col0): its *outer* 10x30 footprint
        // drives its own reported top-left and column0's *position* width
        // (auto_x/auto_y use outer dims, matching Row/Column's convention).
        assert_close(rect_xy(&scene, 1), (-10.0, 10.0));
        // node 2 at (row0,col1): starts right after column0's outer width (10).
        assert_close(rect_xy(&scene, 2), (10.0, 0.0));
        // The group's own auto-size uses PLAIN (non-outer) dims per child,
        // matching Row/Column's existing auto_width/auto_height convention:
        // col_widths=[30 (node1's raw w), 10], row_heights=[max(10, 10)=10].
        assert_close(group_wh(&scene, 0), (40.0, 10.0));
    }

    const PADDING_COLUMN_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "PaddingColumn", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group",
        "layout": {"kind": "column", "gap": 0, "align": 0, "reserve": true},
        "padding_top": 10, "padding_right": 6, "padding_bottom": 4, "padding_left": 20,
        "children": [1]},
       {"kind": "rect", "w": 15, "h": 8, "fill": "red"}
     ]}
  ]
}"#;

    const PADDING_CENTER_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "PaddingCenter", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group", "w": 100, "h": 50,
        "layout": {"kind": "center"},
        "padding_left": 10, "padding_right": 30,
        "children": [1]},
       {"kind": "rect", "w": 20, "h": 10, "fill": "red"}
     ]}
  ]
}"#;

    #[test]
    fn padding_insets_column_and_center_layout() {
        let anim = AnimationDef::from_json(PADDING_COLUMN_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // align=0 (left) + first child: x = padding_left, y = padding_top.
        assert_close(rect_xy(&scene, 1), (20.0, 10.0));
        // group auto-size: content (15x8) + padding (left+right=26, top+bottom=14).
        assert_close(group_wh(&scene, 0), (41.0, 22.0));

        let anim = AnimationDef::from_json(PADDING_CENTER_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // Center layout centers within the *padded* box, not the raw box:
        // x = 10 + (100 - 10 - 30 - 20) / 2 = 30; y = 0 + (50 - 0 - 0 - 10) / 2 = 20.
        assert_close(rect_xy(&scene, 1), (30.0, 20.0));
    }

    const PADDING_ABSENT_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "PaddingAbsent", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "group", "w": 40,
        "layout": {"kind": "column", "gap": 0, "align": 0.5, "reserve": true},
        "children": [1]},
       {"kind": "rect", "w": 20, "h": 10, "fill": "red"}
     ]}
  ]
}"#;

    #[test]
    fn padding_defaults_to_zero_when_absent_from_wire() {
        let anim = AnimationDef::from_json(PADDING_ABSENT_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // No padding_* fields at all in the wire JSON -> identical to
        // pre-padding behavior: (40 - 20) * 0.5 = 10.
        assert_close(rect_xy(&scene, 1), (10.0, 0.0));
    }

    const FLOAT_MAX_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "FloatMax", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0, 1],
     "nodes": [
       {"kind": "rect", "x": ["max", 3, 7], "y": 0, "w": 10, "h": 10, "fill": "red"},
       {"kind": "rect", "x": ["max", 7, 3], "y": 0, "w": 10, "h": 10, "fill": "blue"}
     ]}
  ]
}"#;

    #[test]
    fn float_call_max_picks_larger_operand() {
        let anim = AnimationDef::from_json(FLOAT_MAX_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        assert_close(rect_xy(&scene, 0), (7.0, 0.0));
        assert_close(rect_xy(&scene, 1), (7.0, 0.0));
    }

    /// `collect_world_bounds` used to only build a rotation/scale-aware
    /// `child_transform` for `Group`; an `Image` (which now really rotates) got
    /// an axis-aligned world-bounds entry, and its rotation never propagated to
    /// its `Layer` children. `Layer` itself is deliberately excluded from the
    /// generalization - its own rotation/scale/pivot are wired but not yet
    /// painted (see api-v2-impl.md), so its own AABB must stay on the flat
    /// path to match what's actually rendered; only its *position* (inherited
    /// from the parent's transform) should move.
    const ROTATED_IMAGE_WORLD_BOUNDS_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "RotatedImageBounds", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "image", "x": 100, "y": 50, "w": 50, "h": 30, "rotation": 90,
        "children": [1]},
       {"kind": "layer", "layer_name": "l1", "x": 0, "y": 0, "w": 10, "h": 10}
     ]}
  ]
}"#;

    #[test]
    fn image_world_bounds_reflect_own_rotation_layer_stays_flat() {
        let anim = AnimationDef::from_json(ROTATED_IMAGE_WORLD_BOUNDS_JSON).unwrap();
        let bounds = anim
            .all_node_bounds(FrameId::new(0), SceneSelection::All)
            .unwrap();

        // image: 50x30 rotated 90 degrees about its center pivot (25, 15),
        // positioned at (100, 50) -> world AABB is 30 wide x 50 tall (swapped
        // from its raw 50x30), at (110, 40).
        let image = bounds.get(&0).unwrap();
        assert!((image.x - 110.0).abs() < 1e-3, "image.x = {}", image.x);
        assert!((image.y - 40.0).abs() < 1e-3, "image.y = {}", image.y);
        assert!(
            (image.width - 30.0).abs() < 1e-3,
            "image.width = {}",
            image.width
        );
        assert!(
            (image.height - 50.0).abs() < 1e-3,
            "image.height = {}",
            image.height
        );

        // layer: 10x10, no rotation of its own - its *dimensions* stay 10x10
        // (unswapped) even though its *position* is carried by the image's
        // rotated transform.
        let layer = bounds.get(&1).unwrap();
        assert!(
            (layer.width - 10.0).abs() < 1e-3,
            "layer.width = {}",
            layer.width
        );
        assert!(
            (layer.height - 10.0).abs() < 1e-3,
            "layer.height = {}",
            layer.height
        );
        assert!((layer.x - 130.0).abs() < 1e-3, "layer.x = {}", layer.x);
        assert!((layer.y - 40.0).abs() < 1e-3, "layer.y = {}", layer.y);
    }

    /// A bare `.clip()` (Python side) used to be a complete no-op: both
    /// renderers only clip when `clip_x/y/w/h` differ from the literal
    /// default box, so a group that explicitly requests clipping "at the
    /// full box" was indistinguishable from one that never called `.clip()`
    /// at all. `clip_enabled` is a derived presence flag - true whenever any
    /// of the four fields is present in the wire JSON, regardless of value.
    const CLIP_ENABLED_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "ClipEnabled", "width": 100, "height": 100, "frames": 1,
     "background": "white", "children": [0, 1],
     "nodes": [
       {"kind": "group", "x": 0, "y": 0, "w": 50, "h": 50,
        "layout": {"kind": "center"},
        "clip_x": 0, "clip_y": 0, "clip_w": 1, "clip_h": 1,
        "children": []},
       {"kind": "group", "x": 0, "y": 0, "w": 50, "h": 50,
        "layout": {"kind": "center"},
        "children": []}
     ]}
  ]
}"#;

    fn group_clip_enabled(scene: &renderer_core::Scene, id: u64) -> bool {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Group { clip_enabled, .. } => *clip_enabled,
            other => panic!("expected a group, got {other:?}"),
        }
    }

    #[test]
    fn clip_enabled_reflects_explicit_presence_not_value() {
        let anim = AnimationDef::from_json(CLIP_ENABLED_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // Node 0: clip_x/y/w/h explicitly present at literal-default values -
        // clip_enabled must still be true (the exact bug this fixes).
        assert!(group_clip_enabled(&scene, 0));
        // Node 1: clip_x/y/w/h entirely absent from the JSON - clip_enabled
        // stays false, so the renderers' perf fast-path is unaffected.
        assert!(!group_clip_enabled(&scene, 1));
    }

    /// A `Group` with `camera_zoom`/`camera_x`/`camera_y` entirely absent must
    /// evaluate to the identity camera: `zoom=1.0`, `x/y` at the box's own
    /// center (`w*0.5`/`h*0.5`) - mirrors `pivot_x`/`pivot_y`'s own
    /// computed-default pattern. A second group with explicit values must
    /// round-trip them unchanged.
    const CAMERA_DEFAULTS_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "CameraDefaults", "width": 100, "height": 100, "frames": 1,
     "background": "white", "children": [0, 1],
     "nodes": [
       {"kind": "group", "x": 0, "y": 0, "w": 40, "h": 20,
        "layout": {"kind": "center"}, "children": []},
       {"kind": "group", "x": 0, "y": 0, "w": 40, "h": 20,
        "layout": {"kind": "center"},
        "camera_zoom": 2, "camera_x": 11, "camera_y": 13,
        "children": []}
     ]}
  ]
}"#;

    fn group_camera(scene: &renderer_core::Scene, id: u64) -> renderer_core::Camera {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Group { camera, .. } => *camera,
            other => panic!("expected a group, got {other:?}"),
        }
    }

    #[test]
    fn camera_defaults_to_identity_when_absent() {
        let anim = AnimationDef::from_json(CAMERA_DEFAULTS_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();

        let absent = group_camera(&scene, 0);
        assert_eq!(absent.camera_zoom, 1.0);
        assert_eq!(absent.camera_x, 20.0); // w*0.5
        assert_eq!(absent.camera_y, 10.0); // h*0.5

        let explicit = group_camera(&scene, 1);
        assert_eq!(explicit.camera_zoom, 2.0);
        assert_eq!(explicit.camera_x, 11.0);
        assert_eq!(explicit.camera_y, 13.0);
    }

    /// `Text` gained a `size`/`keep_aspect` (proposal §4.10). Uses `text` nodes
    /// with **no** `tspan`/`tline` children so `measure_text`'s empty-lines
    /// path (`measure_text_lines`, `text_layout.rs`) short-circuits to
    /// `(0.0, 0.0)` before touching any font machinery — keeps this a
    /// deterministic engine-only test independent of `Resources::init()`/
    /// font availability (real single/both-axis aspect derivation and
    /// descendant-scaling are covered by `tests/test_text_size.py`'s golden
    /// renders instead, which do exercise real glyph measurement).
    const TEXT_SIZE_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "TextSize", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0, 1],
     "nodes": [
       {"kind": "text", "x": 10, "y": 20, "w": 80, "h": 30, "keep_aspect": false},
       {"kind": "text", "x": 0, "y": 0}
     ]}
  ]
}"#;

    fn text_wh(scene: &renderer_core::Scene, id: u64) -> (f64, f64) {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Text { node_box, .. } => {
                (node_box.size.width, node_box.size.height)
            }
            other => panic!("expected a text node, got {other:?}"),
        }
    }

    fn text_keep_aspect(scene: &renderer_core::Scene, id: u64) -> bool {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Text { keep_aspect, .. } => *keep_aspect,
            other => panic!("expected a text node, got {other:?}"),
        }
    }

    #[test]
    fn text_explicit_w_h_keep_aspect_round_trip() {
        // `measure_text`'s thread-local layout engine touches
        // `renderer_core::Resources::get()` on first use even for empty
        // lines (font-context construction, not glyph shaping) - harmless
        // and idempotent to call here regardless of system font availability.
        renderer_core::Resources::init();
        let anim = AnimationDef::from_json(TEXT_SIZE_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // Node 0: both axes + keep_aspect explicit -> `get_width`/`get_height`
        // (layout.rs) never call `auto_width`/`auto_height` at all, so the
        // resolved box is exactly the literal values (the `get_size` wiring
        // this slice added, `layout.rs`/`eval.rs`).
        assert_eq!(text_wh(&scene, 0), (80.0, 30.0));
        assert!(!text_keep_aspect(&scene, 0));
        // Node 1: nothing set -> defaults (empty-text natural extent (0, 0),
        // keep_aspect true).
        assert_eq!(text_wh(&scene, 1), (0.0, 0.0));
        assert!(text_keep_aspect(&scene, 1));
    }

    /// `Text` gained `rotate()`/`scale()`/`pivot()` (item 16, proposal §4.1's
    /// transform tier) by adopting a real `NodeBox` (previously deliberately
    /// absent, per item 14's "Text doesn't get a NodeBox" decision). Same
    /// childless-`text`-node trick as `text_explicit_w_h_keep_aspect_round_trip`
    /// to stay independent of font machinery.
    const TEXT_TRANSFORM_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "TextTransform", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0, 1],
     "nodes": [
       {"kind": "text", "x": 10, "y": 20, "w": 80, "h": 30,
        "rotation": 45, "scale_x": 2, "scale_y": 3, "pivot_x": 5, "pivot_y": 6},
       {"kind": "text", "x": 0, "y": 0}
     ]}
  ]
}"#;

    fn text_node_box(scene: &renderer_core::Scene, id: u64) -> &renderer_core::NodeBox {
        let node = find_node(&scene.children, id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Text { node_box, .. } => node_box,
            other => panic!("expected a text node, got {other:?}"),
        }
    }

    #[test]
    fn text_node_box_rotation_scale_pivot_round_trip() {
        renderer_core::Resources::init();
        let anim = AnimationDef::from_json(TEXT_TRANSFORM_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // Node 0: every transform field explicit -> round-trips exactly,
        // same as Rect/Image already do via the shared `eval_node_box`.
        let node_box = text_node_box(&scene, 0);
        assert_eq!(node_box.rotation, 45.0);
        assert_eq!(node_box.scale_x, 2.0);
        assert_eq!(node_box.scale_y, 3.0);
        assert_eq!(node_box.pivot_x, 5.0);
        assert_eq!(node_box.pivot_y, 6.0);
        // Node 1: nothing set -> identity rotation/scale, pivot defaults to
        // the box's own center (w*0.5, h*0.5) — here (0, 0) since w/h are
        // both 0 (empty text, untouched size default).
        let node_box = text_node_box(&scene, 1);
        assert_eq!(node_box.rotation, 0.0);
        assert_eq!(node_box.scale_x, 1.0);
        assert_eq!(node_box.scale_y, 1.0);
        assert_eq!(node_box.pivot_x, 0.0);
        assert_eq!(node_box.pivot_y, 0.0);
    }

    /// Placeable text runs (item 16, proposal §4.1/§4.10): `tline`/`tspan`
    /// gain a real, wire-honored `x`/`y` that overrides the paragraph-layout
    /// position without reflowing siblings. Three lines: one untouched, one
    /// whose `tline` has an override that should cascade to its one
    /// unoverridden child span, and one whose `tspan` has its *own* override
    /// that must win over its parent `tline`'s (nearest self-or-ancestor,
    /// not "outermost ancestor"). Real font shaping is required (natural
    /// position comes from actual glyph layout), so this needs
    /// `Resources::init()`, unlike the two tests above.
    ///
    /// This is also the regression test for a real stack overflow found by
    /// hand while smoke-testing (not by a prior failing test): resolving an
    /// override's delta needs the block's *natural* layout, which — before
    /// the `eval_as_text_child_for_layout` split (`eval.rs`) — was computed
    /// by re-evaluating the whole block including the very span being
    /// resolved, recursing forever. If that split regresses, this test hangs
    /// or crashes rather than failing an assertion.
    const PLACEABLE_RUNS_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "PlaceableRuns", "width": 300, "height": 200, "frames": 1,
     "background": "white", "children": [0],
     "nodes": [
       {"kind": "text", "x": 20, "y": 30, "font_size": 18, "children": [1, 2]},
       {"kind": "tline", "children": [3]},
       {"kind": "tline", "x": 150, "y": 150, "children": [4, 5]},
       {"kind": "tspan", "text": "Line one stays put"},
       {"kind": "tspan", "text": "Second "},
       {"kind": "tspan", "text": "line", "x": 10, "y": 10}
     ]}
  ]
}"#;

    fn find_text_span(
        children: &[renderer_core::TextChild],
        id: u64,
    ) -> Option<&renderer_core::TextSpan> {
        for child in children {
            match child {
                renderer_core::TextChild::Span(s) if s.id == id => return Some(s),
                renderer_core::TextChild::Span(_) => {}
                renderer_core::TextChild::Group(g) => {
                    if let Some(found) = find_text_span(&g.children, id) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    #[test]
    fn placeable_run_override_cascades_to_nearest_self_or_ancestor() {
        renderer_core::Resources::init();
        let anim = AnimationDef::from_json(PLACEABLE_RUNS_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        let renderer_core::NodeKind::Text { lines, .. } =
            &find_node(&scene.children, 0).unwrap().kind
        else {
            panic!("expected a text node");
        };

        // Span 3: no override anywhere in its ancestor chain -> untouched.
        let span3 = find_text_span(lines, 3).expect("span 3 present");
        assert_eq!(span3.override_offset, None);

        // Span 4: no override of its own, but its parent tline has one ->
        // inherits the group's delta. Finite (not NaN/inf) confirms the
        // walk-up actually found and resolved a winner rather than, say,
        // dividing by a degenerate natural extent.
        let span4 = find_text_span(lines, 4).expect("span 4 present");
        let (dx4, dy4) = span4
            .override_offset
            .expect("span 4 inherits group override");
        assert!(dx4.is_finite() && dy4.is_finite());

        // Span 5: has its own override -> wins over the parent group's.
        let span5 = find_text_span(lines, 5).expect("span 5 present");
        let (dx5, dy5) = span5.override_offset.expect("span 5 has its own override");
        assert!(dx5.is_finite() && dy5.is_finite());

        // Span 4's delta must differ from span 5's -- they resolve against
        // different winning nodes (the group vs. span 5 itself), so a bug
        // that always cascaded to the outermost ancestor (ignoring a closer
        // override) would make these equal.
        assert!((dx4, dy4) != (dx5, dy5));
    }
}
