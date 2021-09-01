use std::{fmt::Debug, marker::PhantomData};

use bincode::{deserialize_from, serialize_into};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::{Message, RawMessage};

pub struct ValueSender<T>(pub(crate) oneshot::Sender<Bytes>, PhantomData<T>);

impl<T> Debug for ValueSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug)]
pub struct ValueReceiver<T>(pub(crate) oneshot::Receiver<Bytes>, PhantomData<T>);

#[derive(Debug)]
pub struct MessageSender<T>(pub(crate) mpsc::Sender<RawMessage>, PhantomData<T>);

impl<T> Clone for MessageSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

pub struct MessageReceiver<T>(pub(crate) mpsc::Receiver<RawMessage>, PhantomData<T>);

impl<T> ValueSender<T> {
    pub(crate) fn new(sender: oneshot::Sender<Bytes>) -> Self {
        Self(sender, PhantomData)
    }
}

impl<T: Serialize> ValueSender<T> {
    pub fn send(self, value: T) -> Result<(), T> {
        let data = BytesMut::new();
        let mut writer = data.writer();
        serialize_into(&mut writer, &value).unwrap();
        match self.0.send(writer.into_inner().freeze()) {
            Ok(()) => Ok(()),
            Err(_) => Err(value),
        }
    }
}

impl<T> MessageSender<T> {
    pub async fn send(&self, message: Message<T>) -> Result<(), Message<T>> {
        match self.0.send(message.0).await {
            Ok(()) => Ok(()),
            Err(error) => Err(Message::new(error.0)),
        }
    }

    pub fn into_inner(self) -> RawMessageSender {
        self.0
    }
}

impl<T> MessageReceiver<T> {
    pub fn new(inner: RawMessageReceiver) -> Self {
        Self(inner, PhantomData)
    }

    pub async fn recv(&mut self) -> Option<Message<T>> {
        self.0.recv().await.map(Message::new)
    }
}

pub type RawMessageSender = mpsc::Sender<RawMessage>;
pub type RawMessageReceiver = mpsc::Receiver<RawMessage>;

#[derive(Debug, Error)]
pub enum RecvError {
    #[error("could not deserialize value")]
    Deserialize(#[from] bincode::Error),
    #[error("sending half already dropped")]
    Closed,
}

impl<T: DeserializeOwned> ValueReceiver<T> {
    pub async fn recv(self) -> Result<T, RecvError> {
        match self.0.await {
            Ok(data) => Ok(deserialize_from(data.reader())?),
            Err(_) => Err(RecvError::Closed),
        }
    }
}

pub fn value_channel<T>() -> (ValueSender<T>, ValueReceiver<T>) {
    let (sender, receiver) = oneshot::channel();
    (
        ValueSender(sender, PhantomData),
        ValueReceiver(receiver, PhantomData),
    )
}

pub fn message_channel<T>(buffer: usize) -> (MessageSender<T>, MessageReceiver<T>) {
    let (sender, receiver) = raw_message_channel(buffer);
    (
        MessageSender(sender, PhantomData),
        MessageReceiver(receiver, PhantomData),
    )
}

pub fn raw_message_channel(buffer: usize) -> (RawMessageSender, RawMessageReceiver) {
    mpsc::channel(buffer)
}
