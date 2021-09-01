mod context;
mod image;

pub use context::*;
pub use image::*;
use vulkan::{
    DescriptorPool, DescriptorPoolSetup, Extent2D, Filter, Sampler, SamplerAddressMode,
    SamplerCreateInfo, SamplerMipmapMode, SamplerReductionMode, SamplerReductionModeCreateInfo,
    VkResult,
};

use crate::{cache::Cache, root::RootContext};

pub struct Depth {
    pub image: Cache<Extent2D, DepthImage>,
    pub sampler: Cache<u32, Sampler>,
    pub context: DepthContext,
}

impl Depth {
    pub fn new(root_context: &RootContext, descriptor_pool: &DescriptorPool) -> eyre::Result<Self> {
        let context = DepthContext::new(root_context, descriptor_pool)?;
        Ok(Self {
            image: Cache::default(),
            sampler: Cache::default(),
            context,
        })
    }

    pub fn create_sampler(root_context: &RootContext, mip_levels: u32) -> VkResult<Sampler> {
        let mut sampler_reduction_info =
            SamplerReductionModeCreateInfo::builder().reduction_mode(SamplerReductionMode::MIN);
        let create_info = SamplerCreateInfo::builder()
            .mag_filter(Filter::LINEAR)
            .min_filter(Filter::LINEAR)
            .mipmap_mode(SamplerMipmapMode::NEAREST)
            .address_mode_u(SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(SamplerAddressMode::CLAMP_TO_EDGE)
            .min_lod(0f32)
            .max_lod(mip_levels as f32)
            .push_next(&mut sampler_reduction_info);
        root_context.device.create_sampler(&create_info)
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        DepthContext::pool_setup()
    }
}
