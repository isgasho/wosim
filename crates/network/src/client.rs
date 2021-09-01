use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
    str::FromStr,
    sync::Arc,
};

use quinn::{
    ClientConfigBuilder, ConnectError, ConnectionError, NewConnection, TransportConfig, WriteError,
};
use thiserror::Error;
use tokio::spawn;

use crate::{
    incoming, raw_message_channel, verification::Verification, RawConnection, RawMessageReceiver,
};

#[derive(Debug)]
pub struct Endpoint {
    pub connection: Arc<RawConnection>,
    pub receiver: RawMessageReceiver,
}

pub struct EndpointConfiguration {
    pub hostname: String,
    pub protocol: String,
    pub port: u16,
    pub token: String,
    pub transport_config: TransportConfig,
    pub verification: Verification,
    pub buffer: usize,
    pub size_limit: usize,
}

#[derive(Debug, Error)]
pub enum EndpointError {
    #[error("could not authenticate token")]
    TokenAuthentication(String),
    #[error("certificate authority has invalid certificate")]
    InvalidCaCertificates(#[source] webpki::Error),
    #[error("could not bind to endpoint")]
    Bind(#[source] quinn::EndpointError),
    #[error("could not resolve ip address")]
    IpResolution(#[source] io::Error),
    #[error("could not find a socket address")]
    NoSocketAddrFound,
    #[error("could not connect to server")]
    Connect(#[source] ConnectError),
    #[error("could not connect to server")]
    Connecting(#[source] ConnectionError),
    #[error("could not open stream for to send token")]
    OpenTokenStream(#[source] ConnectionError),
    #[error("could not write to token stream")]
    WriteTokenStream(#[source] WriteError),
    #[error("could not finish token stream")]
    FinishTokenStream(#[source] WriteError),
}

impl Endpoint {
    pub async fn new(configuration: EndpointConfiguration) -> Result<Self, EndpointError> {
        let (sender, receiver) = raw_message_channel(configuration.buffer);
        let remote_address = (configuration.hostname.as_str(), configuration.port)
            .to_socket_addrs()
            .map_err(EndpointError::IpResolution)?
            .next()
            .ok_or(EndpointError::NoSocketAddrFound)?;
        let server_name = match IpAddr::from_str(&configuration.hostname) {
            Ok(_) => "localhost".to_string(),
            Err(_) => configuration.hostname,
        };
        let mut client_config = ClientConfigBuilder::default();
        client_config.protocols(&[configuration.protocol.as_bytes()]);
        let mut client_config = configuration
            .verification
            .apply(client_config)
            .map_err(EndpointError::InvalidCaCertificates)?;
        client_config.transport = Arc::new(configuration.transport_config);
        let mut endpoint = quinn::Endpoint::builder();
        endpoint.default_client_config(client_config);
        let local_address = match remote_address {
            SocketAddr::V4(_) => SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
            SocketAddr::V6(_) => SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0)),
        };
        let (endpoint, _) = endpoint.bind(&local_address).map_err(EndpointError::Bind)?;
        let NewConnection {
            connection,
            bi_streams,
            uni_streams,
            datagrams,
            ..
        } = endpoint
            .connect(&remote_address, &server_name)
            .map_err(EndpointError::Connect)?
            .await
            .map_err(EndpointError::Connecting)?;
        let mut send = connection
            .open_uni()
            .await
            .map_err(EndpointError::OpenTokenStream)?;
        send.write_all(configuration.token.as_bytes())
            .await
            .map_err(EndpointError::WriteTokenStream)?;
        send.finish()
            .await
            .map_err(EndpointError::FinishTokenStream)?;
        spawn(incoming(
            bi_streams,
            uni_streams,
            datagrams,
            sender,
            configuration.size_limit,
        ));
        let connection = Arc::new(RawConnection::new(connection, configuration.size_limit));
        Ok(Self {
            connection,
            receiver,
        })
    }
}
