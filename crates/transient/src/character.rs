use derive::Vec;
use physics::RigidBodyHandle;
use protocol::{Position, Rotation, Transform};
use util::interpolation::InterpolationBuffer;

#[derive(Vec)]
pub struct NPC {
    pub handle: RigidBodyHandle,
    pub transform: InterpolationBuffer<Transform, 4>,
    pub is_ground: bool,
}

#[derive(Vec)]
pub struct PC {
    pub handle: RigidBodyHandle,
    pub transform: InterpolationBuffer<Transform, 4>,
    pub target: (Position, Rotation),
}
