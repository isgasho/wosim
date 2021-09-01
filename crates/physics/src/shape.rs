use rapier3d::prelude::Capsule;

pub fn character_shape() -> Capsule {
    Capsule::new_y(0.25, 0.25)
}
