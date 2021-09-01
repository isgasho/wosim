use std::time::Instant;

use client_gpu::Object;
use egui::{CentralPanel, CtxRef, Window};
use generator::{Control, Generator};
use nalgebra::Isometry;
use network::{value_channel, Connection, Message};
use physics::{character_collider, RigidBodyType};
use protocol::{DynamicUpdate, Entity, Notification, Request, Transform};
use tokio::{spawn, task::JoinHandle};
use util::{handle::HandleFlow, interpolation::InterpolationBuffer};
use vulkan::RenderPass;
use winit::{event::Event, event_loop::EventLoopProxy};

use crate::{
    action::Action,
    character::{NPC, PC},
    game::Game,
    region::Region,
    session::{Session, SessionState},
    world::World,
};

use super::{RootContext, RootFrame, RootSurface};

#[allow(clippy::large_enum_variant)]
pub enum RootState {
    Configure,
    Connect {
        task: Option<JoinHandle<()>>,
    },
    Connected(Session),
    Report {
        error: eyre::Error,
    },
    Generate {
        task: Option<JoinHandle<()>>,
        control: Control,
        generator: Generator,
    },
    GenerateFinished,
}

impl RootState {
    pub fn prepare_render(
        &mut self,
        context: &mut RootContext,
        surface: &RootSurface,
        frame: &mut RootFrame,
    ) -> eyre::Result<()> {
        match self {
            Self::Connected(session) => session.state.prepare_render(context, surface, frame),
            _ => Ok(()),
        }
    }

    pub fn render(
        &mut self,
        context: &mut RootContext,
        render_pass: &RenderPass,
        frame: &mut RootFrame,
        pre_pass: bool,
    ) -> Result<(), vulkan::Error> {
        if let Self::Connected(session) = self {
            session.state.render(context, render_pass, frame, pre_pass)
        } else {
            Ok(())
        }
    }

    pub async fn shutdown(&mut self) -> eyre::Result<()> {
        match self {
            Self::Configure => {}
            Self::Connect { task } => {
                let task = task.take().unwrap();
                task.abort();
                drop(task.await)
            }
            Self::Connected(session) => {
                session
                    .context
                    .connection
                    .send(Message::from(Request::Disconnect))
                    .await?;
                session.context.task.take().unwrap().await?;
                if let Some(server) = session.context.server.as_mut() {
                    server.stop().await?;
                }
            }
            Self::Report { .. } => {}
            Self::Generate {
                task,
                control,
                generator,
            } => {
                control.cancel();
                task.take().unwrap().await?;
                drop(generator.join().await)
            }
            Self::GenerateFinished => {}
        }
        Ok(())
    }

