use std::ffi::CString;

use vulkan::{
    ComputePipelineCreateInfo, DescriptorSetLayout, DescriptorSetLayoutBinding,
    DescriptorSetLayoutCreateFlags, DescriptorType, Pipeline, PipelineLayout,
    PipelineLayoutCreateFlags, PipelineShaderStageCreateInfo, ShaderStageFlags,
};

use crate::root::RootContext;

pub struct CullContext {
    pub pipeline: Pipeline,
    pub pipeline_layout: PipelineLayout,
    pub set_layout: DescriptorSetLayout,
}

impl CullContext {
    pub fn new(root_context: &RootContext) -> eyre::Result<Self> {
        let device = &root_context.device;
        let bindings = [
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(2)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::SAMPLER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(3)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(4)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(5)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(6)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(7)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
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
        let main_name = CString::new("cull").unwrap();
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
        Ok(Self {
            pipeline,
            pipeline_layout,
            set_layout,
        })
    }
}
