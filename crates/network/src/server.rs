use std::{io, net::SocketAddr, str::Utf8Error, sync::Arc};

use futures::StreamExt;
use libmdns::{Responder, Service};
use quinn::{
    crypto::rustls::TLSError, CertificateChain, Connecting, ConnectionError, Incoming,
    NewConnection, PrivateKey, ReadToEndError, ServerConfig, ServerConfigBuilder, TransportConfig,
    VarInt,
};
use thiserror::Error;
use tokio::spawn;
use tracing::error;

use crate::{authenticator::Authenticator, incoming, RawConnection};
pub struct Endpoint {
    _inner: quinn::Endpoint,
    address: SocketAddr,
    _service: Option<Service>,
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Endpoint {{ _inner: {:?}, address: {:?} }}",
            self._inner, self.address
        )
    }
}

#[derive(Debug, Error)]
pub enum EndpointError {
    #[error("invalid certificate / private key pair")]
    InvalidCertificate(#[source] TLSError),
    #[error("could not bind to address")]
    Bind(#[source] quinn::EndpointError),
    #[error("could not get local address")]
    LocalAddress(#[source] io::Error),
}

impl Endpoint {
    pub fn new(configuration: EndpointConfiguration) -> Result<Self, EndpointError> {
        let mut server_config = ServerConfig::default();
        server_config.transport = Arc::new(configuration.transport_config);
        let mut server_config = ServerConfigBuilder::new(server_config);
        server_config.protocols(&[configuration.protocol.as_bytes()]);
        server_config
            .certificate(configuration.certificate_chain, configuration.private_key)
            .map_err(EndpointError::InvalidCertificate)?;
        let server_config = server_config.build();
        let mut endpoint = quinn::Endpoint::builder();
        endpoint.listen(server_config);
        let (_inner, incoming) = endpoint
            .bind(&local_address(configuration.port))
            .map_err(EndpointError::Bind)?;
        let address = _inner.local_addr().map_err(EndpointError::LocalAddress)?;
        spawn(listen(
            incoming,
            configuration.authenticator,
            configuration.token_size_limit,
            configuration.size_limit,
        ));
        let _service = if let Some(mdns) = configuration.mdns {
            let (responder, task) = Responder::with_default_handle().unwrap();
            spawn(task);
            Some(responder.register(
                format!("_{}._udp", mdns.r#type),
                format!("{}-{}", mdns.r#type, address.port()),
                address.port(),
                &[&configuration.protocol, &mdns.name],
            ))
        } else {
            None
        };
        Ok(Self {
            _inner,
            address,
            _service,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }
}

pub struct EndpointConfiguration {
    pub port: u16,
    pub protocol: String,
    pub certificate_chain: CertificateChain,
    pub private_key: PrivateKey,
    pub transport_config: TransportConfig,
    pub token_size_limit: usize,
    pub size_limit: usize,
    pub mdns: Option<MdnsConfiguration>,
    pub authenticator: Arc<dyn Authenticator>,
}

pub struct MdnsConfiguration {
    pub r#type: String,
    pub name: String,
    pub protected: bool,
}

async fn listen(
    mut incoming: Incoming,
    authenticator: Arc<dyn Authenticator>,
    token_size_limit: usize,
    size_limit: usize,
) {
    while let Some(connecting) = incoming.next().await {
        let authenticator = authenticator.clone();
        spawn(async move {
            if let Err(error) =
                accept(connecting, authenticator, token_size_limit, size_limit).await
            {
                error!("{:?}", error)
            }
        });
    }
}

#[derive(Debug, Error)]
enum AcceptError {
    #[error(transparent)]
    Connection(#[from] ConnectionError),
    #[error("no token stream found")]
    NoTokenStream,
    #[error(transparent)]
    ReadToEnd(#[from] ReadToEndError),
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),
}

async fn accept(
    connecting: Connecting,
    authenticator: Arc<dyn Authenticator>,
    token_size_limit: usize,
    size_limit: usize,
) -> Result<(), AcceptError> {
    let NewConnection {
        connection,
        bi_streams,
        mut uni_streams,
        datagrams,
        ..
    } = connecting.await?;
    let recv = uni_streams
        .next()
        .await
        .ok_or(AcceptError::NoTokenStream)??;
    let buf = recv.read_to_end(token_size_limit).await?;
    let token = std::str::from_utf8(&buf)?;
    let connection = Arc::new(RawConnection::new(connection, size_limit));
    let sender = match authenticator.authenticate(token, connection.clone()) {
        Ok(sender) => sender,
        Err(reason) => {
            connection.close(VarInt::from_u32(1), reason.as_bytes());
            return Err(AcceptError::AuthenticationFailed(reason));
        }
    };
    drop(connection);
    incoming(bi_streams, uni_streams, datagrams, sender, size_limit).await;
    Ok(())
}

#[cfg(windows)]
pub fn local_address(port: u16) -> SocketAddr {
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), port)
}

#[cfg(not(windows))]
pub fn local_address(port: u16) -> SocketAddr {
    SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), port)
}
