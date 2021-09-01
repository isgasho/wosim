use network::{MessageSender, SendError};
use persistent::{Configuration, World};
use protocol::{DynamicUpdate, Entity, Notification, Position, RegionPos};
use tokio::task::JoinHandle;
use transient::{character::PC, world::TransientWorld};

use crate::{
    region::{ObserverChange, RegionManager, UpdateLevel},
    User,
};

#[derive(Debug)]
pub struct GlobalObserver {
    pub(crate) _user: User,
    pub sender: MessageSender<Notification>,
    pub task: JoinHandle<Result<(), SendError>>,
    pub last_pos: Position,
    pub pos: Position,
    pub uuid: u128,
    pub id: usize,
    pub pending: bool,
    pub center: RegionPos,
}

pub struct LocalObserver {
    pub sender: MessageSender<Notification>,
    pub level: UpdateLevel,
}

impl LocalObserver {
    pub fn new(sender: MessageSender<Notification>) -> Self {
        Self {
            sender,
            level: UpdateLevel::Static,
        }
    }
}

impl GlobalObserver {
    pub(crate) fn new(
        user: User,
        pos: Position,
        id: usize,
        pc: PC,
        region_manager: &mut RegionManager,
        world: &mut World,
    ) -> Self {
        let (sender, task) = user.connection.channel(16);
        let center = world.configuration.region(pos);
        for pos in region_manager.iterator(center) {
            let level = region_manager.level(pos.distance(center));
            region_manager.update(pos, center, user.uuid, &sender, None, level);
        }
        region_manager.enqueue(
            center,
            center,
            user.uuid,
            ObserverChange::SetupPlayer(id, pc),
        );
        Self {
            _user: user.clone(),
            sender,
            task,
            id,
            last_pos: pos,
            pos,
            pending: true,
            center,
            uuid: user.uuid,
        }
    }

    pub fn update(
        &mut self,
        pos: Position,
        regions: &mut RegionManager,
        configuration: &Configuration,
    ) {
        if configuration.near_region(pos, self.center) {
            return;
        }
        let new_center = configuration.region(pos);
        for pos in regions.iterator(new_center) {
            let new_level = regions.level(pos.distance(new_center));
            let old_level = regions.level(pos.distance(self.center));
            regions.update(
                pos,
                new_center,
                self.uuid,
                &self.sender,
                old_level,
                new_level,
            );
        }
        for pos in regions.iterator(self.center) {
            let new_level = regions.level(pos.distance(new_center));
            if new_level.is_some() {
                continue;
            }
            let old_level = regions.level(pos.distance(self.center));
            regions.update(
                pos,
                self.center,
                self.uuid,
                &self.sender,
                old_level,
                new_level,
            );
        }
        self.center = new_center;
    }

    pub fn remove(
        self,
        regions: &mut RegionManager,
        persistent: &mut World,
        transient: &mut TransientWorld,
        physics: &mut physics::World,
    ) {
        let player = transient.pcs.remove_by_id(self.id).unwrap();
        let transform = player.transform.last().0;
        persistent.pcs.position.write()[self.id] = transform.translation.into();
        persistent.pcs.rotation.write()[self.id] = transform.rotation.into();
        physics.remove_body(player.handle);
        let region_pos = persistent.pcs.region.read()[self.id];
        if let Some(region) = regions.get_mut(region_pos) {
            region
                .dynamic_updates
                .push(DynamicUpdate::Exit(Entity::PC(self.id as u32), None))
        }
        for pos in regions.iterator(self.center) {
            let level = regions.level(pos.distance(self.center));
            regions.update(pos, self.center, self.uuid, &self.sender, level, None);
        }
    }
}
