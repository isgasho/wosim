#![cfg_attr(
    target_arch = "spirv",
    no_std,
    feature(register_attr, lang_items),
    register_attr(spirv)
)]

use gpu_util::abs;
use spirv_std::glam::{
    vec2, vec3, vec4, UVec3, UVec4, Vec2, Vec2Swizzles, Vec3, Vec3Swizzles, Vec4Swizzles,
};
#[cfg(not(target_arch = "spirv"))]
use spirv_std::macros::spirv;

#[spirv(compute(threads(16, 1, 16)))]
pub fn height_map(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] input: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] heights: &mut [u8],
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
) {
    let size = input.w;
    let (x, z) = (global_invocation_id.x, global_invocation_id.z);
    if x >= size || z >= size {
        return;
    }
    let y = ((fbm(vec2(
        (x + input.x) as f32 / 4096.0,
        (z + input.z) as f32 / 4096.0,
    )) + 1.0)
        * 127.0) as u8;
    heights[(z * size + x) as usize] = y;
}

fn mod289_3(x: Vec3) -> Vec3 {
    x - (x / 289.0).floor() * 289.0
}

fn mod289_2(x: Vec2) -> Vec2 {
    x - (x / 289.0).floor() * 289.0
}

fn permute(x: Vec3) -> Vec3 {
    mod289_3(((x * 34.0) + 1.0) * x)
}

const OCTAVES: usize = 32;

fn fbm(mut v: Vec2) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    for _ in 0..OCTAVES {
        value += amplitude * snoise(v);
        v *= 2.0;
        amplitude /= 2.0;
    }
    value
}

#[allow(clippy::many_single_char_names, non_snake_case)]
fn snoise(v: Vec2) -> f32 {
    let C = vec4(0.211_324_87, 0.366_025_42, -0.577_350_26, 0.024_390_243);
    let i = (v + v.dot(C.yy())).floor();
    let x0 = v - i + i.dot(C.xx());
    let i1 = if x0.x > x0.y {
        vec2(1.0, 0.0)
    } else {
        vec2(0.0, 1.0)
    };
    let x1 = x0.xy() + C.xx() - i1;
    let x2 = x0.xy() + C.zz();
    let i = mod289_2(i);
    let p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
    let m = (0.5 - vec3(x0.dot(x0), x1.dot(x1), x2.dot(x2))).max(vec3(0.0, 0.0, 0.0));
    let m = m * m;
    let m = m * m;
    let x = 2.0 * (p * C.www()).fract() - 1.0;
    let h = abs(x) - 0.5;
    let ox = (x + 0.5).floor();
    let a0 = x - ox;
    let m = m * (1.792_842_9 - 0.853_734_73 * (a0 * a0 + h * h));
    let g_x = a0.x * x0.x + h.x * x0.y;
    let g_yz = a0.yz() * vec2(x1.x, x2.x) + h.yz() * vec2(x1.y, x2.y);
    let g = vec3(g_x, g_yz.x, g_yz.y);
    130.0 * m.dot(g)
}
