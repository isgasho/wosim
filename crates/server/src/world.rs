use std::{
    collections::HashMap,
    io,
    mem::swap,
    time::{Duration, Instant},
};

use database::{add_mapping, remove_mapping, Database};
use itertools::izip;
use nalgebra::Isometry3;
use network::Message;
use protocol::{DynamicUpdate, Entity, GlobalSetup, GlobalUpdate, Notification, Transform};
use tracing::warn;
use transient::{
    character::{NPCVec, PCVec},
    world::TransientWorld,
};
use util::interpolation::Interpolate;

use crate::{region::RegionManager, GlobalObserver};

pub struct ServerWorld {
    pub persistent: persistent::World,
    pub database: Database,
    pub regions: RegionManager,
    pub observers: HashMap<u128, GlobalObserver>,
    pub updates: Vec<GlobalUpdate>,
    pub physics: physics::World,
    pub transient: TransientWorld,
    pub tick_period: Duration,
    pub tick_max: Instant,
    pub tick: usize,
    pub skipped_ticks: usize,
}

impl ServerWorld {
    pub fn new(tick_start: Instant, tick_period: Duration) -> io::Result<Self> {
        let (database, persistent) = Database::open("world.db")?;
        let persistent: persistent::World = persistent;
        Ok(Self {
            database,
            regions: RegionManager::new(&persistent.configuration),
            persistent,
            observers: HashMap::new(),
            physics: physics::World::default(),
            updates: Vec::new(),
            transient: TransientWorld {
                npcs: NPCVec::new(),
                pcs: PCVec::new(),
            },
            tick_period,
            tick_max: tick_start,
            tick: 0,
            skipped_ticks: 0,
        })
    }

    pub fn snapshot(&mut self) -> io::Result<()> {
        self.database.snapshot(&mut self.persistent)
    }

    pub async fn tick(&mut self) {
        self.tick += 1;
        self.tick_max += self.tick_period;
        let start = Instant::now();
        if start > self.tick_max {
            self.skipped_ticks += 1;
            return;
        } else if self.skipped_ticks > 0 {
            warn!(
                "skipped server ticks {}-{}",
                self.tick - self.skipped_ticks,
                self.tick - 1
            );
            self.skipped_ticks = 0;
        }
        self.physics.step();
        self.update_npcs();
        self.update_pcs();
        let mut updates = Vec::new();
        swap(&mut self.updates, &mut updates);
        let update_message = if updates.is_empty() {
            None
        } else {
            Some(Message::from(Notification::GlobalUpdates(updates)))
        };
        let mut setup_message = None;
        for (_, observer) in self.observers.iter_mut() {
            if observer.pending {
                if setup_message.is_none() {
                    setup_message = Some(Message::from(Notification::GlobalSetup(GlobalSetup {})))
                }
                let _ = observer.sender.send(setup_message.clone().unwrap());
                observer.pending = false;
            } else if let Some(update_message) = &update_message {
                let _ = observer.sender.send(update_message.clone()).await;
            }
        }
        self.regions.flush(self.tick).await;
        self.regions
            .process(
                &mut self.persistent,
                &mut self.transient,
                &mut self.physics,
                self.tick,
                self.tick_max.saturating_duration_since(Instant::now()),
            )
            .await;
    }

