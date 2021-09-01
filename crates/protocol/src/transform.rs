use nalgebra::Isometry3;
use util::interpolation::Interpolate;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Transform(pub Isometry3<f32>);

impl Interpolate for Transform {
    fn interpolate(a: Self, b: Self, t: f32) -> Self {
        if let Some(result) = a.0.try_lerp_slerp(&b.0, t, 0.001) {
            Self(result)
        } else {
            Self(Isometry3::from_parts(
                a.0.translation
                    .vector
                    .lerp(&b.0.translation.vector, t)
                    .into(),
                a.0.rotation.nlerp(&b.0.rotation, t),
            ))
        }
    }
}
