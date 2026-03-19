use std::collections::{HashMap, HashSet};
use anyhow::{bail, Context};
use crate::avalue::AnimatedValue;
use crate::basictypes::{AvId, NodeId};
use crate::defs::{Node};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Element {
    Node(NodeId),
    Av(AvId),
}

pub(crate) struct CycleChecker<'a> {
    nodes: &'a HashMap<NodeId, Node>,
    avs: &'a HashMap<AvId, AnimatedValue>,
    visited: HashSet<Element>, current: HashSet<Element>, stack: Vec<Element>
}

impl<'a> CycleChecker<'a> {
    pub fn new(nodes: &'a HashMap<NodeId, Node>, avs: &'a HashMap<AvId, AnimatedValue>) -> Self {
        CycleChecker {
            nodes,
            avs,
            visited: HashSet::new(),
            current: HashSet::new(),
            stack: Vec::new(),
        }
    }

    pub fn push_element(&mut self, element: Element) -> anyhow::Result<bool> {
        if self.current.contains(&element) {
            bail!("Cycle detected; path: {:?}", &self.stack);
        }
        if self.visited.contains(&element) {
            return Ok(false);
        }
        self.current.insert(element);
        self.visited.insert(element);
        self.stack.push(element);
        Ok(true)
    }

    pub fn pop_element(&mut self, element: Element) {
        assert!(self.current.remove(&element));
        self.stack.pop().unwrap();
    }

    // pub fn check_node(&mut self, node_id: NodeId) -> anyhow::Result<()> {
    //     if !self.push_element(Element::Node(node_id))? {
    //         return Ok(())
    //     }
    //     let node = self.nodes.get(&node_id).unwrap();
    //
    //     node.kind.check_attributes(&mut |av_id|
    //         self.check_av(av_id)
    //     )?;
    //
    //     for child_id in node.kind.children() {
    //         self.check_node(*child_id)?;
    //     }
    //     self.pop_element(Element::Node(node_id));
    //     Ok(())
    // }
}