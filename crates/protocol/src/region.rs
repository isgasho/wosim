use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Copy,
    Debug,
    Pod,
    Zeroable,
    Serialize,
    Deserialize,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
)]
#[repr(C)]
pub struct RegionPos {
    pub x: u16,
    pub z: u16,
}

impl RegionPos {
    pub fn distance(self, other: Self) -> u16 {
        (self.x.max(other.x) - self.x.min(other.x)).max(self.z.max(other.z) - self.z.min(other.z))
    }

    pub fn into_index(self, size: u32) -> u32 {
        (self.x as u32) * size + (self.z as u32)
    }
}
