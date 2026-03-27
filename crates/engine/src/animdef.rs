use crate::FrameId;
use crate::avalue::{AnimatedValue, FrameValue};
use crate::basictypes::{AvId, NodeId};
use crate::defs::{CallExpr, Expr, Node, SceneDef};
use crate::eval::EvalCtx;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use tracing::debug;

pub struct AnimationDef {
    pub(crate) scene: SceneDef,
    pub(crate) nodes: HashMap<NodeId, Node>,
    pub(crate) animated_values: HashMap<AvId, AnimatedValue>,
}

#[derive(Deserialize)]
struct RawAnimationDef {
    scene: SceneDef,
    nodes: Vec<Node>,
    animated_values: Vec<AnimatedValue>,
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

impl AnimationDef {
    pub fn build_scene(&self, frame_id: FrameId) -> anyhow::Result<renderer::Scene> {
        let ctx = EvalCtx::new(frame_id, self);
        self.scene.eval(&ctx)
    }

    pub fn key_frames(&self) -> Vec<FrameId> {
        let mut frames: HashSet<FrameId> = Default::default();
        for av in self.animated_values.values() {
            av.collect_key_frames(&mut frames);
        }
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

    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        let raw: RawAnimationDef = serde_json::from_str(s)?;
        Self::from_raw(raw)
    }

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

        let animated_values: HashMap<AvId, AnimatedValue> = raw
            .animated_values
            .into_iter()
            .map(|av| (av.id, av))
            .collect();

        Ok(AnimationDef {
            scene: raw.scene,
            nodes,
            animated_values,
        })
    }
}

// ───────────────────────────────── Tests ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avalue::AnimatedValueKind;
    use crate::defs::Value;

    const EXAMPLE_JSON: &str = r#"{
  "scene": {"kind": "scene", "id": 0, "width": 1, "height": 2, "fill_color": 3, "children": [10, 20]},
  "animated_values": [
    {"kind": "const",    "id": 1, "value": 200},
    {"kind": "const",    "id": 2, "value": 200},
    {"kind": "const",    "id": 3, "value": {"color": "white"}},
    {"kind": "animated", "id": 10, "values": [{"frame": 0, "value": 100, "tr": "sharp"}]},
    {"kind": "const",    "id": 11, "value": 0},
    {"kind": "const",    "id": 12, "value": 0},
    {"kind": "const",    "id": 13, "value": 0},
    {"kind": "const",    "id": 14, "value": 1},
    {"kind": "animated", "id": 15, "values": [{"frame": 30, "value": 30, "tr": "linear"}]},
    {"kind": "const",    "id": 16, "value": 1},
    {"kind": "const",    "id": 17, "value": 1},
    {"kind": "const",    "id": 18, "value": 0},
    {"kind": "const",    "id": 19, "value": 0},
    {"kind": "animated", "id": 30, "values": [{"frame": 0, "value": 20, "tr": "sharp"}]},
    {"kind": "animated", "id": 31, "values": [{"frame": 0, "value": 20, "tr": "sharp"}]},
    {"kind": "animated", "id": 32, "values": [{"frame": 0, "value": {"color": "orange"}, "tr": "sharp"}]},
    {"kind": "const",    "id": 33, "value": null},
    {"kind": "const",    "id": 34, "value": 1},
    {"kind": "const",    "id": 35, "value": 1},
    {"kind": "animated", "id": 40, "values": [{"frame": 0, "value": {"color": "black"}, "tr": "sharp"}]},
    {"kind": "animated", "id": 41, "values": [{"frame": 0, "value": {"color": "red"}, "tr": "sharp"}]},
    {"kind": "const",    "id": 42, "value": 1},
    {"kind": "const",    "id": 43, "value": 1},
    {"kind": "animated", "id": 50, "values": [{"frame": 0, "value": 20, "tr": "sharp"}]},
    {"kind": "animated", "id": 51, "values": [{"frame": 0, "value": 20, "tr": "sharp"}]},
    {"kind": "animated", "id": 60, "values": [
      {"frame": 0,  "value": 10, "tr": "sharp"},
      {"frame": 10, "op": "hold"},
      {"frame": 20, "value": 4,  "tr": "sharp"}
    ]}
  ],
  "nodes": [
    {"kind": "group", "id": 10,
     "x": 10, "y": 11, "width": 12, "height": 13, "alpha": 14,
     "rotation": 15, "scale_x": 16, "scale_y": 17,
     "children": [20, 21]},
    {"kind": "rect", "id": 20,
     "x": 18, "y": 19, "width": 30, "height": 31,
     "fill_color": 32, "stroke_color": 33, "stroke_width": 34, "alpha": 35},
    {"kind": "path", "id": 21,
     "fill_color": 40, "stroke_color": 41, "stroke_width": 42, "alpha": 43,
     "children": [50]},
    {"kind": "move", "id": 50, "x": 50, "y": 51}
  ]
}"#;

    #[test]
    fn parse_flat_format() {
        let anim = AnimationDef::from_str(EXAMPLE_JSON).unwrap();

        // scene
        assert_eq!(anim.scene.children.len(), 2);

        // animated values indexed by id
        let av1 = &anim.animated_values[&AvId::new(1)];
        assert!(matches!(
            av1.kind,
            AnimatedValueKind::Const {
                value: crate::defs::Expr::Const(Value::Int(200))
            }
        ));

        let av3 = &anim.animated_values[&AvId::new(3)];
        assert!(matches!(
            av3.kind,
            AnimatedValueKind::Const {
                value: crate::defs::Expr::Const(Value::Color(_))
            }
        ));

        let av10 = &anim.animated_values[&AvId::new(10)];
        assert!(matches!(av10.kind, AnimatedValueKind::Animated { .. }));

        // call expr with hold
        let av60 = &anim.animated_values[&AvId::new(60)];
        if let AnimatedValueKind::Animated { values } = &av60.kind {
            assert_eq!(values.len(), 3);
            use crate::avalue::FrameValue;
            use crate::basictypes::FrameId;
            assert!(matches!(
                values.get(&FrameId::new(10)),
                Some(FrameValue::Hold)
            ));
        } else {
            panic!("expected animated");
        }

        // nodes indexed by id
        assert!(anim.nodes.contains_key(&NodeId::new(20)));
    }
}
