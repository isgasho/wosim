use derive::DbVec;
use protocol::{Position, RegionPos, Rotation};

#[derive(DbVec)]
pub struct NPC {
    pub region: RegionPos,
    pub region_index: usize,
    pub position: Position,
    pub rotation: Rotation,
}

#[derive(DbVec)]
pub struct PC {
    pub region: RegionPos,
    pub position: Position,
    pub rotation: Rotation,
    pub player: u32,
    pub slot: u8,
}
