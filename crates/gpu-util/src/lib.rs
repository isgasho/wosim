#![no_std]
#![feature(asm)]
#![allow(incomplete_features)]
#![feature(const_generics)]

pub use spirv_std::glam;
use spirv_std::{
    glam::{vec3, Vec3},
    image::Image,
    image::{
        AccessQualifier, Arrayed, Dimensionality, ImageCoordinate, ImageDepth, ImageFormat,
        Multisampled, SampleType, Sampled,
    },
    num_traits::Float,
    vector::Vector,
    Sampler,
};

pub type Bool32 = u32;
pub const TRUE: Bool32 = 1;

#[spirv_std::macros::gpu_only]
pub fn atomic_increment(value: &mut u32) -> u32 {
    let a: u32 = 1;
    let b: u32 = 0;
    let result: u32;
    unsafe {
        asm!(
            "{result} = OpAtomicIIncrement typeof{result} {value} {a} {b}",
            result = out(reg) result,
            value = in(reg) value,
            a = in(reg) a,
            b = in(reg) b,
        );
    }
    result
}

pub fn abs(v: Vec3) -> Vec3 {
    vec3(v.x.abs(), v.y.abs(), v.z.abs())
}

pub trait ImageExt<
    SampledType: SampleType<FORMAT>,
    const DIM: Dimensionality,
    const FORMAT: ImageFormat,
    const ARRAYED: Arrayed,
>
{
    fn sample_by_lod_offset_left<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>;

    fn sample_by_lod_offset_right<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>;

    fn sample_by_lod_offset_top<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>;

    fn sample_by_lod_offset_bottom<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>;
}

impl<
        SampledType: SampleType<FORMAT>,
        const DIM: Dimensionality,
        const DEPTH: ImageDepth,
        const FORMAT: ImageFormat,
        const ARRAYED: Arrayed,
        const SAMPLED: Sampled,
        const ACCESS_QUALIFIER: Option<AccessQualifier>,
    > ImageExt<SampledType, DIM, FORMAT, ARRAYED>
    for Image<
        SampledType,
        DIM,
        DEPTH,
        ARRAYED,
        { Multisampled::False },
        SAMPLED,
        FORMAT,
        ACCESS_QUALIFIER,
    >
{
    #[spirv_std::macros::gpu_only]
    fn sample_by_lod_offset_left<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>,
    {
        let mut result = Default::default();
        unsafe {
            asm!(
                "%image = OpLoad _ {this}",
                "%sampler = OpLoad _ {sampler}",
                "%coordinate = OpLoad _ {coordinate}",
                "%lod = OpLoad _ {lod}",
                "%sampledImage = OpSampledImage _ %image %sampler",
                "%int = OpTypeInt 32 1",
                "%v2int = OpTypeVector %int 2",
                "%offsetX = OpConstant %int -1",
                "%offsetY = OpConstant %int 0",
                "%offset = OpConstantComposite %v2int %offsetX %offsetY",
                "%result = OpImageSampleExplicitLod _ %sampledImage %coordinate Lod|ConstOffset %lod %offset",
                "OpStore {result} %result",
                result = in(reg) &mut result,
                this = in(reg) self,
                sampler = in(reg) &sampler,
                coordinate = in(reg) &coordinate,
                lod = in(reg) &lod,
            );
        }
        result
    }

    #[spirv_std::macros::gpu_only]
    fn sample_by_lod_offset_right<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>,
    {
        let mut result = Default::default();
        unsafe {
            asm!(
                "%image = OpLoad _ {this}",
                "%sampler = OpLoad _ {sampler}",
                "%coordinate = OpLoad _ {coordinate}",
                "%lod = OpLoad _ {lod}",
                "%sampledImage = OpSampledImage _ %image %sampler",
                "%int = OpTypeInt 32 1",
                "%v2int = OpTypeVector %int 2",
                "%offsetX = OpConstant %int 1",
                "%offsetY = OpConstant %int 0",
                "%offset = OpConstantComposite %v2int %offsetX %offsetY",
                "%result = OpImageSampleExplicitLod _ %sampledImage %coordinate Lod|ConstOffset %lod %offset",
                "OpStore {result} %result",
                result = in(reg) &mut result,
                this = in(reg) self,
                sampler = in(reg) &sampler,
                coordinate = in(reg) &coordinate,
                lod = in(reg) &lod,
            );
        }
        result
    }

    #[spirv_std::macros::gpu_only]
    fn sample_by_lod_offset_top<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>,
    {
        let mut result = Default::default();
        unsafe {
            asm!(
                "%image = OpLoad _ {this}",
                "%sampler = OpLoad _ {sampler}",
                "%coordinate = OpLoad _ {coordinate}",
                "%lod = OpLoad _ {lod}",
                "%sampledImage = OpSampledImage _ %image %sampler",
                "%int = OpTypeInt 32 1",
                "%v2int = OpTypeVector %int 2",
                "%offsetX = OpConstant %int 0",
                "%offsetY = OpConstant %int -1",
                "%offset = OpConstantComposite %v2int %offsetX %offsetY",
                "%result = OpImageSampleExplicitLod _ %sampledImage %coordinate Lod|ConstOffset %lod %offset",
                "OpStore {result} %result",
                result = in(reg) &mut result,
                this = in(reg) self,
                sampler = in(reg) &sampler,
                coordinate = in(reg) &coordinate,
                lod = in(reg) &lod,
            );
        }
        result
    }

    #[spirv_std::macros::gpu_only]
    fn sample_by_lod_offset_bottom<F, V>(
        &self,
        sampler: Sampler,
        coordinate: impl ImageCoordinate<F, DIM, ARRAYED>,
        lod: f32,
    ) -> V
    where
        F: Float,
        V: Vector<SampledType, 4>,
    {
        let mut result = Default::default();
        unsafe {
            asm!(
                "%image = OpLoad _ {this}",
                "%sampler = OpLoad _ {sampler}",
                "%coordinate = OpLoad _ {coordinate}",
                "%lod = OpLoad _ {lod}",
                "%sampledImage = OpSampledImage _ %image %sampler",
                "%int = OpTypeInt 32 1",
                "%v2int = OpTypeVector %int 2",
                "%offsetX = OpConstant %int 0",
                "%offsetY = OpConstant %int 1",
                "%offset = OpConstantComposite %v2int %offsetX %offsetY",
                "%result = OpImageSampleExplicitLod _ %sampledImage %coordinate Lod|ConstOffset %lod %offset",
                "OpStore {result} %result",
                result = in(reg) &mut result,
                this = in(reg) self,
                sampler = in(reg) &sampler,
                coordinate = in(reg) &coordinate,
                lod = in(reg) &lod,
            );
        }
        result
    }
}
