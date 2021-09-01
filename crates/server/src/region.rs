use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    mem::swap,
    time::Duration,
};

use nalgebra::{vector, Isometry, UnitQuaternion};
use network::{Message, MessageSender};
use persistent::{Configuration, World};
use physics::{character_collider, RigidBodyType};
use protocol::{
    DynamicSetup, DynamicUpdate, Entity, Notification, RegionPos, StaticSetup, StaticUpdate,
    Transform,
};
use tokio::time::Instant;
use tracing::debug;
use transient::{
    character::{NPC, PC},
    world::TransientWorld,
};
use util::interpolation::InterpolationBuffer;

use crate::LocalObserver;

#[derive(Default)]
pub struct Region {
    pub observers: HashMap<u128, LocalObserver>,
    pub static_updates: Vec<StaticUpdate>,
    pub dynamic_updates: Vec<DynamicUpdate>,
    pub full_observers: usize,
}

pub fn dynamic_setup(
    region_pos: RegionPos,
    transient: &mut TransientWorld,
    persistent: &mut World,
    physics: &mut physics::World,
    tick: usize,
) {
    let region =
        &mut persistent.regions[region_pos.into_index(persistent.configuration.size) as usize];
    let positions = persistent.npcs.position.read();
    let rotations = persistent.npcs.rotation.read();
    for id in region.npcs.read().iter().cloned() {
        let pos = positions[id];
        let rotation = rotations[id];
        let transform = Isometry::from_parts(
            vector![pos.x, pos.y, pos.z].into(),
            UnitQuaternion::from_euler_angles(rotation.roll, rotation.pitch, rotation.yaw),
        );
        let handle = physics.add_body(
            RigidBodyType::KinematicPositionBased,
            transform,
            character_collider(),
            Entity::NPC(id as u32).into(),
        );
        transient.npcs.insert(
            id,
            NPC {
                handle,
                transform: InterpolationBuffer::new(Transform(transform), tick),
                is_ground: false,
            },
        );
    }
}

pub fn dynamic_teardown(
    region_pos: RegionPos,
    transient: &mut TransientWorld,
    persistent: &mut World,
    physics: &mut physics::World,
) {
    let region =
        &mut persistent.regions[region_pos.into_index(persistent.configuration.size) as usize];
    let mut positions = persistent.npcs.position.write();
    let mut rotations = persistent.npcs.rotation.write();
    for id in region.npcs.read().iter().cloned() {
        let npc = transient.npcs.remove_by_id(id).unwrap();
        let transform = npc.transform.last().0;
        positions[id] = transform.translation.into();
        rotations[id] = transform.rotation.into();
        physics.remove_body(npc.handle);
    }
}

fn static_setup(
    _pos: RegionPos,
    _persistent: &mut World,
    _transient: &mut TransientWorld,
    _physics: &mut physics::World,
) {
}

fn static_teardown(
    _pos: RegionPos,
    _transient: &mut TransientWorld,
    _physics: &mut physics::World,
) {
}

impl Region {
    pub async fn flush(&mut self, pos: RegionPos, tick: usize) {
        let mut updates = Vec::new();
        swap(&mut self.static_updates, &mut updates);
        let static_update_message = if updates.is_empty() {
            None
        } else {
            Some(Message::from(Notification::StaticUpdates((pos, updates))))
        };
        let mut updates = Vec::new();
        swap(&mut self.dynamic_updates, &mut updates);
        let dynamic_update_message = if updates.is_empty() || self.full_observers == 0 {
            None
        } else {
            Some(Message::from(Notification::DynamicUpdates((
                pos,
                updates,
                tick as u64,
            ))))
        };
        for (_, observer) in self.observers.iter() {
            if let Some(message) = static_update_message.clone() {
                let _ = observer.sender.send(message).await;
            }
            if observer.level == UpdateLevel::Full {
                if let Some(message) = dynamic_update_message.clone() {
                    let _ = observer.sender.send(message).await;
                }
            }
        }
    }
}

pub struct RegionManager {
    regions: HashMap<RegionPos, Region>,
    full_distance: u16,
    static_distance: u16,
    size: u32,
    queue: BinaryHeap<Entry>,
}

