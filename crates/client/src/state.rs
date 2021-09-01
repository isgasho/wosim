use tokio::task::JoinHandle;

use crate::root::RootState;

pub enum InitialState {
    Configure,
    Connect(JoinHandle<()>),
}

impl InitialState {
    pub fn create(self) -> RootState {
        match self {
            InitialState::Configure => RootState::Configure,
            InitialState::Connect(task) => RootState::Connect { task: Some(task) },
        }
    }
}
