use std::collections::HashSet;

use derive::Vec;
use protocol::RegionPos;

use crate::terrain::TerrainContext;

#[derive(Vec)]
pub struct Region {
    pub pcs: HashSet<u32>,
    pub npcs: HashSet<u32>,
    pub heights: Vec<u8>,
}

impl Region {
    pub fn new(
        region_pos: RegionPos,
        heights: Vec<u8>,
        _region_size: u32,
        _physics: &mut physics::World,
        terrain: &mut TerrainContext,
    ) -> Self {
        terrain.add(region_pos, heights.clone());
        Self {
            pcs: HashSet::new(),
            npcs: HashSet::new(),
            heights,
        }
    }

    pub fn cleanup(
        self,
        pos: RegionPos,
        _world: &mut physics::World,
        terrain: &mut TerrainContext,
    ) {
        terrain.remove(pos);
    }
}
