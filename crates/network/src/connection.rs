use std::{io, marker::PhantomData, sync::Arc, usize};

use bytes::{Bytes, BytesMut};
use quinn::{
    ConnectionError, ReadExactError, ReadToEndError, SendDatagramError, VarInt, WriteError,
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    spawn,
    sync::mpsc,
    task::JoinHandle,
};

use crate::{message_channel, ConnectionStats, Message, MessageSender, RawMessage};

#[derive(Debug)]
pub struct RawConnection {
    inner: quinn::Connection,
    size_limit: usize,
}

impl RawConnection {
    pub fn new(inner: quinn::Connection, size_limit: usize) -> Self {
        Self { inner, size_limit }
    }

    pub fn close(&self, error_code: VarInt, reason: &[u8]) {
        self.inner.close(error_code, reason);
    }
}

const MASK_BI: u32 = 0x80000000;
const MASK_UNI: u32 = 0x40000000;
const MASK_DATAGRAM: u32 = 0x00000000;

#[derive(Debug, Error)]
pub enum SendError {
    #[error("could not open stream")]
    Open(#[source] ConnectionError),
    #[error(transparent)]
    Write(#[from] WriteError),
    #[error(transparent)]
    ReadToEnd(#[from] ReadToEndError),
    #[error(transparent)]
    ReadExact(#[from] ReadExactError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    SendDatagram(#[from] SendDatagramError),
    #[error("could not return bytes")]
    Return(Bytes),
    #[error("message size ({0}) is larger than allowed ({1})")]
    SizeTooLarge(usize, usize),
}

impl RawConnection {
    async fn send(&self, message: RawMessage) -> Result<(), SendError> {
        match message {
            RawMessage::Bi(data, sender) => {
                let (mut tx, rx) = self.inner.open_bi().await.map_err(SendError::Open)?;
                tx.write_u32_le(0).await?;
                tx.write_all(&data).await?;
                tx.finish().await?;
                let data = rx.read_to_end(self.size_limit).await?.into();
                sender.send(data).map_err(SendError::Return)?;
            }
            RawMessage::Uni(data) => {
                let mut tx = self.inner.open_uni().await.map_err(SendError::Open)?;
                tx.write_all(&data).await?;
                tx.finish().await?;
            }
            RawMessage::Datagram(data) => {
                self.inner.send_datagram(data)?;
            }
        }
        Ok(())
    }

    async fn send_all(&self, mut messages: mpsc::Receiver<RawMessage>) -> Result<(), SendError> {
        let (mut tx, mut rx) = self.inner.open_bi().await.map_err(SendError::Open)?;
        tx.write_u32_le(u32::MAX).await?;
        while let Some(message) = messages.recv().await {
            match message {
                RawMessage::Bi(data, sender) => {
                    tx.write_u32_le(data.len() as u32 | MASK_BI).await?;
                    tx.write_all(&data).await?;
                    let size = rx.read_u32_le().await? as usize;
                    if size > self.size_limit {
                        return Err(SendError::SizeTooLarge(size, self.size_limit));
                    }
                    let mut data = BytesMut::with_capacity(size);
                    unsafe { data.set_len(size) };
                    rx.read_exact(&mut data).await?;
                    sender.send(data.freeze()).map_err(SendError::Return)?;
                }
                RawMessage::Uni(data) => {
                    tx.write_u32_le(data.len() as u32 | MASK_UNI).await?;
                    tx.write_all(&data).await?;
                }
                RawMessage::Datagram(data) => {
                    tx.write_u32_le(data.len() as u32 | MASK_DATAGRAM).await?;
                    tx.write_all(&data).await?;
                }
            }
        }
        tx.write_u32_le(u32::MAX).await?;
        tx.finish().await?;
        Ok(())
    }
}

pub struct Connection<T>(Arc<RawConnection>, PhantomData<T>);

impl<T> Connection<T> {
    pub fn new(raw: Arc<RawConnection>) -> Self {
        Self(raw, PhantomData)
    }

    pub async fn send(&self, message: Message<T>) -> Result<(), SendError> {
        let raw_connection = self.0.clone();
        let raw_message = message.0;
        raw_connection.send(raw_message).await
    }

    pub fn spawn_send(&self, message: Message<T>) -> JoinHandle<Result<(), SendError>> {
        let raw_connection = self.0.clone();
        let raw_message = message.0;
        spawn(async move { raw_connection.send(raw_message).await })
    }

    pub fn channel(&self, buffer: usize) -> (MessageSender<T>, JoinHandle<Result<(), SendError>>) {
        let (sender, receiver) = message_channel(buffer);
        let raw_connection = self.0.clone();
        let raw_messages = receiver.0;
        (
            sender,
            spawn(async move { raw_connection.send_all(raw_messages).await }),
        )
    }

    pub fn stats(&self) -> ConnectionStats {
        self.0.inner.stats().into()
    }

    pub fn close(&self, error_code: VarInt, reason: &[u8]) {
        self.0.close(error_code, reason);
    }
}

impl<T> Clone for Connection<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<T> std::fmt::Debug for Connection<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Drop for RawConnection {
    fn drop(&mut self) {
        self.inner.close(VarInt::from_u32(0), &[]);
    }
}
