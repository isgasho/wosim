#![cfg_attr(
    target_arch = "spirv",
    no_std,
    feature(register_attr, lang_items),
    register_attr(spirv)
)]

use spirv_std::num_traits::FloatConst;

use client_gpu::{Constants, DrawCommand, DrawData, Model, Object};
use gpu_util::{
    atomic_increment,
    glam::{ivec2, vec3, Vec2, Vec3A, Vec3Swizzles},
    ImageExt, TRUE,
};
#[cfg(not(target_arch = "spirv"))]
use spirv_std::macros::spirv;
#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;
use spirv_std::{
    glam::{mat4, vec2, vec4, Mat4, Quat, UVec2, UVec3, Vec3, Vec4, Vec4Swizzles},
    image::{Image2d, Image2dArray},
    num_traits::Pow,
    Image, Sampler,
};

#[allow(clippy::too_many_arguments)]
#[spirv(compute(threads(256)))]
pub fn cull(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] constants: &Constants,
    #[spirv(descriptor_set = 0, binding = 1)] depth_pyramid_image: &Image2d,
    #[spirv(descriptor_set = 0, binding = 2)] depth_pyramid_sampler: &Sampler,
    #[spirv(storage_buffer(std140), descriptor_set = 0, binding = 3)] objects: &[Object],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] models: &[Model],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] draw_count: &mut u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] draw_commands: &mut [DrawCommand],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] draw_data: &mut [DrawData],
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
) {
    if global_invocation_id.x >= constants.object_count {
        return;
    }
    let object = objects[global_invocation_id.x as usize];
    let model = models[object.model as usize];
    let transform = mat4(
        vec4(object.transform.scale.x, 0.0, 0.0, 0.0),
        vec4(0.0, object.transform.scale.y, 0.0, 0.0),
        vec4(0.0, 0.0, object.transform.scale.z, 0.0),
        vec4(
            object.transform.translation.x,
            object.transform.translation.y,
            object.transform.translation.z,
            1.0,
        ),
    ) * quat_to_mat4(object.transform.rotation);
    let radius = max_component(object.transform.scale) * model.bounds.w * 1.1;
    let model_center = vec4(model.bounds.x, model.bounds.y, model.bounds.z, 1.0);
    let world_center = transform * model_center;
    let view_center = constants.view * world_center;
    let visible =
        model.mesh.index_count != 0 && frustum_visible(view_center.xyz(), radius, constants);
    let view_center = constants.previous_view * world_center;
    let color = vec4(0.0, 0.0, 0.0, 1.0);
    let visible = visible
        && if -view_center.z - radius > constants.znear {
            let pyramid_size: UVec2 = depth_pyramid_image.query_size_lod(0);
            let screen_pos = view_center.xy()
                / (2.0 * -view_center.z * vec2(constants.w, -constants.h))
                + vec2(0.5, 0.5);
            let r = radius / (-view_center.z * constants.w) * pyramid_size.x as f32 * 1.0;
            let level = r.log2().floor();
            let depth: Vec4 =
                depth_pyramid_image.sample_by_lod(*depth_pyramid_sampler, screen_pos, level);
            let depth = depth.x;
            let max_depth = (view_center.z * constants.znear + constants.zfar * constants.znear)
                / ((constants.zfar - constants.znear) * -view_center.z);
            depth <= max_depth
        } else {
            true
        };
    if !visible && constants.use_draw_count == TRUE {
        return;
    }
    let draw_index = if constants.use_draw_count == TRUE {
        atomic_increment(draw_count)
    } else {
        global_invocation_id.x
    };
    draw_commands[draw_index as usize] = DrawCommand {
        first_index: model.mesh.first_index,
        index_count: model.mesh.index_count,
        vertex_offset: model.mesh.vertex_offset,
        first_instance: draw_index,
        instance_count: if visible { 1 } else { 0 },
    };
    draw_data[draw_index as usize] = DrawData { transform, color };
}

#[spirv(compute(threads(16, 16)))]
pub fn depth_pyramid(
    #[spirv(descriptor_set = 0, binding = 0)] in_image: &Image2d,
    #[spirv(descriptor_set = 0, binding = 1)] in_sampler: &Sampler,
    #[spirv(descriptor_set = 0, binding = 2)] out_image: &mut Image!(2D, format=r32f, sampled=false),
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
) {
    let size: UVec2 = out_image.query_size();
    let pos = global_invocation_id.xy();
    let depth: Vec4 = in_image.sample_by_lod(
        *in_sampler,
        (vec2(pos.x as f32, pos.y as f32) + vec2(0.5, 0.5)) / size.x as f32,
        0.0,
    );
    let depth = depth.x;
    unsafe {
        out_image.write(
            ivec2(pos.x as i32, pos.y as i32),
            vec4(depth, depth, depth, depth),
        );
    }
}

