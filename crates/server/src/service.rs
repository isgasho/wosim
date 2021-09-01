use std::{
    io,
    time::{Duration, Instant},
};

use thiserror::Error;
use tokio::sync::mpsc;

use crate::{handle_request, world::ServerWorld, Action};

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("could not setup world")]
    SetupWorld(#[source] io::Error),
    #[error("could not create snapshot")]
    Snaphot(#[source] io::Error),
}

pub(crate) async fn run(
    mut actions: mpsc::Receiver<Action>,
    tick_start: Instant,
    tick_period: Duration,
) -> Result<(), ServiceError> {
    let mut world = ServerWorld::new(tick_start, tick_period).map_err(ServiceError::SetupWorld)?;
    while let Some(action) = actions.recv().await {
        match action {
            Action::Connected(user) => {
                world.persistent.initialize_player(user.uuid);
            }
            Action::Disconnected(user) => {
                if let Some(observer) = world.observers.remove(&user.uuid) {
                    observer.remove(
                        &mut world.regions,
                        &mut world.persistent,
                        &mut world.transient,
                        &mut world.physics,
                    );
                }
            }
            Action::Request(user, request) => {
                if let Err(error) = handle_request(request, &mut world, &user).await {
                    user.connection
                        .close(error.code(), error.reason().as_bytes())
                }
            }
            Action::Stop => break,
            Action::Tick => world.tick().await,
        }
    }
    world.snapshot().map_err(ServiceError::Snaphot)?;
    Ok(())
}
