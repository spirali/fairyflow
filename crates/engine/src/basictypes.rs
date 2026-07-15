use serde::Deserialize;
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash)]
pub(crate) struct NodeId(u32);

impl NodeId {
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
