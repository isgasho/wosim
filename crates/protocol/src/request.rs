use network::{DeserializeError, RawMessage, SerializeMessage, TryDeserializeMessage, ValueSender};

use crate::{PlayerSlots, Position, Rotation, WorldInfo};

#[derive(Debug)]
pub enum Request {
    Disconnect,
    WorldInfo(ValueSender<WorldInfo>),
    Slots(ValueSender<PlayerSlots>),
    Create(u8, ValueSender<u32>),
    Delete(u8, ValueSender<()>),
    Enter(u8),
    Exit(ValueSender<()>),
    UpdateSelf((Position, Rotation)),
}

impl SerializeMessage for Request {
    fn serialize(self) -> RawMessage {
        match self {
            Self::Disconnect => RawMessage::uni(0, &()),
            Self::WorldInfo(sender) => RawMessage::bi(1, &(), sender),
            Self::Slots(sender) => RawMessage::bi(2, &(), sender),
            Self::Create(payload, sender) => RawMessage::bi(3, &payload, sender),
            Self::Delete(payload, sender) => RawMessage::bi(4, &payload, sender),
            Self::Enter(payload) => RawMessage::uni(5, &payload),
            Self::Exit(sender) => RawMessage::bi(6, &(), sender),
            Self::UpdateSelf(payload) => RawMessage::uni(7, &payload),
        }
    }
}

impl TryDeserializeMessage for Request {
    fn try_deserialize(mut message: RawMessage) -> Result<Self, DeserializeError> {
        match message.read_u32() {
            0 => Ok(Self::Disconnect),
            1 => Ok(Self::WorldInfo(message.sender()?)),
            2 => Ok(Self::Slots(message.sender()?)),
            3 => Ok(Self::Create(message.deserialize()?, message.sender()?)),
            4 => Ok(Self::Delete(message.deserialize()?, message.sender()?)),
            5 => Ok(Self::Enter(message.deserialize()?)),
            6 => Ok(Self::Exit(message.sender()?)),
            7 => Ok(Self::UpdateSelf(message.deserialize()?)),
            id => Err(DeserializeError::InvalidMessageId(id)),
        }
    }
}
