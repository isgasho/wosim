mod context;
mod state;

pub use context::*;
use network::Connection;
use protocol::Request;
use server::Server;
pub use state::*;
use tokio::task::JoinHandle;

pub struct Session {
    pub state: SessionState,
    pub context: SessionContext,
}

impl Session {
    pub fn new(
        connection: Connection<Request>,
        task: JoinHandle<()>,
        server: Option<Server>,
        state: SessionState,
    ) -> Self {
        let context = SessionContext::new(connection, task, server);
        Self { state, context }
    }
}
