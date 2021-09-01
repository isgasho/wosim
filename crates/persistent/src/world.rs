use std::{io::ErrorKind, mem::swap};

use database::{DatabaseRef, Entry, Format, Len, Object, Tree};
use protocol::{Position, Rotation, SLOT_COUNT};

use crate::{Configuration, NPCVec, PCVec, Player, PlayerVec, Region, NPC, PC};

pub struct World {
    pub heights: database::Vec<u8>,
    pub npcs: NPCVec,
    pub pcs: PCVec,
    pub players: PlayerVec,
    pub player_index: Tree<u128, u32>,
    pub regions: Vec<Region>,
    pub configuration: Configuration,
}

impl World {
    pub fn new(database: DatabaseRef, configuration: Configuration) -> Self {
        let size = (configuration.size as usize).pow(2);
        let mut regions = Vec::with_capacity(size);
        regions.resize_with(size, || Region::new(database.clone()));
        Self {
            heights: database::Vec::new(database.clone()),
            npcs: NPCVec::new(database.clone()),
            pcs: PCVec::new(database.clone()),
            players: PlayerVec::new(database.clone()),
            player_index: Tree::new(database),
            regions,
            configuration,
        }
    }

    pub fn initialize_player(&mut self, uuid: u128) {
        let mut player_index = self.player_index.write();
        if let Entry::Vacant(vacant) = player_index.entry(&uuid) {
            vacant.insert(self.players.add(Player {
                slots: [u32::MAX; SLOT_COUNT],
            }) as u32);
        }
    }

    pub fn spawn_npc(&mut self, mut position: Position, rotation: Rotation) -> usize {
        let region_pos = self.configuration.region(position);
        let size = self.configuration.full_size();
        let x = (position.x as usize).clamp(0, size - 1);
        let z = (position.z as usize).clamp(0, size - 1);
        position.y = self.heights.read()[z * size + x] as f32 + 1.0;
        let id = self.npcs.add(NPC {
            position,
            rotation,
            region: region_pos,
            region_index: usize::MAX,
        });
        let mut npcs = self.regions[region_pos.into_index(self.configuration.size) as usize]
            .npcs
            .write();
        self.npcs.region_index.write()[id] = npcs.len();
        npcs.push(id);
        id
    }

    pub fn spawn_pc(
        &mut self,
        mut position: Position,
        rotation: Rotation,
        player: u32,
        slot: u8,
    ) -> Option<usize> {
        let slot_container = &mut self.players.slots.write()[player as usize][slot as usize];
        if *slot_container != u32::MAX {
            return None;
        }
        let region_pos = self.configuration.region(position);
        let size = (self.configuration.size * self.configuration.region_size) as usize + 1;
        let x = (position.x as usize).clamp(0, size - 1);
        let z = (position.z as usize).clamp(0, size - 1);
        position.y = self.heights.read()[z * size + x] as f32 + 1.0;
        let id = self.pcs.add(PC {
            position,
            rotation,
            region: region_pos,
            player,
            slot,
        });
        *slot_container = id as u32;
        Some(id)
    }

    pub fn delete_pc(&mut self, player: u32, slot: u8) {
        let mut slots = self.players.slots.write();
        let mut new_slot = u32::MAX;
        swap(&mut new_slot, &mut slots[player as usize][slot as usize]);
        if new_slot != u32::MAX {
            self.pcs.free(new_slot as usize);
        }
    }
}

impl Object for World {
    fn format() -> Format {
        [64; 256]
    }

    fn serialize(&mut self, mut writer: impl std::io::Write) -> std::io::Result<()> {
        bincode::serialize_into(&mut writer, &self.configuration)
            .map_err(|_| std::io::Error::new(ErrorKind::Other, "oh no!"))?;
        self.heights.serialize(&mut writer)?;
        self.npcs.serialize(&mut writer)?;
        self.pcs.serialize(&mut writer)?;
        self.players.serialize(&mut writer)?;
        self.player_index.serialize(&mut writer)?;
        for region in self.regions.iter_mut() {
            region.serialize(&mut writer)?;
        }
        Ok(())
    }

    fn deserialize(mut reader: impl std::io::Read, database: DatabaseRef) -> std::io::Result<Self> {
        let configuration: Configuration = bincode::deserialize_from(&mut reader)
            .map_err(|_| std::io::Error::new(ErrorKind::Other, "oh no!"))?;
        let heights = database::Vec::deserialize(&mut reader, database.clone())?;
        let npcs = NPCVec::deserialize(&mut reader, database.clone())?;
        let pcs = PCVec::deserialize(&mut reader, database.clone())?;
        let players = PlayerVec::deserialize(&mut reader, database.clone())?;
        let player_index = Tree::deserialize(&mut reader, database.clone())?;
        let size = (configuration.size as usize).pow(2);
        let mut regions = Vec::with_capacity(size);
        for _ in 0..size {
            regions.push(Region::deserialize(&mut reader, database.clone())?)
        }
        Ok(Self {
            heights,
            npcs,
            pcs,
            players,
            player_index,
            configuration,
            regions,
        })
    }
}
