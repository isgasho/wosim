use network::Connection;
use protocol::Notification;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct User {
    pub uuid: u128,
    pub name: String,
    pub connection: Connection<Notification>,
    pub role: Role,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Role {
    Admin,
    Player,
    Guest,
}