    pub fn update_npcs(&mut self) {
        let mut remove = Vec::new();
        let heights = self.persistent.heights.read();
        let size = self.persistent.configuration.full_size();
        let max_coord = size - 1;
        for (id, handle, transform_buffer, is_ground) in izip!(
            self.transient.npcs.id.iter().cloned(),
            self.transient.npcs.handle.iter().cloned(),
            self.transient.npcs.transform.iter_mut(),
            self.transient.npcs.is_ground.iter_mut(),
        ) {
            let last: Isometry3<f32> = transform_buffer.last().0;
            let mut current = last;
            if !*is_ground {
                let v = current.translation.vector;
                let (x, y, z) = (v[0], v[1], v[2]);
                let (x0, z0) = ((x as usize).min(max_coord), (z as usize).min(max_coord));
                let (x1, z1) = ((x0 + 1).min(max_coord), (z0 + 1).min(max_coord));
                let (t_x, t_z) = (x - x.floor(), z - z.floor());
                let (y00, y10, y01, y11) = (
                    heights[z0 * size + x0] as f32,
                    heights[z0 * size + x1] as f32,
                    heights[z1 * size + x0] as f32,
                    heights[z1 * size + x0] as f32,
                );
                let min_y = Interpolate::interpolate(
                    Interpolate::interpolate(y00, y10, t_x),
                    Interpolate::interpolate(y01, y11, t_x),
                    t_z,
                ) + 1.0;
                let y = y + self.physics.gravity[1] * self.tick_period.as_secs_f32();
                if y <= min_y {
                    current.translation.vector[1] = min_y;
                    *is_ground = true;
                } else {
                    current.translation.vector[1] = y;
                }
            }
            transform_buffer.insert(self.tick, Transform(current));
            if current != last {
                self.physics.set_next_position(handle, current);
                let last_position = last.translation.into();
                let current_position = current.translation.into();
                let current_rotation = current.rotation.into();
                let last_region = self.persistent.configuration.region(last_position);
                let current_region = self.persistent.configuration.region(current_position);
                if current_region != last_region {
                    remove_mapping(
                        &mut self.persistent.npcs.region_index.write(),
                        id,
                        &mut self.persistent.regions
                            [last_region.into_index(self.persistent.configuration.size) as usize]
                            .npcs
                            .write(),
                    );
                    self.persistent.npcs.region.write()[id] = current_region;
                    add_mapping(
                        &mut self.persistent.npcs.region_index.write(),
                        id,
                        &mut self.persistent.regions[current_region
                            .into_index(self.persistent.configuration.size)
                            as usize]
                            .npcs
                            .write(),
                    );
                    self.regions
                        .get_mut(last_region)
                        .unwrap()
                        .dynamic_updates
                        .push(DynamicUpdate::Exit(
                            Entity::NPC(id as u32),
                            Some(current_region),
                        ));
                    if let Some(region) = self.regions.get_mut(current_region) {
                        region.dynamic_updates.push(DynamicUpdate::Enter(
                            Entity::NPC(id as u32),
                            Some(last_region),
                            current_position,
                            current_rotation,
                        ));
                    } else {
                        self.persistent.npcs.position.write()[id] = current_position;
                        self.persistent.npcs.rotation.write()[id] = current_rotation;
                        remove.push(id);
                    }
                } else {
                    self.regions
                        .get_mut(current_region)
                        .unwrap()
                        .dynamic_updates
                        .push(DynamicUpdate::Update(
                            Entity::NPC(id as u32),
                            current_position,
                            current_rotation,
                        ));
                }
            }
        }
        for id in remove {
            let character = self.transient.npcs.remove_by_id(id).unwrap();
            self.physics.remove_body(character.handle);
        }
    }

    pub fn update_pcs(&mut self) {
        for (id, handle, transform_buffer, target) in izip!(
            self.transient.pcs.id.iter().cloned(),
            self.transient.pcs.handle.iter().cloned(),
            self.transient.pcs.transform.iter_mut(),
            self.transient.pcs.target.iter(),
        ) {
            let last: Isometry3<f32> = transform_buffer.last().0;
            let current = Isometry3::from_parts(target.0.into(), target.1.into());
            transform_buffer.insert(self.tick, Transform(current));
            if current != last {
                self.physics.set_next_position(handle, current);
                let last_position = last.translation.into();
                let current_position = current.translation.into();
                let current_rotation = current.rotation.into();
                let last_region = self.persistent.configuration.region(last_position);
                let current_region = self.persistent.configuration.region(current_position);
                if current_region != last_region {
                    self.persistent.pcs.region.write()[id] = current_region;
                    self.regions
                        .get_mut(last_region)
                        .unwrap()
                        .dynamic_updates
                        .push(DynamicUpdate::Exit(
                            Entity::PC(id as u32),
                            Some(current_region),
                        ));
                    self.regions
                        .get_mut(current_region)
                        .unwrap()
                        .dynamic_updates
                        .push(DynamicUpdate::Enter(
                            Entity::PC(id as u32),
                            Some(last_region),
                            current_position,
                            current_rotation,
                        ));
                } else {
                    self.regions
                        .get_mut(current_region)
                        .unwrap()
                        .dynamic_updates
                        .push(DynamicUpdate::Update(
                            Entity::PC(id as u32),
                            current_position,
                            current_rotation,
                        ));
                }
            }
        }
    }
}
