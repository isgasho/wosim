use gpu_util::glam::{EulerRot, Quat, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
}

pub struct Camera {
    pub translation: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, self.roll)
    }

    pub fn rotation_xy(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw, 0.0, 0.0)
    }
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

pub struct ControlState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub fast: bool,
}
