use client_gpu::Object;
use derive::Vec;
use physics::RigidBodyHandle;
use protocol::Transform;
use util::interpolation::InterpolationBuffer;

#[derive(Vec)]
pub struct NPC {
    pub transform: InterpolationBuffer<Transform, 8>,
    pub handle: RigidBodyHandle,
    pub object: Object,
}

#[derive(Vec)]
pub struct PC {
    pub transform: InterpolationBuffer<Transform, 8>,
    pub handle: RigidBodyHandle,
    pub object: Object,
}