struct Entry(RegionPos, u16, u128, ObserverChange);

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Entry {}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.3, &other.3) {
            (ObserverChange::SetupStatic(_), ObserverChange::SetupStatic(_)) => other
                .1
                .cmp(&self.1)
                .then_with(|| self.0.cmp(&other.0))
                .then_with(|| self.2.cmp(&other.2)),
            (ObserverChange::SetupStatic(_), ObserverChange::SetupDynamic) => Ordering::Greater,
            (ObserverChange::SetupStatic(_), ObserverChange::SetupPlayer(_, _)) => {
                Ordering::Greater
            }
            (ObserverChange::SetupStatic(_), ObserverChange::TeardownDynamic) => Ordering::Greater,
            (ObserverChange::SetupStatic(_), ObserverChange::TeardownStatic) => Ordering::Greater,
            (ObserverChange::SetupDynamic, ObserverChange::SetupStatic(_)) => Ordering::Less,
            (ObserverChange::SetupDynamic, ObserverChange::SetupDynamic) => other
                .1
                .cmp(&self.1)
                .then_with(|| self.0.cmp(&other.0))
                .then_with(|| self.2.cmp(&other.2)),
            (ObserverChange::SetupDynamic, ObserverChange::SetupPlayer(_, _)) => Ordering::Greater,
            (ObserverChange::SetupDynamic, ObserverChange::TeardownDynamic) => Ordering::Greater,
            (ObserverChange::SetupDynamic, ObserverChange::TeardownStatic) => Ordering::Less,
            (ObserverChange::SetupPlayer(_, _), ObserverChange::SetupStatic(_)) => Ordering::Less,
            (ObserverChange::SetupPlayer(_, _), ObserverChange::SetupDynamic) => Ordering::Less,
            (ObserverChange::SetupPlayer(_, _), ObserverChange::SetupPlayer(_, _)) => other
                .1
                .cmp(&self.1)
                .then_with(|| self.0.cmp(&other.0))
                .then_with(|| self.2.cmp(&other.2)),
            (ObserverChange::SetupPlayer(_, _), ObserverChange::TeardownDynamic) => {
                Ordering::Greater
            }
            (ObserverChange::SetupPlayer(_, _), ObserverChange::TeardownStatic) => {
                Ordering::Greater
            }
            (ObserverChange::TeardownDynamic, ObserverChange::SetupStatic(_)) => Ordering::Less,
            (ObserverChange::TeardownDynamic, ObserverChange::SetupDynamic) => Ordering::Less,
            (ObserverChange::TeardownDynamic, ObserverChange::SetupPlayer(_, _)) => Ordering::Less,
            (ObserverChange::TeardownDynamic, ObserverChange::TeardownDynamic) => self
                .1
                .cmp(&other.1)
                .then_with(|| self.0.cmp(&other.0))
                .then_with(|| self.2.cmp(&other.2)),
            (ObserverChange::TeardownDynamic, ObserverChange::TeardownStatic) => Ordering::Greater,
            (ObserverChange::TeardownStatic, ObserverChange::SetupStatic(_)) => Ordering::Less,
            (ObserverChange::TeardownStatic, ObserverChange::SetupDynamic) => Ordering::Less,
            (ObserverChange::TeardownStatic, ObserverChange::SetupPlayer(_, _)) => Ordering::Less,
            (ObserverChange::TeardownStatic, ObserverChange::TeardownDynamic) => Ordering::Less,
            (ObserverChange::TeardownStatic, ObserverChange::TeardownStatic) => self
                .1
                .cmp(&other.1)
                .then_with(|| self.0.cmp(&other.0))
                .then_with(|| self.2.cmp(&other.2)),
        }
    }
}

pub enum ObserverChange {
    SetupStatic(MessageSender<Notification>),
    SetupDynamic,
    SetupPlayer(usize, PC),
    TeardownDynamic,
    TeardownStatic,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum UpdateLevel {
    Static,
    Full,
}

impl RegionManager {
    pub fn new(configuration: &Configuration) -> Self {
        Self {
            static_distance: configuration.static_distance,
            full_distance: configuration.full_distance,
            regions: HashMap::new(),
            size: configuration.size,
            queue: BinaryHeap::new(),
        }
    }

