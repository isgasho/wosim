use protocol::{Position, RegionPos};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Configuration {
    pub size: u32,
    pub region_size: u32,
    pub static_distance: u16,
    pub full_distance: u16,
}

impl Configuration {
    pub fn region(&self, pos: Position) -> RegionPos {
        RegionPos {
            x: ((pos.x / self.region_size as f32).max(0.0) as u32).min(self.size - 1) as u16,
            z: ((pos.z / self.region_size as f32).max(0.0) as u32).min(self.size - 1) as u16,
        }
    }

    pub fn near_region(&self, pos: Position, region: RegionPos) -> bool {
        pos.x >= (region.x as f32 - 0.5) * self.region_size as f32
            && pos.x <= (region.x as f32 + 1.5) * self.region_size as f32
            && pos.z >= (region.z as f32 - 0.5) * self.region_size as f32
            && pos.z <= (region.z as f32 + 1.5) * self.region_size as f32
    }

    pub fn full_size(&self) -> usize {
        self.size as usize * self.region_size as usize + 1
    }
}
