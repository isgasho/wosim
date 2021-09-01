use network::{DeserializeError, RawMessage, SerializeMessage, TryDeserializeMessage};

use crate::{
    DynamicSetup, DynamicUpdate, GlobalSetup, GlobalUpdate, RegionPos, StaticSetup, StaticUpdate,
    WorldEnter,
};

#[derive(Debug)]
pub enum Notification {
    GlobalSetup(GlobalSetup),
    StaticSetup((RegionPos, StaticSetup)),
    DynamicSetup((RegionPos, DynamicSetup, u64)),
    GlobalUpdates(Vec<GlobalUpdate>),
    StaticUpdates((RegionPos, Vec<StaticUpdate>)),
    DynamicUpdates((RegionPos, Vec<DynamicUpdate>, u64)),
    Enter(WorldEnter),
    StaticTeardown(RegionPos),
    DynamicTeardown(RegionPos),
}

impl SerializeMessage for Notification {
    fn serialize(self) -> RawMessage {
        match self {
            Self::GlobalSetup(payload) => RawMessage::uni(0, &payload),
            Self::StaticSetup(payload) => RawMessage::uni(1, &payload),
            Self::DynamicSetup(payload) => RawMessage::uni(2, &payload),
            Self::GlobalUpdates(payload) => RawMessage::uni(3, &payload),
            Self::StaticUpdates(payload) => RawMessage::uni(4, &payload),
            Self::DynamicUpdates(payload) => RawMessage::uni(5, &payload),
            Self::Enter(payload) => RawMessage::uni(6, &payload),
            Self::StaticTeardown(payload) => RawMessage::uni(7, &payload),
            Self::DynamicTeardown(payload) => RawMessage::uni(8, &payload),
        }
    }
}

impl TryDeserializeMessage for Notification {
    fn try_deserialize(mut message: RawMessage) -> Result<Self, DeserializeError> {
        match message.read_u32() {
            0 => Ok(Self::GlobalSetup(message.deserialize()?)),
            1 => Ok(Self::StaticSetup(message.deserialize()?)),
            2 => Ok(Self::DynamicSetup(message.deserialize()?)),
            3 => Ok(Self::GlobalUpdates(message.deserialize()?)),
            4 => Ok(Self::StaticUpdates(message.deserialize()?)),
            5 => Ok(Self::DynamicUpdates(message.deserialize()?)),
            6 => Ok(Self::Enter(message.deserialize()?)),
            7 => Ok(Self::StaticTeardown(message.deserialize()?)),
            8 => Ok(Self::DynamicTeardown(message.deserialize()?)),
            id => Err(DeserializeError::InvalidMessageId(id)),
        }
    }
}