    pub fn get_mut(&mut self, pos: RegionPos) -> Option<&mut Region> {
        self.regions.get_mut(&pos)
    }

    pub async fn process(
        &mut self,
        persistent: &mut World,
        transient: &mut TransientWorld,
        physics: &mut physics::World,
        tick: usize,
        timeout: Duration,
    ) {
        let start = Instant::now();
        while let Some(Entry(pos, _, uuid, change)) = self.queue.pop() {
            self.apply(pos, uuid, change, persistent, transient, physics, tick)
                .await;
            if Instant::now().duration_since(start) > timeout {
                break;
            }
        }
        if !self.queue.is_empty() {
            debug!(
                "unprocessed observer updates at tick {}: {}",
                tick,
                self.queue.len()
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn apply(
        &mut self,
        pos: RegionPos,
        uuid: u128,
        change: ObserverChange,
        persistent: &mut World,
        transient: &mut TransientWorld,
        physics: &mut physics::World,
        tick: usize,
    ) {
        match change {
            ObserverChange::SetupStatic(sender) => {
                let region = self.regions.entry(pos).or_insert_with(|| {
                    static_setup(pos, persistent, transient, physics);
                    Region::default()
                });
                let mut heights = Vec::new();
                let heights_src = persistent.heights.read();
                let size = (persistent.configuration.region_size as usize)
                    * (persistent.configuration.size as usize)
                    + 1;
                for z in 0..persistent.configuration.region_size + 1 {
                    let z = (pos.z as usize) * (persistent.configuration.region_size as usize)
                        + (z as usize);
                    for x in 0..persistent.configuration.region_size + 1 {
                        let x = (pos.x as usize) * (persistent.configuration.region_size as usize)
                            + (x as usize);
                        heights.push(heights_src[z * size + x])
                    }
                }
                let _ = sender
                    .send(Message::from(Notification::StaticSetup((
                        pos,
                        StaticSetup {
                            heights,
                            region_size: persistent.configuration.region_size,
                        },
                    ))))
                    .await;
                region.observers.insert(uuid, LocalObserver::new(sender));
            }
            ObserverChange::SetupDynamic => {
                let region = self.regions.get_mut(&pos).unwrap();
                if region.full_observers == 0 {
                    dynamic_setup(pos, transient, persistent, physics, tick);
                }
                region.full_observers += 1;
                let observer = region.observers.get_mut(&uuid).unwrap();
                observer.level = UpdateLevel::Full;
                let mut npcs = Vec::new();
                let mut pcs = Vec::new();
                for id in persistent.regions[pos.into_index(persistent.configuration.size) as usize]
                    .npcs
                    .read()
                    .iter()
                    .cloned()
                {
                    let index = transient.npcs.index[&id];
                    let transform = transient.npcs.transform[index].last().0;
                    npcs.push((
                        id as u32,
                        transform.translation.into(),
                        transform.rotation.into(),
                    ));
                }
                let region = persistent.pcs.region.read();
                for (id, index) in transient.pcs.index.iter() {
                    if region[*id] == pos {
                        let transform = transient.pcs.transform[*index].last().0;
                        pcs.push((
                            *id as u32,
                            transform.translation.into(),
                            transform.rotation.into(),
                        ));
                    }
                }
                let _ = observer
                    .sender
                    .send(Message::from(Notification::DynamicSetup((
                        pos,
                        DynamicSetup { npcs, pcs },
                        tick as u64,
                    ))))
                    .await;
            }
            ObserverChange::SetupPlayer(id, pc) => {
                let transform = pc.transform.last().0;
                transient.pcs.insert(id, pc);
                let region = self.regions.get_mut(&pos).unwrap();
                region.dynamic_updates.push(DynamicUpdate::Enter(
                    Entity::PC(id as u32),
                    None,
                    transform.translation.into(),
                    transform.rotation.into(),
                ))
            }
            ObserverChange::TeardownDynamic => {
                let region = self.regions.get_mut(&pos).unwrap();
                let observer = region.observers.get_mut(&uuid).unwrap();
                observer.level = UpdateLevel::Static;
                let _ = observer
                    .sender
                    .send(Message::from(Notification::DynamicTeardown(pos)))
                    .await;
                region.full_observers -= 1;
                if region.full_observers == 0 {
                    dynamic_teardown(pos, transient, persistent, physics)
                }
            }
            ObserverChange::TeardownStatic => {
                let region = self.regions.get_mut(&pos).unwrap();
                let observer = region.observers.remove(&uuid).unwrap();
                let _ = observer
                    .sender
                    .send(Message::from(Notification::StaticTeardown(pos)))
                    .await;
                if region.observers.is_empty() {
                    static_teardown(pos, transient, physics);
                    self.regions.remove(&pos).unwrap();
                }
            }
        }
    }

    pub fn enqueue(
        &mut self,
        pos: RegionPos,
        center: RegionPos,
        uuid: u128,
        change: ObserverChange,
    ) {
        self.queue
            .push(Entry(pos, pos.distance(center), uuid, change))
    }

    pub fn update(
        &mut self,
        pos: RegionPos,
        center: RegionPos,
        uuid: u128,
        sender: &MessageSender<Notification>,
        old_level: Option<UpdateLevel>,
        new_level: Option<UpdateLevel>,
    ) {
        match (old_level, new_level) {
            (None, Some(UpdateLevel::Static)) => self.enqueue(
                pos,
                center,
                uuid,
                ObserverChange::SetupStatic(sender.clone()),
            ),
            (None, Some(UpdateLevel::Full)) => {
                self.enqueue(
                    pos,
                    center,
                    uuid,
                    ObserverChange::SetupStatic(sender.clone()),
                );
                self.enqueue(pos, center, uuid, ObserverChange::SetupDynamic);
            }
            (Some(UpdateLevel::Static), None) => {
                self.enqueue(pos, center, uuid, ObserverChange::TeardownStatic)
            }
            (Some(UpdateLevel::Full), None) => {
                self.enqueue(pos, center, uuid, ObserverChange::TeardownDynamic);
                self.enqueue(pos, center, uuid, ObserverChange::TeardownStatic);
            }
            (Some(UpdateLevel::Static), Some(UpdateLevel::Full)) => {
                self.enqueue(pos, center, uuid, ObserverChange::SetupDynamic);
            }
            (Some(UpdateLevel::Full), Some(UpdateLevel::Static)) => {
                self.enqueue(pos, center, uuid, ObserverChange::TeardownDynamic);
            }
            _ => {}
        }
    }

    pub fn level(&self, distance: u16) -> Option<UpdateLevel> {
        if distance <= self.full_distance {
            Some(UpdateLevel::Full)
        } else if distance <= self.static_distance {
            Some(UpdateLevel::Static)
        } else {
            None
        }
    }

    pub fn iterator(&self, center: RegionPos) -> RegionIterator {
        let begin_x = center.x.saturating_sub(self.static_distance);
        let end_x = center
            .x
            .saturating_add(self.static_distance + 1)
            .min(self.size as u16);
        let begin_z = center.z.saturating_sub(self.static_distance);
        let end_z = center
            .z
            .saturating_add(self.static_distance + 1)
            .min(self.size as u16);
        RegionIterator {
            begin_x,
            end_x,
            end_z,
            current_x: begin_x,
            current_z: begin_z,
        }
    }

    pub async fn flush(&mut self, tick: usize) {
        for (pos, region) in self.regions.iter_mut() {
            region.flush(*pos, tick).await;
        }
    }
}

pub struct RegionIterator {
    begin_x: u16,
    current_x: u16,
    end_x: u16,
    current_z: u16,
    end_z: u16,
}

impl Iterator for RegionIterator {
    type Item = RegionPos;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_z == self.end_z {
            return None;
        }
        let pos = RegionPos {
            x: self.current_x,
            z: self.current_z,
        };
        self.current_x += 1;
        if self.current_x == self.end_x {
            self.current_x = self.begin_x;
            self.current_z += 1;
        }
        Some(pos)
    }
}
