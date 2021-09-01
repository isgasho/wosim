mod context;

pub use context::*;
use protocol::WorldEnter;

use crate::root::RootContext;

pub struct Game {
    pub context: GameContext,
}

impl Game {
    pub fn new(root_context: &RootContext, enter: WorldEnter) -> eyre::Result<Self> {
        let context = GameContext::new(root_context, enter)?;
        Ok(Self { context })
    }
}
