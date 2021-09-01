use std::convert::TryInto;
use std::ffi::CString;
use vulkan::{
    ComputePipelineCreateInfo, DescriptorPool, DescriptorPoolSetup, DescriptorSet,
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
    DescriptorType, Pipeline, PipelineLayout, PipelineLayoutCreateFlags,
    PipelineShaderStageCreateInfo, ShaderStageFlags,
};

use crate::root::RootContext;

use super::DepthImage;

pub struct DepthContext {
    pub pipeline: Pipeline,
    pub pipeline_layout: PipelineLayout,
    pub set_layout: DescriptorSetLayout,
    pub descriptor_sets: [DescriptorSet; DepthImage::MAX_MIP_LEVELS],
}

impl DepthContext {
    pub fn new(root_context: &RootContext, descriptor_pool: &DescriptorPool) -> eyre::Result<Self> {
        let device = &root_context.device;
        let bindings = [
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::SAMPLER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(2)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_IMAGE)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
        ];
        let set_layout = device
            .create_descriptor_set_layout(DescriptorSetLayoutCreateFlags::empty(), &bindings)?;
        let pipeline_layout = device.create_pipeline_layout(
            PipelineLayoutCreateFlags::empty(),
            &[&set_layout],
            &[],
        )?;
        let main_name = CString::new("depth_pyramid").unwrap();
        let stage = PipelineShaderStageCreateInfo::builder()
            .stage(ShaderStageFlags::COMPUTE)
            .module(*root_context.shader_module)
            .name(&main_name)
            .build();
        let create_infos = [ComputePipelineCreateInfo::builder()
            .stage(stage)
            .layout(*pipeline_layout)
            .build()];
        let mut pipelines = root_context.pipeline_cache.create_compute(&create_infos)?;
        let pipeline = pipelines.remove(0);
        let descriptor_sets =
            descriptor_pool.allocate(&[&set_layout; DepthImage::MAX_MIP_LEVELS])?;
        Ok(Self {
            pipeline,
            pipeline_layout,
            set_layout,
            descriptor_sets: descriptor_sets.try_into().unwrap(),
        })
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        DescriptorPoolSetup {
            storage_buffers: 0,
            uniform_buffers: 0,
            sets: 1,
            combined_image_samplers: 0,
            samplers: 1,
            sampled_images: 1,
            storage_images: 1,
        } * DepthImage::MAX_MIP_LEVELS as u32
    }
}
