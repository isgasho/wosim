#![no_std]

use gpu_util::{
    glam::{Mat4, Quat, Vec3A, Vec4},
    Bool32,
};

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Constants {
    pub view: Mat4,
    pub previous_view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub view_pos: Vec3A,
    pub znear: f32,
    pub zfar: f32,
    pub w: f32,
    pub h: f32,
    pub object_count: u32,
    pub use_draw_count: Bool32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Object {
    pub transform: Transform,
    pub model: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Transform {
    pub translation: Vec3A,
    pub scale: Vec3A,
    pub rotation: Quat,
}

#[cfg(feature = "nalgebra")]
impl From<nalgebra::geometry::Isometry3<f32>> for Transform {
    fn from(isometry: nalgebra::geometry::Isometry3<f32>) -> Self {
        use gpu_util::glam::vec3a;
        Self {
            translation: vec3a(
                isometry.translation.vector.x,
                isometry.translation.vector.y,
                isometry.translation.vector.z,
            ),
            scale: vec3a(1.0, 1.0, 1.0),
            rotation: Quat::from_xyzw(
                isometry.rotation.i,
                isometry.rotation.j,
                isometry.rotation.k,
                isometry.rotation.w,
            ),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Model {
    pub bounds: Vec4,
    pub mesh: Mesh,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Mesh {
    pub first_index: u32,
    pub index_count: u32,
    pub vertex_offset: i32,
}

#[repr(C)]
pub struct DrawCommand {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct DrawData {
    pub transform: Mat4,
    pub color: Vec4,
}
