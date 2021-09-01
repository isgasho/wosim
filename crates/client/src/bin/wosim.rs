use std::net::SocketAddr;
use std::time::Duration;

use client::action::Action;
use client::run::run;
use client::state::InitialState;
use network::client::{Endpoint, EndpointConfiguration};
use network::{value_channel, Connection, Message, TransportConfig, Verification};
use protocol::{Request, ALPN_ID};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use server::{Server, ServerConfiguration, ServerType, Token};
use structopt::StructOpt;
use tokio::{runtime::Runtime, spawn};
use uuid::Uuid;
use winit::event_loop::EventLoop;

#[derive(StructOpt)]
#[structopt(name = "wosim")]
enum Command {
    Join(JoinCommand),
    Play {
        #[structopt(long, default_value)]
        uuid: Uuid,
        #[structopt(long)]
        username: Option<String>,
        #[structopt(long, short, default_value)]
        port: u16,
        #[structopt(long)]
        password: Option<String>,
        #[structopt(short, long)]
        name: Option<String>,
    },
    Create {
        #[structopt(short, long)]
        delete: bool,
    },
}

#[derive(StructOpt)]
enum JoinCommand {
    Direct {
        port: u16,
        uuid: Uuid,
        password: Option<String>,
        #[structopt(long)]
        username: Option<String>,
        #[structopt(long, short, default_value = "localhost")]
        hostname: String,
        #[structopt(long)]
        verify: bool,
    },
    Token {
        hostname: String,
        port: u16,
        token: String,
        #[structopt(long)]
        skip_verification: bool,
    },
}

impl Command {
    fn run(self) -> eyre::Result<()> {
        let event_loop = EventLoop::with_user_event();
        let runtime = Runtime::new()?;
        let _guard = runtime.enter();
        let initial_state = match self {
            Command::Join(command) => {
                let (hostname, port, token, skip_verification) = match command {
                    JoinCommand::Direct {
                        port,
                        uuid,
                        password,
                        username,
                        hostname,
                        verify,
                    } => {
                        let token = serde_json::to_string(&Token {
                            password,
                            secret: None,
                            username: username.unwrap_or_else(whoami::username),
                            uuid: uuid.as_u128(),
                        })
                        .unwrap();
                        (hostname, port, token, !verify)
                    }
                    JoinCommand::Token {
                        hostname,
                        port,
                        token,
                        skip_verification,
                    } => (hostname, port, token, skip_verification),
                };
                let proxy = event_loop.create_proxy();
                let task = spawn(async move {
                    let verification = if skip_verification {
                        Verification::Skip
                    } else {
                        Verification::CertificateAuthorities(Vec::new())
                    };
                    let mut transport_config = TransportConfig::default();
                    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
                    let endpoint = match Endpoint::new(EndpointConfiguration {
                        hostname,
                        protocol: ALPN_ID.to_owned(),
                        port,
                        token,
                        transport_config,
                        verification,
                        buffer: 16,
                        size_limit: 4096 * 4096,
                    })
                    .await
                    {
                        Ok(endpoint) => endpoint,
                        Err(error) => {
                            proxy
                                .send_event(Action::Error(eyre::Error::new(error)))
                                .unwrap();
                            return;
                        }
                    };
                    let (sender, receiver) = value_channel();
                    match Connection::new(endpoint.connection.clone())
                        .send(Message::from(Request::WorldInfo(sender)))
                        .await
                    {
                        Ok(()) => {}
                        Err(error) => {
                            proxy
                                .send_event(Action::Error(eyre::Error::new(error)))
                                .unwrap();
                            return;
                        }
                    }
                    let info = match receiver.recv().await {
                        Ok(info) => info,
                        Err(error) => {
                            proxy
                                .send_event(Action::Error(eyre::Error::new(error)))
                                .unwrap();
                            return;
                        }
                    };
                    proxy
                        .send_event(Action::Connected(endpoint, info, None))
                        .unwrap();
                });
                InitialState::Connect(task)
            }
            Command::Create { delete } => {
                if delete {
                    let _ = std::fs::remove_file("world.db");
                }
                InitialState::Configure
            }
            Command::Play {
                uuid,
                username,
                port,
                password,
                name,
            } => {
                let secret: String = thread_rng()
                    .sample_iter(Alphanumeric)
                    .take(32)
                    .map(char::from)
                    .collect();
                let token = serde_json::to_string(&Token {
                    uuid: uuid.as_u128(),
                    username: username.unwrap_or_else(whoami::username),
                    password: None,
                    secret: Some(secret.clone()),
                })
                .unwrap();
                let server_type = if let Some(name) = name {
                    ServerType::Visible {
                        name,
                        password,
                        secret,
                    }
                } else {
                    ServerType::Invisible { secret }
                };
                let proxy = event_loop.create_proxy();
                let task = spawn(async move {
                    let server = match Server::new(ServerConfiguration {
                        action_buffer: 64,
                        request_buffer: 16,
                        port,
                        r#type: server_type,
                        tick_period: Duration::from_millis(50),
                    }) {
                        Ok(server) => server,
                        Err(error) => {
                            proxy
                                .send_event(Action::Error(eyre::Error::new(error)))
                                .unwrap();
                            return;
                        }
                    };
                    let address = local_server_address(server.address().port());
                    let mut transport_config = TransportConfig::default();
                    transport_config.keep_alive_interval(Some(Duration::from_secs(5)));
                    let endpoint = match Endpoint::new(EndpointConfiguration {
                        hostname: address.ip().to_string(),
                        protocol: ALPN_ID.to_owned(),
                        port: address.port(),
                        token,
                        transport_config,
                        verification: Verification::Skip,
                        buffer: 16,
                        size_limit: 4096 * 4096,
                    })
                    .await
                    {
                        Ok(endpoint) => endpoint,
                        Err(error) => {
                            proxy
                                .send_event(Action::Error(eyre::Error::new(error)))
                                .unwrap();
                            return;
                        }
                    };
                    let (sender, receiver) = value_channel();
                    match Connection::new(endpoint.connection.clone())
                        .send(Message::from(Request::WorldInfo(sender)))
                        .await
                    {
                        Ok(()) => {}
                        Err(error) => {
                            proxy
                                .send_event(Action::Error(eyre::Error::new(error)))
                                .unwrap();
                            return;
                        }
                    }
                    let info = match receiver.recv().await {
                        Ok(info) => info,
                        Err(error) => {
                            proxy
                                .send_event(Action::Error(eyre::Error::new(error)))
                                .unwrap();
                            return;
                        }
                    };
                    proxy
                        .send_event(Action::Connected(endpoint, info, Some(server)))
                        .unwrap();
                });
                InitialState::Connect(task)
            }
        };
        run(runtime, event_loop, initial_state)
    }
}

#[cfg(not(target_os = "macos"))]
fn setup_env() {}

#[cfg(target_os = "macos")]
fn setup_env() {
    std::env::set_var("MVK_CONFIG_FULL_IMAGE_VIEW_SWIZZLE", "1");
}

#[cfg(windows)]
pub fn local_server_address(port: u16) -> SocketAddr {
    SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), port)
}

#[cfg(not(windows))]
pub fn local_server_address(port: u16) -> SocketAddr {
    SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), port)
}

fn main() -> eyre::Result<()> {
    stable_eyre::install()?;
    setup_env();
    Command::from_args().run()
}
