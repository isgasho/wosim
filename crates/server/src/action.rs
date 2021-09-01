use protocol::Request;

use crate::User;

#[derive(Debug)]
pub enum Action {
    Connected(User),
    Disconnected(User),
    Request(User, Request),
    Stop,
    Tick,
}
