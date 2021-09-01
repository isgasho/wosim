use serde::{Deserialize, Serialize};

use crate::{Position, RegionPos, Rotation};

#[derive(Debug, Serialize, Deserialize)]
pub enum DynamicUpdate {
    Enter(Entity, Option<RegionPos>, Position, Rotation),
    Exit(Entity, Option<RegionPos>),
    Update(Entity, Position, Rotation),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StaticUpdate {}

#[derive(Debug, Serialize, Deserialize)]
pub enum GlobalUpdate {}

#[derive(Debug, Serialize, Deserialize)]
pub enum Entity {
    NPC(u32),
    PC(u32),
}

impl From<Entity> for u128 {
    fn from(value: Entity) -> Self {
        let mut bytes = [0; 16];
        let (ty, id) = match value {
            Entity::NPC(id) => (0u32, id),
            Entity::PC(id) => (1u32, id),
        };
        bytes[0..4].copy_from_slice(&ty.to_le_bytes());
        bytes[4..8].copy_from_slice(&id.to_le_bytes());
        Self::from_le_bytes(bytes)
    }
}

impl From<u128> for Entity {
    fn from(value: u128) -> Self {
        let bytes = value.to_le_bytes();
        let mut ty_bytes = [0; 4];
        let mut id_bytes = [0; 4];
        ty_bytes.copy_from_slice(&bytes[0..4]);
        id_bytes.copy_from_slice(&bytes[4..8]);
        let ty = u32::from_le_bytes(ty_bytes);
        let id = u32::from_le_bytes(id_bytes);
        match ty {
            0 => Self::NPC(id),
            1 => Self::PC(id),
            _ => panic!(),
        }
    }
}
