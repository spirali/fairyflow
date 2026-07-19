use serde::{Deserialize, de};
use std::fmt::Display;

#[derive(Default, Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq, Deserialize, Hash)]
pub struct FrameId(u32);

impl FrameId {
    #[inline]
    pub fn new(id: u32) -> Self {
        FrameId(id)
    }

    #[inline]
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    pub fn prev(&self) -> FrameId {
        FrameId(self.0.saturating_sub(1))
    }
}

impl Display for FrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct NodeId(u32);

impl NodeId {
    pub const SCENE: NodeId = NodeId(u32::MAX);

    #[inline]
    pub fn new(id: u32) -> Self {
        NodeId(id)
    }

    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0 as u64
    }
}

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Wire format: a plain non-negative integer is a normal (dense `0..n`) node
/// id; `-1` is the reserved `NodeId::SCENE` sentinel. Any other negative
/// value, or a value that overflows `u32`, is a hard parse error rather than
/// silently wrapping/truncating.
impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let n = i64::deserialize(deserializer)?;
        if n == -1 {
            Ok(NodeId::SCENE)
        } else if (0..=u32::MAX as i64).contains(&n) {
            Ok(NodeId(n as u32))
        } else {
            Err(de::Error::custom(format!(
                "invalid node id {n}: expected a non-negative index or -1 (scene/root)"
            )))
        }
    }
}