#[spirv(vertex)]
pub fn egui_vertex(
    in_pos: Vec2,
    in_uv: Vec2,
    in_color: Vec4,
    #[spirv(push_constant)] size: &Vec2,
    #[spirv(position)] out_pos: &mut Vec4,
    out_uv: &mut Vec2,
    out_color: &mut Vec4,
) {
    let pos = 2.0 * in_pos / *size - vec2(1.0, 1.0);
    *out_pos = vec4(pos.x, pos.y, 0.0, 1.0);
    *out_uv = in_uv;
    *out_color = in_color;
}

#[spirv(fragment)]
pub fn egui_fragment(
    in_uv: Vec2,
    in_color: Vec4,
    #[spirv(descriptor_set = 0, binding = 0)] font_image: &Image2d,
    #[spirv(descriptor_set = 0, binding = 1)] font_sampler: &Sampler,
    out_color: &mut Vec4,
) {
    let color: Vec4 = font_image.sample(*font_sampler, in_uv);
    *out_color = in_color * color;
}

#[allow(clippy::too_many_arguments)]
#[spirv(vertex)]
pub fn scene_vertex(
    in_pos: Vec3,
    in_normal: Vec3,
    in_color: Vec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] constants: &Constants,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] draw_data: &[DrawData],
    #[spirv(base_instance)] base_instance: u32,
    #[spirv(position)] out_pos: &mut Vec4,
    out_world_pos: &mut Vec3,
    out_normal: &mut Vec3,
    out_color: &mut Vec3,
) {
    let data = draw_data[base_instance as usize];
    let pos = data.transform * vec4(in_pos.x, in_pos.y, in_pos.z, 1.0);
    *out_pos = constants.view_projection * pos;
    *out_world_pos = pos.xyz() / pos.w;
    *out_normal = (data.transform * vec4(in_normal.x, in_normal.y, in_normal.z, 0.0))
        .xyz()
        .normalize();
    *out_color = in_color;
}

#[spirv(vertex)]
pub fn terrain_vertex(in_vertex: Vec3, #[spirv(position)] out_pos: &mut Vec4, out_layer: &mut f32) {
    *out_pos = vec4(in_vertex.x, 0.0, in_vertex.z, 1.0);
    *out_layer = in_vertex.y;
}

#[spirv(tessellation_control(output_vertices = 4))]
pub fn terrain_tessellation_control(
    #[spirv(position)] in_pos: [Vec4; 4],
    #[spirv(position)] out_pos: &mut [Vec4; 4],
    in_layer: [f32; 4],
    out_layer: &mut [f32; 4],
    #[spirv(invocation_id)] invocation_id: i32,
    #[spirv(tess_level_outer)] outer: &mut [f32; 4],
    #[spirv(tess_level_inner)] inner: &mut [f32; 2],
) {
    out_pos[invocation_id as usize] = in_pos[invocation_id as usize];
    out_layer[invocation_id as usize] = in_layer[invocation_id as usize];
    if invocation_id == 0 {
        outer[0] = 255.0;
        outer[1] = 255.0;
        outer[2] = 255.0;
        outer[3] = 255.0;
        inner[0] = 255.0;
        inner[1] = 255.0;
    }
}

#[allow(clippy::too_many_arguments)]
#[spirv(tessellation_evaluation(quads, spacing_equal, vertex_order_ccw))]
pub fn terrain_tessellation_evaluation(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] constants: &Constants,
    #[spirv(descriptor_set = 0, binding = 1)] heightmaps: &Image2dArray,
    #[spirv(descriptor_set = 0, binding = 2)] sampler: &Sampler,
    #[spirv(tess_coord)] tess_coord: Vec3,
    #[spirv(position)] in_pos: [Vec4; 4],
    in_layer: [f32; 4],
    #[spirv(position)] out_pos: &mut Vec4,
    out_world_pos: &mut Vec3,
    out_normal: &mut Vec3,
    out_color: &mut Vec3,
) {
    let tex_coords = vec3(
        tess_coord.x * (255.0 / 256.0) + (1.0 / 512.0),
        tess_coord.y * (255.0 / 256.0) + (1.0 / 512.0),
        in_layer[0],
    );
    let height: Vec4 = heightmaps.sample_by_lod(*sampler, tex_coords, 0.0);
    let left: Vec4 = heightmaps.sample_by_lod_offset_left(*sampler, tex_coords, 0.0);
    let right: Vec4 = heightmaps.sample_by_lod_offset_right(*sampler, tex_coords, 0.0);
    let bottom: Vec4 = heightmaps.sample_by_lod_offset_bottom(*sampler, tex_coords, 0.0);
    let top: Vec4 = heightmaps.sample_by_lod_offset_top(*sampler, tex_coords, 0.0);
    let normal = vec3((left.x - right.x) * 255.0, 1.0, (bottom.x - top.x) * 255.0).normalize();
    let height = height.x;
    let pos = in_pos[0]
        .lerp(in_pos[1], tess_coord.x)
        .lerp(in_pos[3].lerp(in_pos[2], tess_coord.x), tess_coord.y)
        + vec4(0.0, height * 255.0, 0.0, 0.0);
    *out_pos = constants.view_projection * pos;
    *out_world_pos = pos.xyz();
    *out_normal = normal;
    *out_color = height * vec3(0.306, 0.445, 0.249);
}

