use network::client::Endpoint;
use protocol::{Notification, PlayerSlots, WorldInfo};
use server::Server;

#[derive(Debug)]
pub enum Action {
    Create,
    Notification(Notification),
    GeneratorNotification(generator::Notification),
    GeneratorFinished,
    Log(Vec<u8>),
    Disconnected,
    Connected(Endpoint, WorldInfo, Option<Server>),
    Error(eyre::Error),
    Close,
    UpdateLobbySlots(PlayerSlots),
    UpdateLobbySlot(u8, u32),
}
