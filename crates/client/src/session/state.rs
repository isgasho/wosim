use std::time::Instant;

use protocol::PlayerSlots;
use util::handle::HandleFlow;

use vulkan::RenderPass;
use winit::event::Event;

use crate::{
    game::Game,
    root::{RootContext, RootFrame, RootSurface},
};

use super::SessionContext;

#[allow(clippy::large_enum_variant)]
pub enum SessionState {
    Lobby { slots: PlayerSlots },
    InGame(Game),
}

impl SessionState {
    pub fn handle_event(&mut self, event: &Event<()>, grab: bool) -> HandleFlow {
        match self {
            SessionState::InGame(game) => game.context.handle_event(event, grab),
            _ => HandleFlow::Unhandled,
        }
    }

    pub fn update(&mut self, now: Instant, context: &SessionContext) {
        if let SessionState::InGame(game) = self {
            game.context.update(now, &context.connection)
        }
    }

    pub fn prepare_render(
        &mut self,
        root_context: &RootContext,
        root_surface: &RootSurface,
        root_frame: &mut RootFrame,
    ) -> eyre::Result<()> {
        match self {
            SessionState::InGame(game) => {
                game.context
                    .prepare_render(root_context, root_surface, root_frame)
            }
            _ => Ok(()),
        }
    }

    pub fn render(
        &mut self,
        root_context: &RootContext,
        render_pass: &RenderPass,
        root_frame: &mut RootFrame,
        pre_pass: bool,
    ) -> Result<(), vulkan::Error> {
        if let SessionState::InGame(game) = self {
            game.context
                .render(root_context, render_pass, root_frame, pre_pass)
        } else {
            Ok(())
        }
    }
}
