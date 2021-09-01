use serde::{Deserialize, Serialize};

use crate::{Position, Rotation};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalSetup {}

#[derive(Debug, Deserialize, Serialize)]
pub struct StaticSetup {
    pub heights: Vec<u8>,
    pub region_size: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DynamicSetup {
    pub npcs: Vec<(u32, Position, Rotation)>,
    pub pcs: Vec<(u32, Position, Rotation)>,
}
