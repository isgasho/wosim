use std::io;

use bytes::BytesMut;
use futures::{future::join_all, StreamExt};
use quinn::{
    Datagrams, IncomingBiStreams, IncomingUniStreams, ReadExactError, ReadToEndError, RecvStream,
    SendStream, WriteError,
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    spawn,
    sync::{mpsc, oneshot},
};
use tracing::error;

use crate::{RawMessage, RawMessageSender};

pub async fn incoming(
    bi_streams: IncomingBiStreams,
    uni_streams: IncomingUniStreams,
    datagrams: Datagrams,
    sender: RawMessageSender,
    size_limit: usize,
) {
    join_all(vec![
        spawn(self::bi_streams(bi_streams, sender.clone(), size_limit)),
        spawn(self::uni_streams(uni_streams, sender.clone(), size_limit)),
        spawn(self::datagrams(datagrams, sender.clone())),
    ])
    .await;
}

async fn bi_streams(
    mut bi_streams: IncomingBiStreams,
    sender: RawMessageSender,
    size_limit: usize,
) {
    while let Some(Ok((send, recv))) = bi_streams.next().await {
        let sender = sender.clone();
        spawn(async move {
            if let Err(error) = bi_stream(send, recv, sender, size_limit).await {
                error!("{:?}", error);
            }
        });
    }
}

async fn uni_streams(
    mut uni_streams: IncomingUniStreams,
    sender: RawMessageSender,
    size_limit: usize,
) {
    while let Some(Ok(recv)) = uni_streams.next().await {
        let sender = sender.clone();
        spawn(async move {
            if let Err(error) = uni_stream(recv, size_limit, sender).await {
                error!("{:?}", error);
            }
        });
    }
}

async fn datagrams(mut datagrams: Datagrams, sender: RawMessageSender) {
    while let Some(Ok(datagram)) = datagrams.next().await {
        let sender = sender.clone();
        spawn(async move {
            if let Err(error) = sender.send(RawMessage::Datagram(datagram)).await {
                error!("{:?}", error);
            }
        });
    }
}

async fn bi_stream(
    mut tx: SendStream,
    mut rx: RecvStream,
    sender: RawMessageSender,
    size_limit: usize,
) -> Result<(), RecvError> {
    let tag = rx.read_u32_le().await?;
    if tag == u32::MAX {
        loop {
            let tag = rx.read_u32_le().await?;
            if tag == u32::MAX {
                break;
            }
            let tag_type = tag & 0xc0000000;
            let size = (tag & 0x3fffffff) as usize;
            if size > size_limit {
                return Err(RecvError::SizeTooLarge(size, size_limit));
            }
            let mut buf = BytesMut::with_capacity(size);
            unsafe { buf.set_len(size) };
            rx.read_exact(&mut buf).await?;
            if tag_type == 0x00000000 {
                let (send, recv) = oneshot::channel();
                sender.send(RawMessage::Bi(buf.freeze(), send)).await?;
                let data = recv.await?;
                tx.write_u32_le(data.len() as u32).await?;
                tx.write_all(&data).await?;
            } else if tag_type == 0x40000000 {
                sender.send(RawMessage::Uni(buf.freeze())).await?;
            } else {
                sender.send(RawMessage::Datagram(buf.freeze())).await?;
            }
        }
    } else {
        let data = rx.read_to_end(size_limit).await?.into();
        let (send, recv) = oneshot::channel();
        sender.send(RawMessage::Bi(data, send)).await?;
        let data = recv.await?;
        tx.write_all(&data).await?;
    }
    tx.finish().await?;
    Ok(())
}

async fn uni_stream(
    recv: RecvStream,
    size_limit: usize,
    sender: RawMessageSender,
) -> Result<(), RecvError> {
    let buf = recv.read_to_end(size_limit).await?.into();
    sender.send(RawMessage::Uni(buf)).await?;
    Ok(())
}

#[derive(Debug, Error)]
enum RecvError {
    #[error(transparent)]
    ReadToEnd(#[from] ReadToEndError),
    #[error(transparent)]
    ReadExact(#[from] ReadExactError),
    #[error(transparent)]
    Write(#[from] WriteError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("could not receive return message")]
    RecvReturn(#[from] oneshot::error::RecvError),
    #[error("could not send return message")]
    SendReturn(#[from] mpsc::error::SendError<RawMessage>),
    #[error("message size ({0}) is larger than allowed ({1})")]
    SizeTooLarge(usize, usize),
}
