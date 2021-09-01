use std::collections::hash_map::Entry;

use glam::vec3;
use nalgebra::{vector, Isometry};
use network::Message;
use physics::{character_collider, RigidBodyType};
use protocol::{
    Entity, Notification, Request, Rotation, Transform, WorldEnter, WorldInfo, SLOT_COUNT,
};
use quinn::VarInt;
use thiserror::Error;
use transient::character::PC;
use util::interpolation::InterpolationBuffer;

use crate::{observer::GlobalObserver, user::User, world::ServerWorld};

#[derive(Error, Debug)]
pub enum RequestError {
    #[error("illegal slot")]
    IllegalSlot,
    #[error("slot already bound")]
    SlotAlreadyBound,
    #[error("slot not bound")]
    SlotNotBound,
    #[error("not in game")]
    NotInGame,
    #[error("already in game")]
    AlreadyInGame,
}

impl RequestError {
    pub fn code(&self) -> VarInt {
        VarInt::from_u32(match self {
            RequestError::IllegalSlot => 1001,
            RequestError::SlotAlreadyBound => 1002,
            RequestError::SlotNotBound => 1003,
            RequestError::NotInGame => 1004,
            RequestError::AlreadyInGame => 1005,
        })
    }

    pub fn reason(&self) -> String {
        self.to_string()
    }
}

pub(crate) async fn handle_request(
    request: Request,
    world: &mut ServerWorld,
    user: &User,
) -> Result<(), RequestError> {
    match request {
        Request::Disconnect => panic!(),
        Request::WorldInfo(sender) => {
            sender
                .send(WorldInfo {
                    region_size: world.persistent.configuration.region_size,
                    size: world.persistent.configuration.size,
                    static_distance: world.persistent.configuration.static_distance,
                })
                .unwrap();
        }
        Request::Slots(sender) => {
            let player_id = player_id(world, user.uuid);
            sender
                .send(world.persistent.players.slots.read()[player_id as usize])
                .unwrap();
        }
        Request::Create(slot, sender) => {
            validate_slot(slot)?;
            let player_id = player_id(world, user.uuid);
            let size = world.persistent.configuration.full_size() as f32;
            if let Some(id) = world.persistent.spawn_pc(
                vec3(size / 2.0, 0.0, size / 2.0),
                Rotation {
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                },
                player_id,
                slot,
            ) {
                sender.send(id as u32).unwrap();
            } else {
                return Err(RequestError::SlotAlreadyBound);
            }
        }
        Request::Delete(slot, sender) => {
            validate_slot(slot)?;
            let player_id = player_id(world, user.uuid);
            world.persistent.delete_pc(player_id, slot);
            sender.send(()).unwrap();
        }
        Request::Enter(slot) => {
            validate_slot(slot)?;
            let player_id = player_id(world, user.uuid);
            match world.observers.entry(user.uuid) {
                Entry::Occupied(_) => return Err(RequestError::AlreadyInGame),
                Entry::Vacant(entry) => {
                    let id =
                        world.persistent.players.slots.read()[player_id as usize][slot as usize];
                    if id == u32::MAX {
                        return Err(RequestError::SlotNotBound);
                    }
                    let id = id as usize;
                    let pos = world.persistent.pcs.position.read()[id];
                    let rotation = world.persistent.pcs.rotation.read()[id];
                    let transform =
                        Isometry::from_parts(vector![pos.x, pos.y, pos.z].into(), rotation.into());
                    let handle = world.physics.add_body(
                        RigidBodyType::KinematicPositionBased,
                        transform,
                        character_collider(),
                        Entity::PC(id as u32).into(),
                    );
                    let observer = entry.insert(GlobalObserver::new(
                        user.clone(),
                        pos,
                        id,
                        PC {
                            handle,
                            target: (pos, rotation),
                            transform: InterpolationBuffer::new(Transform(transform), world.tick),
                        },
                        &mut world.regions,
                        &mut world.persistent,
                    ));
                    let _ = observer
                        .sender
                        .send(Message::from(Notification::Enter(WorldEnter {
                            self_id: id as u32,
                            size: world.persistent.configuration.size,
                            region_size: world.persistent.configuration.region_size,
                            tick_delta: world.tick_period,
                            tick: world.tick as u64,
                            max_active_regions: (world.persistent.configuration.static_distance
                                as u32
                                + 1)
                                * (world.persistent.configuration.static_distance as u32 + 1)
                                * 4,
                            pos,
                            rotation,
                        })))
                        .await;
                }
            };
        }
        Request::Exit(sender) => {
            if let Some(observer) = world.observers.remove(&user.uuid) {
                observer.remove(
                    &mut world.regions,
                    &mut world.persistent,
                    &mut world.transient,
                    &mut world.physics,
                );
                sender.send(()).unwrap()
            } else {
                return Err(RequestError::NotInGame);
            }
        }
        Request::UpdateSelf((pos, rotation)) => {
            let observer = world
                .observers
                .get_mut(&user.uuid)
                .ok_or(RequestError::NotInGame)?;
            let live_id = observer.id;
            if let Some(index) = world.transient.pcs.index.get(&live_id) {
                world.transient.pcs.target[*index] = (pos, rotation);
            }
            observer.update(pos, &mut world.regions, &world.persistent.configuration);
        }
    }
    Ok(())
}

fn validate_slot(slot: u8) -> Result<(), RequestError> {
    if (slot as usize) < SLOT_COUNT {
        Ok(())
    } else {
        Err(RequestError::IllegalSlot)
    }
}

fn player_id(world: &ServerWorld, uuid: u128) -> u32 {
    *world.persistent.player_index.read().get(&uuid).unwrap()
}
