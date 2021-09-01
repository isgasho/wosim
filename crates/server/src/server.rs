use std::{net::SocketAddr, sync::Arc, time::Duration};

use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation};
use network::{
    message_channel, self_signed,
    server::{self, Endpoint, EndpointConfiguration, EndpointError, MdnsConfiguration},
    Authenticator, MessageSender, RawConnection, RawMessageSender,
};
use protocol::{Request, ALPN_ID, MDNS_TYPE};
use quinn::{CertificateChain, PrivateKey, TransportConfig};
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use tokio::{
    spawn,
    sync::mpsc,
    task::JoinHandle,
    time::{interval_at, Instant},
};
use tracing::error;

use crate::{run, user::Role, Action, ServiceError, User};

#[derive(Debug)]
pub struct Server {
    endpoint: server::Endpoint,
    sender: mpsc::Sender<Action>,
    task: Option<JoinHandle<Result<(), ServiceError>>>,
}

impl Server {
    pub fn new(configuration: ServerConfiguration) -> Result<Self, EndpointError> {
        let (sender, receiver) = mpsc::channel(configuration.action_buffer);
        let ((certificate_chain, private_key), mdns, authenticator) = match configuration.r#type {
            ServerType::Invisible { secret } => (
                self_signed(),
                None,
                Arc::new(InvisibleAuthenticator {
                    secret,
                    sender: sender.clone(),
                    buffer: configuration.request_buffer,
                }) as Arc<dyn Authenticator>,
            ),
            ServerType::Visible {
                secret,
                name,
                password,
            } => (
                self_signed(),
                Some(MdnsConfiguration {
                    name,
                    r#type: MDNS_TYPE.to_owned(),
                    protected: password.is_some(),
                }),
                Arc::new(VisibleAuthenticator {
                    secret,
                    password,
                    sender: sender.clone(),
                    buffer: configuration.request_buffer,
                }) as Arc<dyn Authenticator>,
            ),
            ServerType::Dedicated {
                decoding_key,
                certificate_chain,
                private_key,
            } => (
                (certificate_chain, private_key),
                None,
                Arc::new(DedicatedAuthenticator {
                    decoding_key,
                    buffer: configuration.request_buffer,
                    sender: sender.clone(),
                }) as Arc<dyn Authenticator>,
            ),
        };
        let endpoint = Endpoint::new(EndpointConfiguration {
            authenticator,
            certificate_chain,
            private_key,
            mdns,
            port: configuration.port,
            protocol: ALPN_ID.to_owned(),
            token_size_limit: 4096,
            size_limit: 4096 * 4096,
            transport_config: TransportConfig::default(),
        })?;
        let tick_start = Instant::now();
        {
            let sender = sender.clone();
            let tick_start = tick_start;
            let tick_period = configuration.tick_period;
            spawn(async move {
                let mut interval = interval_at(tick_start, tick_period);
                loop {
                    interval.tick().await;
                    if sender.send(Action::Tick).await.is_err() {
                        break;
                    }
                }
            });
        }
        let task = Some(spawn(run(
            receiver,
            tick_start.into_std(),
            configuration.tick_period,
        )));
        Ok(Self {
            endpoint,
            sender,
            task,
        })
    }

    pub async fn stop(&mut self) -> Result<(), ServiceError> {
        if let Some(task) = self.task.take() {
            self.sender.send(Action::Stop).await.unwrap();
            task.await.unwrap()
        } else {
            Ok(())
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.endpoint.address()
    }
}

pub enum ServerType {
    Invisible {
        secret: String,
    },
    Visible {
        secret: String,
        name: String,
        password: Option<String>,
    },
    Dedicated {
        decoding_key: DecodingKey<'static>,
        certificate_chain: CertificateChain,
        private_key: PrivateKey,
    },
}

