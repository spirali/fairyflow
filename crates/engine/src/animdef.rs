use crate::FrameId;
use crate::avalue::AnimatedValue;
use crate::basictypes::{AvId, NodeId};
use crate::nodes::{Node, SceneDef};
use crate::eval::EvalCtx;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

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
    /// Raw debug info from the scene JSON, passed through uninterpreted.
    pub info: serde_json::Value,
}

// ───────────────────────────── Internal single scene ─────────────────────────

struct SingleScene {
    name: String,
    scene: SceneDef,
    nodes: HashMap<NodeId, Node>,
    info: serde_json::Value,
}

// ──────────────────────────────── Public type ─────────────────────────────────

pub struct AnimationDef {
    scenes: Vec<SingleScene>,
    /// `frame_offsets[i]` = absolute start frame of scene `i` in "All" mode.
    frame_offsets: Vec<u32>,
}

// ────────────────────────── Deserialization helpers ──────────────────────────

#[derive(Deserialize)]
struct RawAnimationDef {
    scene: SceneDef,
    nodes: Vec<Node>,
    #[serde(default)]
    info: serde_json::Value,
}

fn check_no_cycles(
    nodes: &HashMap<NodeId, Node>,
    children: &[NodeId],
    visited: &mut HashSet<NodeId>,
    stack: &mut Vec<NodeId>,
) -> anyhow::Result<()> {
    for child_id in children {
        if !visited.insert(*child_id) {
            if visited.contains(&child_id) {
                anyhow::bail!("cycle detected; path: {:?}", &stack);
            }
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
    fn from_raw(raw: RawAnimationDef) -> anyhow::Result<Self> {
        let mut parents: Vec<(NodeId, NodeId)> = Vec::with_capacity(raw.nodes.len());
        for node in &raw.nodes {
            for child_id in node.kind.children() {
                parents.push((*child_id, node.id));
            }
        }
        let mut nodes: HashMap<NodeId, Node> = raw.nodes.into_iter().map(|n| (n.id, n)).collect();

        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        check_no_cycles(&nodes, &raw.scene.children, &mut visited, &mut stack)?;

        for (node_id, parent_id) in parents {
            nodes.get_mut(&node_id).unwrap().parent = Some(parent_id);
        }

        let name = raw
            .scene
            .name
            .clone()
            .unwrap_or_else(|| "Scene".to_string());

        Ok(SingleScene {
            name,
            scene: raw.scene,
            nodes,
            info: raw.info,
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

impl AnimationDef {
    fn from_scenes(scenes: Vec<SingleScene>) -> Self {
        let mut frame_offsets = Vec::with_capacity(scenes.len());
        let mut offset = 0u32;
        for s in &scenes {
            frame_offsets.push(offset);
            offset += s.frame_count();
        }
        AnimationDef { scenes, frame_offsets }
    }

    pub fn build_scene(
        &self,
        frame_id: FrameId,
        selection: SceneSelection,
    ) -> anyhow::Result<renderer::Scene> {
        let (s, local_frame) = self.resolve(frame_id, selection);
        let ctx = EvalCtx::new(local_frame, &s.scene, &s.nodes);
        s.scene.eval(&ctx)
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

    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        let raws: Vec<RawAnimationDef> = if s.trim_start().starts_with('[') {
            serde_json::from_str(s)?
        } else {
            let single: RawAnimationDef = serde_json::from_str(s)?;
            vec![single]
        };
        if raws.is_empty() {
            anyhow::bail!("animation must contain at least one scene");
        }
        let scenes = raws
            .into_iter()
            .enumerate()
            .map(|(i, raw)| {
                SingleScene::from_raw(raw)
                    .map_err(|e| anyhow::anyhow!("scene {}: {}", i, e))
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

// ───────────────────────────────── Tests ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal single-scene JSON using the current inline-expression format.
    const SCENE_JSON: &str = r#"{
  "scene": {"kind": "scene", "id": 0, "width": 200, "height": 200,
            "fill_color": {"color": "white"}, "children": [1], "name": "Test"},
  "animated_values": [],
  "nodes": [
    {"kind": "rect", "id": 1,
     "x": 10, "y": 10, "width": 50, "height": 50,
     "fill_color": {"color": "green"}, "stroke_color": null,
     "stroke_width": 1, "alpha": 1,
     "z_level": {"kind": "inherited", "expr": 0}}
  ]
}"#;

    #[test]
    fn parse_flat_format() {
        let anim = AnimationDef::from_str(SCENE_JSON).unwrap();
        assert_eq!(anim.scene_count(), 1);
        assert_eq!(anim.scenes[0].name, "Test");
        let kf = anim.key_frames(SceneSelection::All);
        assert!(!kf.is_empty());
    }

    #[test]
    fn parse_array_format() {
        let json = format!("[{}]", SCENE_JSON);
        let anim = AnimationDef::from_str(&json).unwrap();
        assert_eq!(anim.scene_count(), 1);
    }

    #[test]
    fn multi_scene_frame_offsets() {
        let json = format!("[{0}, {0}]", SCENE_JSON);
        let anim = AnimationDef::from_str(&json).unwrap();
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
        let anim = AnimationDef::from_str(SCENE_JSON).unwrap();
        anim.build_scene(FrameId::new(0), SceneSelection::All).unwrap();
    }

    #[test]
    fn build_scene_multi_resolves_correctly() {
        let json = format!("[{0}, {0}]", SCENE_JSON);
        let anim = AnimationDef::from_str(&json).unwrap();
        // Frame 0 → scene 0, local 0
        anim.build_scene(FrameId::new(0), SceneSelection::All).unwrap();
        // Frame 1 → scene 1, local 0
        anim.build_scene(FrameId::new(1), SceneSelection::All).unwrap();
        // Single scene selection
        anim.build_scene(FrameId::new(0), SceneSelection::Single(1)).unwrap();
    }
}