    pub fn render_egui(&mut self, ctx: &CtxRef, proxy: &EventLoopProxy<Action>) {
        match self {
            Self::Connect { .. } => {
                CentralPanel::default().show(ctx, |ui| {
                    ui.label("connecting to server");
                });
            }
            Self::Configure {} => {
                CentralPanel::default().show(ctx, |ui| {
                    if ui.button("create").clicked() {
                        proxy.send_event(Action::Create).unwrap()
                    };
                });
            }
            Self::Connected(session) => match &session.state {
                SessionState::Lobby { slots } => {
                    CentralPanel::default().show(ctx, |ui| {
                        for (slot, id) in slots.iter().cloned().enumerate() {
                            ui.horizontal(|ui| {
                                if id != u32::MAX {
                                    ui.label(format!("{}", id));
                                    if ui.button("play").clicked() {
                                        let connection = session.context.connection.clone();
                                        spawn(async move {
                                            connection
                                                .send(Message::from(Request::Enter(slot as u8)))
                                                .await
                                                .unwrap();
                                        });
                                    }
                                    if ui.button("unbind").clicked() {
                                        let proxy = proxy.clone();
                                        let connection = session.context.connection.clone();
                                        spawn(async move {
                                            let (sender, receiver) = value_channel();
                                            connection
                                                .send(Message::from(Request::Delete(
                                                    slot as u8, sender,
                                                )))
                                                .await
                                                .unwrap();
                                            receiver.recv().await.unwrap();
                                            proxy
                                                .send_event(Action::UpdateLobbySlot(
                                                    slot as u8,
                                                    u32::MAX,
                                                ))
                                                .unwrap();
                                        });
                                    }
                                } else {
                                    ui.label("unbound");
                                    if ui.button("bind").clicked() {
                                        let proxy = proxy.clone();
                                        let connection = session.context.connection.clone();
                                        spawn(async move {
                                            let (sender, receiver) = value_channel();
                                            connection
                                                .send(Message::from(Request::Create(
                                                    slot as u8, sender,
                                                )))
                                                .await
                                                .unwrap();
                                            let value = receiver.recv().await.unwrap();
                                            proxy
                                                .send_event(Action::UpdateLobbySlot(
                                                    slot as u8, value,
                                                ))
                                                .unwrap();
                                        });
                                    }
                                }
                            });
                        }
                    });
                }
                SessionState::InGame(game) => {
                    if let Some((entity, toi)) = &game.context.target {
                        Window::new("Target").title_bar(false).show(ctx, |ui| {
                            ui.label(format!("Entity: {:?}", entity));
                            ui.label(format!("Distance: {}", toi));
                        });
                    }
                }
            },
            Self::Report { error } => {
                CentralPanel::default().show(ctx, |ui| {
                    ui.label(format!("{}", error));
                    if ui.button("exit").clicked() {
                        proxy.send_event(Action::Close).unwrap()
                    };
                });
            }
            Self::Generate { .. } => {
                CentralPanel::default().show(ctx, |ui| {
                    ui.label("generating world");
                });
            }
            Self::GenerateFinished => {
                CentralPanel::default().show(ctx, |ui| {
                    if ui.button("exit").clicked() {
                        proxy.send_event(Action::Close).unwrap()
                    };
                });
            }
        }
    }

    pub async fn update(&mut self, now: Instant) {
        if let Self::Connected(session) = self {
            session.state.update(now, &session.context)
        }
    }

