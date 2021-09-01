use network::Connection;
use protocol::Request;
use server::Server;
use tokio::task::JoinHandle;

pub struct SessionContext {
    pub connection: Connection<Request>,
    pub task: Option<JoinHandle<()>>,
    pub server: Option<Server>,
}

impl SessionContext {
    pub fn new(
        connection: Connection<Request>,
        task: JoinHandle<()>,
        server: Option<Server>,
    ) -> Self {
        Self {
            connection,
            task: Some(task),
            server,
        }
    }
}
