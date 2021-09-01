use bytemuck::{Pod, Zeroable};
use glam::{EulerRot, Quat};
use nalgebra::UnitQuaternion;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Pod, Zeroable, Serialize, Deserialize, Default, PartialEq)]
#[repr(C)]
pub struct Rotation {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

impl From<Rotation> for Quat {
    fn from(value: Rotation) -> Self {
        Quat::from_euler(EulerRot::YXZ, value.yaw, value.pitch, value.roll)
    }
}

impl From<Quat> for Rotation {
    fn from(value: Quat) -> Self {
        let (yaw, pitch, roll) = value.to_euler(EulerRot::YXZ);
        Self { roll, pitch, yaw }
    }
}

impl From<UnitQuaternion<f32>> for Rotation {
    fn from(value: UnitQuaternion<f32>) -> Self {
        Quat::from(value).into()
    }
}

impl From<Rotation> for UnitQuaternion<f32> {
    fn from(value: Rotation) -> Self {
        Quat::from(value).into()
    }
}