    pub fn apply(
        &mut self,
        root_context: &RootContext,
        notification: Notification,
    ) -> eyre::Result<()> {
        match self {
            Self::Connected(session) => {
                if let Notification::Enter(enter) = notification {
                    session.state = SessionState::InGame(Game::new(root_context, enter)?);
                    return Ok(());
                }
                match &mut session.state {
                    SessionState::InGame(game) => match notification {
                        Notification::GlobalSetup(_) => {}
                        Notification::StaticSetup((region_pos, setup)) => {
                            game.context.world.regions.insert(
                                region_pos.into_index(game.context.world.size) as usize,
                                Region::new(
                                    region_pos,
                                    setup.heights,
                                    setup.region_size,
                                    &mut game.context.world.physics,
                                    &mut game.context.terrain.context,
                                ),
                            );
                        }
                        Notification::StaticTeardown(region_pos) => {
                            game.context
                                .world
                                .regions
                                .remove_by_id(
                                    region_pos.into_index(game.context.world.size) as usize
                                )
                                .unwrap()
                                .cleanup(
                                    region_pos,
                                    &mut game.context.world.physics,
                                    &mut game.context.terrain.context,
                                );
                        }
                        Notification::GlobalUpdates(updates) => {
                            for update in updates {
                                match update {}
                            }
                        }
                        Notification::StaticUpdates((_, updates)) => {
                            for update in updates {
                                match update {}
                            }
                        }
                        Notification::DynamicUpdates((region_pos, updates, tick)) => {
                            let region_index = game.context.world.regions.index
                                [&(region_pos.into_index(game.context.world.size) as usize)];
                            for update in updates {
                                match update {
                                    DynamicUpdate::Enter(entity, from, pos, rotation) => {
                                        match entity {
                                            protocol::Entity::NPC(id) => {
                                                game.context.world.regions.npcs[region_index]
                                                    .insert(id);
                                                if let Some(region_pos) = from {
                                                    if game
                                                        .context
                                                        .world
                                                        .regions
                                                        .index
                                                        .contains_key(
                                                            &(region_pos
                                                                .into_index(game.context.world.size)
                                                                as usize),
                                                        )
                                                    {
                                                        continue;
                                                    }
                                                }
                                                let transform = Isometry::from_parts(
                                                    pos.into(),
                                                    rotation.into(),
                                                );
                                                let handle = game.context.world.physics.add_body(
                                                    RigidBodyType::KinematicPositionBased,
                                                    transform,
                                                    character_collider(),
                                                    entity.into(),
                                                );
                                                game.context.world.npcs.insert(
                                                    id as usize,
                                                    NPC {
                                                        transform: InterpolationBuffer::new(
                                                            Transform(transform),
                                                            tick as usize,
                                                        ),
                                                        handle,
                                                        object: Object {
                                                            transform: transform.into(),
                                                            model: game.context.cube_model,
                                                        },
                                                    },
                                                );
                                            }
                                            protocol::Entity::PC(id) => {
                                                if id == game.context.self_id {
                                                    continue;
                                                }
                                                game.context.world.regions.pcs[region_index]
                                                    .insert(id);
                                                if let Some(region_pos) = from {
                                                    if game
                                                        .context
                                                        .world
                                                        .regions
                                                        .index
                                                        .contains_key(
                                                            &(region_pos
                                                                .into_index(game.context.world.size)
                                                                as usize),
                                                        )
                                                    {
                                                        continue;
                                                    }
                                                }
                                                let transform = Isometry::from_parts(
                                                    pos.into(),
                                                    rotation.into(),
                                                );
                                                let handle = game.context.world.physics.add_body(
                                                    RigidBodyType::KinematicPositionBased,
                                                    transform,
                                                    character_collider(),
                                                    entity.into(),
                                                );
                                                game.context.world.pcs.insert(
                                                    id as usize,
                                                    PC {
                                                        transform: InterpolationBuffer::new(
                                                            Transform(transform),
                                                            tick as usize,
                                                        ),
                                                        handle,
                                                        object: Object {
                                                            transform: transform.into(),
                                                            model: game.context.cube_model,
                                                        },
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    DynamicUpdate::Exit(entity, to) => match entity {
                                        protocol::Entity::NPC(id) => {
                                            game.context.world.regions.npcs[region_index]
                                                .remove(&id);
                                            if let Some(region_pos) = to {
                                                if game.context.world.regions.index.contains_key(
                                                    &(region_pos.into_index(game.context.world.size)
                                                        as usize),
                                                ) {
                                                    continue;
                                                }
                                            }
                                            let npc = game
                                                .context
                                                .world
                                                .npcs
                                                .remove_by_id(id as usize)
                                                .unwrap();
                                            game.context.world.physics.remove_body(npc.handle);
                                        }
                                        protocol::Entity::PC(id) => {
                                            if id == game.context.self_id {
                                                continue;
                                            }
                                            game.context.world.regions.pcs[region_index]
                                                .remove(&id);
                                            if let Some(region_pos) = to {
                                                if game.context.world.regions.index.contains_key(
                                                    &(region_pos.into_index(game.context.world.size)
                                                        as usize),
                                                ) {
                                                    continue;
                                                }
                                            }
                                            let pc = game
                                                .context
                                                .world
                                                .pcs
                                                .remove_by_id(id as usize)
                                                .unwrap();
                                            game.context.world.physics.remove_body(pc.handle);
                                        }
                                    },
                                    DynamicUpdate::Update(entity, pos, rotation) => match entity {
                                        protocol::Entity::NPC(id) => {
                                            let npc_index =
                                                game.context.world.npcs.index[&(id as usize)];
                                            let transform =
                                                Isometry::from_parts(pos.into(), rotation.into());
                                            game.context.world.npcs.transform[npc_index]
                                                .insert(tick as usize, Transform(transform));
                                        }
                                        protocol::Entity::PC(id) => {
                                            if id == game.context.self_id {
                                                continue;
                                            }
                                            if let Some(pc_index) = game
                                                .context
                                                .world
                                                .pcs
                                                .index
                                                .get(&(id as usize))
                                                .cloned()
                                            {
                                                let transform = Isometry::from_parts(
                                                    pos.into(),
                                                    rotation.into(),
                                                );
                                                game.context.world.pcs.transform[pc_index]
                                                    .insert(tick as usize, Transform(transform))
                                            }
                                        }
                                    },
                                }
                            }
                        }
                        Notification::Enter(_) => panic!(),
                        Notification::DynamicSetup((region_pos, setup, tick)) => {
                            let region_index = game.context.world.regions.index
                                [&(region_pos.into_index(game.context.world.size) as usize)];
                            for (id, pos, rotation) in setup.npcs {
                                game.context.world.regions.npcs[region_index].insert(id);
                                let transform = Isometry::from_parts(pos.into(), rotation.into());
                                let handle = game.context.world.physics.add_body(
                                    RigidBodyType::KinematicPositionBased,
                                    transform,
                                    character_collider(),
                                    Entity::NPC(id).into(),
                                );
                                game.context.world.npcs.insert(
                                    id as usize,
                                    NPC {
                                        transform: InterpolationBuffer::new(
                                            Transform(transform),
                                            tick as usize,
                                        ),
                                        handle,
                                        object: Object {
                                            transform: transform.into(),
                                            model: game.context.cube_model,
                                        },
                                    },
                                );
                            }
                            for (id, pos, rotation) in setup.pcs {
                                if id == game.context.self_id {
                                    continue;
                                }
                                game.context.world.regions.pcs[region_index].insert(id);
                                let transform = Isometry::from_parts(pos.into(), rotation.into());
                                let handle = game.context.world.physics.add_body(
                                    RigidBodyType::KinematicPositionBased,
                                    transform,
                                    character_collider(),
                                    Entity::PC(id).into(),
                                );
                                game.context.world.pcs.insert(
                                    id as usize,
                                    PC {
                                        transform: InterpolationBuffer::new(
                                            Transform(transform),
                                            tick as usize,
                                        ),
                                        handle,
                                        object: Object {
                                            transform: transform.into(),
                                            model: game.context.cube_model,
                                        },
                                    },
                                );
                            }
                        }
                        Notification::DynamicTeardown(region_pos) => {
                            let region_index = game.context.world.regions.index
                                [&(region_pos.into_index(game.context.world.size) as usize)];
                            for id in game.context.world.regions.npcs[region_index].drain() {
                                let npc =
                                    game.context.world.npcs.remove_by_id(id as usize).unwrap();
                                game.context.world.physics.remove_body(npc.handle);
                            }
                            for id in game.context.world.regions.pcs[region_index].drain() {
                                let pc = game.context.world.pcs.remove_by_id(id as usize).unwrap();
                                game.context.world.physics.remove_body(pc.handle);
                            }
                        }
                    },
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
        Ok(())
    }

    pub fn handle_event(&mut self, event: &Event<()>, grab: bool) -> HandleFlow {
        match self {
            Self::Connected(session) => session.state.handle_event(event, grab),
            _ => HandleFlow::Unhandled,
        }
    }

    pub fn connection(&self) -> Option<&Connection<Request>> {
        if let Self::Connected(session) = self {
            Some(&session.context.connection)
        } else {
            None
        }
    }

    pub fn world(&self) -> Option<&World> {
        if let Self::Connected(session) = self {
            if let SessionState::InGame(game) = &session.state {
                return Some(&game.context.world);
            }
        }
        None
    }

    pub fn world_mut(&mut self) -> Option<&mut World> {
        if let Self::Connected(session) = self {
            if let SessionState::InGame(game) = &mut session.state {
                return Some(&mut game.context.world);
            }
        }
        None
    }

    pub fn can_grab(&self) -> bool {
        if let Self::Connected(session) = self {
            if let SessionState::InGame(_) = &session.state {
                return true;
            }
        }
        false
    }

    pub fn game(&self) -> Option<&Game> {
        if let Self::Connected(session) = self {
            if let SessionState::InGame(game) = &session.state {
                return Some(game);
            }
        }
        None
    }
}
