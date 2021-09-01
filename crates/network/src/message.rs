use std::marker::PhantomData;

use bincode::serialize_into;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::sync::oneshot;

use crate::ValueSender;

#[derive(Debug)]
pub enum RawMessage {
    Bi(Bytes, oneshot::Sender<Bytes>),
    Uni(Bytes),
    Datagram(Bytes),
}

impl RawMessage {
    pub fn datagram<T: Serialize>(id: u32, payload: &T) -> Self {
        let mut data = BytesMut::new();
        data.put_u32(id);
        let mut writer = data.writer();
        serialize_into(&mut writer, payload).unwrap();
        Self::Datagram(writer.into_inner().freeze())
    }

    pub fn uni<T: Serialize>(id: u32, payload: &T) -> Self {
        let mut data = BytesMut::new();
        data.put_u32(id);
        let mut writer = data.writer();
        serialize_into(&mut writer, payload).unwrap();
        Self::Uni(writer.into_inner().freeze())
    }

    pub fn bi<T: Serialize, U>(id: u32, payload: &T, sender: ValueSender<U>) -> Self {
        let mut data = BytesMut::new();
        data.put_u32(id);
        let mut writer = data.writer();
        serialize_into(&mut writer, payload).unwrap();
        Self::Bi(writer.into_inner().freeze(), sender.0)
    }

    pub fn try_clone(&self) -> Option<Self> {
        match self {
            RawMessage::Bi(_, _) => None,
            RawMessage::Uni(data) => Some(Self::Uni(data.clone())),
            RawMessage::Datagram(data) => Some(Self::Datagram(data.clone())),
        }
    }

    pub fn data_mut(&mut self) -> &mut Bytes {
        match self {
            RawMessage::Bi(data, _) => data,
            RawMessage::Uni(data) => data,
            RawMessage::Datagram(data) => data,
        }
    }

    pub fn data(&self) -> &Bytes {
        match self {
            RawMessage::Bi(data, _) => data,
            RawMessage::Uni(data) => data,
            RawMessage::Datagram(data) => data,
        }
    }

    pub fn read_u32(&mut self) -> u32 {
        self.data_mut().get_u32()
    }

    pub fn deserialize<T: DeserializeOwned>(&self) -> Result<T, bincode::Error> {
        bincode::deserialize(self.data())
    }

    pub fn sender<T>(self) -> Result<ValueSender<T>, DeserializeError> {
        match self {
            RawMessage::Bi(_, sender) => Ok(ValueSender::new(sender)),
            RawMessage::Uni(_) => Err(DeserializeError::MissingSender),
            RawMessage::Datagram(_) => Err(DeserializeError::MissingSender),
        }
    }
}

impl Clone for RawMessage {
    fn clone(&self) -> Self {
        self.try_clone().unwrap()
    }
}

pub trait SerializeMessage {
    fn serialize(self) -> RawMessage;
}

#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error(transparent)]
    Bincode(#[from] bincode::Error),
    #[error("message has no return sender")]
    MissingSender,
    #[error("invalid message id ({0})")]
    InvalidMessageId(u32),
}

pub trait TryDeserializeMessage: Sized {
    fn try_deserialize(message: RawMessage) -> Result<Self, DeserializeError>;
}

pub struct Message<T>(pub(crate) RawMessage, PhantomData<T>);

impl<T> Message<T> {
    pub(crate) fn new(raw: RawMessage) -> Self {
        Self(raw, PhantomData)
    }

    pub fn size(&self) -> usize {
        self.0.data().len()
    }
}

impl<T: SerializeMessage> From<T> for Message<T> {
    fn from(value: T) -> Self {
        Self(value.serialize(), PhantomData)
    }
}

impl<T: TryDeserializeMessage> Message<T> {
    pub fn try_into(self) -> Result<T, DeserializeError> {
        T::try_deserialize(self.0)
    }
}

impl<T> Clone for Message<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}
