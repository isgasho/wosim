use std::time::{Duration, Instant};

use gpu_util::glam::vec3;
use protocol::{Position, RegionPos};

use crate::character::{NPCVec, PCVec};
use crate::region::RegionVec;

pub struct World {
    pub regions: RegionVec,
    pub physics: physics::World,
    pub pcs: PCVec,
    pub npcs: NPCVec,
    pub size: u32,
    pub region_size: u32,
    pub max_active_regions: u32,
    pub tick: u64,
    pub tick_time: Instant,
    pub tick_delta: Duration,
    pub client_delta: Duration,
}

impl World {
    pub fn region(&self, pos: Position) -> RegionPos {
        RegionPos {
            x: ((pos.x / self.region_size as f32).max(0.0) as u32).min(self.size - 1) as u16,
            z: ((pos.z / self.region_size as f32).max(0.0) as u32).min(self.size - 1) as u16,
        }
    }

    pub fn region_offset(&self, pos: Position, region_pos: RegionPos) -> Position {
        vec3(
            pos.x - (self.region_size as f32 * region_pos.x as f32),
            pos.y,
            pos.z - (self.region_size as f32 * region_pos.z as f32),
        )
    }
}
