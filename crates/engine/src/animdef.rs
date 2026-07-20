use crate::FrameId;
use crate::basictypes::NodeId;
use crate::eval::EvalCtx;
use crate::nodes::{AttrExpr, Node, NodeDef, NodeKind, SceneDef, Size};
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

    if let NodeKind::Group {
        node_box, children, ..
    } = &node.kind
    {
        let sx = node_box.scale_x.eval_or(ctx, 1.0).unwrap_or(1.0);
        let sy = node_box.scale_y.eval_or(ctx, 1.0).unwrap_or(1.0);
        let rot = node_box.rotation.eval_or(ctx, 0.0).unwrap_or(0.0);
        let pvx = node_box.pivot_x.eval_or(ctx, w * 0.5).unwrap_or(w * 0.5) as f32;
        let pvy = node_box.pivot_y.eval_or(ctx, h * 0.5).unwrap_or(h * 0.5) as f32;
        let pos = RcPosition { x, y };

        // Maps group-local coordinates → world coordinates
        let child_transform = positional_transform(
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

        // Group's own AABB: corners (0,0)-(w,h) in group-local space
        map.insert(
            node_id.as_u64(),
            corners_aabb(0.0, 0.0, w as f32, h as f32, child_transform),
        );

        for &c in children {
            collect_world_bounds(nodes, c, ctx, child_transform, map);
        }
    } else {
        // Non-group: position is in parent-local space; apply parent_transform to corners
        map.insert(
            node_id.as_u64(),
            corners_aabb(x as f32, y as f32, w as f32, h as f32, parent_transform),
        );

        for &c in node.kind.children() {
            collect_world_bounds(nodes, c, ctx, parent_transform, map);
        }
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

    fn rect_xy(scene: &renderer_core::Scene, id: u64) -> (f64, f64) {
        let node = scene.children.iter().find(|n| n.id == id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Rect { position, .. } => (position.x, position.y),
            other => panic!("expected a rect, got {other:?}"),
        }
    }

    fn rect_wh(scene: &renderer_core::Scene, id: u64) -> (f64, f64) {
        let node = scene.children.iter().find(|n| n.id == id).unwrap();
        match &node.kind {
            renderer_core::NodeKind::Rect { size, .. } => (size.width, size.height),
            other => panic!("expected a rect, got {other:?}"),
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

    /// A `path` node is a legitimate transform-contributing frame (it has children,
    /// unlike a leaf `Rect`/`Ellipse`): rotating it must correctly rotate points read
    /// out of its local frame via `map_x`/`map_y` (e.g. a path command handle's
    /// `.at()`, Python side). `Path` has no position/size of its own (`get_position`/
    /// `get_size` return `None`, `layout.rs`), so its own `pos`/`pivot` contribution is
    /// always `(0, 0)` regardless of `pivot_x`/`pivot_y` - only `rotation` has any
    /// effect here, which is exactly what this exercises.
    const PATH_ROTATION_JSON: &str = r#"{
  "version": 2,
  "scenes": [
    {"name": "PathRotation", "width": 200, "height": 200, "frames": 1,
     "background": "white", "children": [0, 2],
     "nodes": [
       {"kind": "path", "rotation": 90, "children": [1]},
       {"kind": "move", "x": 10, "y": 0},
       {"kind": "rect",
        "x": ["map_x", 0, -1, 10, 0],
        "y": ["map_y", 0, -1, 10, 0],
        "w": 10, "h": 10, "fill": "purple"}
     ]}
  ]
}"#;

    #[test]
    fn path_rotation_composes_into_map_x_map_y() {
        let anim = AnimationDef::from_json(PATH_ROTATION_JSON).unwrap();
        let scene = anim
            .build_scene(FrameId::new(0), SceneSelection::All)
            .unwrap();
        // Point (10, 0) rotated 90 degrees around the path's local origin (0, 0),
        // then mapped straight into the scene frame (path itself is a direct
        // scene child, so no further ancestor transform applies).
        assert_close(rect_xy(&scene, 2), (0.0, 10.0));
    }

    /// Same shape as `path_rotation_composes_into_map_x_map_y`, but for `image`/
    /// `layer` - both have real position/size (unlike `Path`), so this also
    /// exercises a non-degenerate pivot, matching `Group`'s existing (never
    /// directly rotation-tested before this) transform formula.
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
}
