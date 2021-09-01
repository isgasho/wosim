use crate::Groups;
use nalgebra::{vector, DMatrix, Isometry, UnitQuaternion};
use protocol::RegionPos;
use rapier3d::prelude::{
    ActiveCollisionTypes, Collider, ColliderBuilder, HeightField, InteractionGroups,
};

pub fn character_collider() -> Collider {
    ColliderBuilder::cuboid(1.0, 1.0, 1.0)
        .collision_groups(InteractionGroups::new(
            Groups::CHARACTER.bits(),
            Groups::WALKABLE.bits(),
        ))
        .active_collision_types(
            ActiveCollisionTypes::default() | ActiveCollisionTypes::KINEMATIC_STATIC,
        )
        .position(Isometry::from_parts(
            vector![0.0, 0.0, 0.0].into(),
            UnitQuaternion::default(),
        ))
        .build()
}

pub fn height_field_collider(region_pos: RegionPos, region_size: u32, heights: &[f32]) -> Collider {
    let (vertices, indices) = HeightField::new(
        DMatrix::from_row_slice(
            (region_size + 1) as usize,
            (region_size + 1) as usize,
            heights,
        ),
        vector![region_size as f32, 1.0, region_size as f32],
    )
    .to_trimesh();
    ColliderBuilder::trimesh(vertices, indices)
        .translation(vector![
            region_pos.x as f32 * region_size as f32 + 0.5 * region_size as f32,
            0.0,
            region_pos.z as f32 * region_size as f32 + 0.5 * region_size as f32
        ])
        .build()
}