fn frustum_visible(p: Vec3, r: f32, constants: &Constants) -> bool {
    (-p.x - constants.w * p.z + r > 0.0)
        && (p.x - constants.w * p.z + r > 0.0)
        && (-p.y - constants.h * p.z + r > 0.0)
        && (p.y - constants.h * p.z + r > 0.0)
        && (-p.z - constants.znear + r > 0.0)
        && (p.z + constants.zfar + r > 0.0)
}

fn quat_to_mat4(q: Quat) -> Mat4 {
    mat4(
        vec4(
            1.0 - 2.0 * (q.y * q.y + q.z * q.z),
            2.0 * (q.x * q.y + q.z * q.w),
            2.0 * (q.x * q.z - q.y * q.w),
            0.0,
        ),
        vec4(
            2.0 * (q.x * q.y - q.z * q.w),
            1.0 - 2.0 * (q.x * q.x + q.z * q.z),
            2.0 * (q.y * q.z + q.x * q.w),
            0.0,
        ),
        vec4(
            2.0 * (q.x * q.z + q.y * q.w),
            2.0 * (q.y * q.z - q.x * q.w),
            1.0 - 2.0 * (q.x * q.x + q.y * q.y),
            0.0,
        ),
        vec4(0.0, 0.0, 0.0, 1.0),
    )
}

fn max_component(v: Vec3A) -> f32 {
    v.x.abs().max(v.y.abs()).max(v.z.abs())
}

fn specular_ggx(nh: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let f = (nh * a - nh) * nh + 1.0;
    a / (f32::PI() * f * f)
}

fn specular_smith_ggx_correlated(nv: f32, nl: f32, roughness: f32) -> f32 {
    let a2 = roughness * roughness;
    let ggxl = nv * ((-nl * a2 + nl) * nl + a2).sqrt();
    let ggxv = nl * ((-nv * a2 + nv) * nv + a2).sqrt();
    0.5 / (ggxv + ggxl)
}

fn specular_schlick(u: f32, f0: Vec3) -> Vec3 {
    let f = (1.0 - u).pow(5.0);
    f0 + (1.0 - f0) * f
}

fn diffuse_lambert() -> f32 {
    1.0 / f32::PI()
}

fn brdf(
    base_color: Vec3,
    n: Vec3,
    v: Vec3,
    l: Vec3,
    perceptual_roughness: f32,
    metallic: f32,
    reflectance: f32,
) -> Vec3 {
    let diffuse_color = (1.0 - metallic) * base_color;
    let f0 = 0.16 * reflectance * reflectance * (1.0 - metallic) + base_color * metallic;

    let h = (v + l).normalize();
    let nv = n.dot(v).abs() + 1e-5;
    let nl = n.dot(l).clamp(0.0, 1.0);
    let nh = n.dot(h).clamp(0.0, 1.0);
    let lh = l.dot(h).clamp(0.0, 1.0);

    let roughness = perceptual_roughness * perceptual_roughness;
    let specular_d = specular_ggx(nh, roughness);
    let specular_f = specular_schlick(lh, f0);
    let specular_v = specular_smith_ggx_correlated(nv, nl, roughness);
    let specular = (specular_d * specular_v) * specular_f;
    let diffuse = diffuse_color * diffuse_lambert();

    specular + diffuse
}

#[spirv(fragment)]
pub fn default_fragment(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] constants: &Constants,
    in_world_pos: Vec3,
    in_normal: Vec3,
    in_base_color: Vec3,
    out_color: &mut Vec4,
) {
    let l = vec3(0.1, -1.0, 0.3).normalize();
    let color = brdf(
        in_base_color,
        in_normal.normalize(),
        (vec3(
            constants.view_pos.x,
            constants.view_pos.y,
            constants.view_pos.z,
        ) - in_world_pos)
            .normalize(),
        -l,
        0.5,
        0.1,
        0.1,
    );
    *out_color = vec4(color.x, color.y, color.z, 1.0);
}
