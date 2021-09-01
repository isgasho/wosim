use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Position, Rotation};

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldEnter {
    pub self_id: u32,
    pub pos: Position,
    pub rotation: Rotation,
    pub size: u32,
    pub region_size: u32,
    pub max_active_regions: u32,
    pub tick_delta: Duration,
    pub tick: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldInfo {
    pub region_size: u32,
    pub size: u32,
    pub static_distance: u16,
}