pub struct ServerConfiguration {
    pub port: u16,
    pub r#type: ServerType,
    pub action_buffer: usize,
    pub request_buffer: usize,
    pub tick_period: Duration,
}

struct InvisibleAuthenticator {
    secret: String,
    sender: mpsc::Sender<Action>,
    buffer: usize,
}

struct VisibleAuthenticator {
    secret: String,
    password: Option<String>,
    sender: mpsc::Sender<Action>,
    buffer: usize,
}

struct DedicatedAuthenticator {
    decoding_key: DecodingKey<'static>,
    sender: mpsc::Sender<Action>,
    buffer: usize,
}

impl Authenticator for InvisibleAuthenticator {
    fn authenticate(
        &self,
        token: &str,
        connection: Arc<RawConnection>,
    ) -> Result<RawMessageSender, String> {
        let token: Token = from_str(token).map_err(|_| "token could not be parsed".to_owned())?;
        let secret = token
            .secret
            .ok_or_else(|| "token must provide the secret".to_owned())?;
        if secret != self.secret {
            return Err("provided secret is wrong".to_owned());
        }
        let user = User {
            uuid: token.uuid,
            name: token.username,
            role: Role::Admin,
            connection: network::Connection::new(connection),
        };
        Ok(session(user, self.sender.clone(), self.buffer).into_inner())
    }
}

impl Authenticator for VisibleAuthenticator {
    fn authenticate(
        &self,
        token: &str,
        connection: Arc<RawConnection>,
    ) -> Result<RawMessageSender, String> {
        let token: Token = from_str(token).map_err(|_| "token could not be parsed".to_owned())?;
        let role = if let Some(secret) = token.secret {
            if secret == self.secret {
                Role::Admin
            } else {
                return Err("provided secret is wrong".to_owned());
            }
        } else if let Some(password) = self.password.as_ref() {
            if let Some(given_password) = token.password {
                if &given_password == password {
                    Role::Player
                } else {
                    return Err("provided password is wrong".to_owned());
                }
            } else {
                return Err("no password is provided".to_owned());
            }
        } else {
            Role::Guest
        };
        let user = User {
            uuid: token.uuid,
            name: token.username,
            role,
            connection: network::Connection::new(connection),
        };
        Ok(session(user, self.sender.clone(), self.buffer).into_inner())
    }
}

impl Authenticator for DedicatedAuthenticator {
    fn authenticate(
        &self,
        token: &str,
        connection: Arc<RawConnection>,
    ) -> Result<RawMessageSender, String> {
        let token: TokenData<Claims> = jsonwebtoken::decode(
            token,
            &self.decoding_key,
            &Validation::new(Algorithm::RS256),
        )
        .map_err(|e| format!("token could not be decoded: {}", e))?;
        let user = User {
            uuid: token.claims.uuid,
            name: token.claims.username,
            role: token.claims.role,
            connection: network::Connection::new(connection),
        };
        Ok(session(user, self.sender.clone(), self.buffer).into_inner())
    }
}

fn session(user: User, sender: mpsc::Sender<Action>, buffer: usize) -> MessageSender<Request> {
    let (send, mut recv) = message_channel(buffer);
    spawn(async move {
        if let Err(error) = sender.send(Action::Connected(user.clone())).await {
            error!("{}", error);
            return;
        }
        while let Some(message) = recv.recv().await {
            let request = match message.try_into() {
                Ok(request) => request,
                Err(error) => {
                    error!("{}", error);
                    break;
                }
            };
            if let Request::Disconnect = request {
                break;
            }
            if let Err(error) = sender.send(Action::Request(user.clone(), request)).await {
                error!("{}", error);
                return;
            }
        }
        if let Err(error) = sender.send(Action::Disconnected(user.clone())).await {
            error!("{}", error)
        }
    });
    send
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Token {
    pub uuid: u128,
    pub username: String,
    pub password: Option<String>,
    pub secret: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    uuid: u128,
    username: String,
    role: Role,
}
