use vulkan::{
    ApiResult, DescriptorBufferInfo, DescriptorImageInfo, DescriptorPool, DescriptorPoolSetup,
    DescriptorSet, DescriptorType, Extent2D, ImageLayout, Sampler, WriteDescriptorSet, WHOLE_SIZE,
};

use crate::{
    cache::Cache,
    depth::DepthImage,
    root::RootContext,
    scene::{SceneContext, SceneFrame},
};

use super::CullContext;

pub struct CullFrame {
    pub descriptor_set: DescriptorSet,
    pub setup: Cache<Extent2D, ()>,
}

impl CullFrame {
    pub fn new(
        root_context: &RootContext,
        context: &CullContext,
        scene_context: &SceneContext,
        scene_frame: &SceneFrame,
        descriptor_pool: &DescriptorPool,
    ) -> Result<Self, ApiResult> {
        let mut descriptor_sets = descriptor_pool.allocate(&[&context.set_layout])?;
        let descriptor_set = descriptor_sets.remove(0);
        let constants_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(**scene_frame.constants.buffer())
            .build()];
        let model_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(**scene_context.models.buffer())
            .build()];
        let objects_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(**scene_frame.objects.buffer())
            .build()];
        let draw_count_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(*scene_frame.draw_count)
            .build()];
        let commands_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(*scene_frame.commands)
            .build()];
        let draw_data_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(*scene_frame.draw_data)
            .build()];
        let descriptor_writes = [
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&constants_buffer_info)
                .build(),
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(3)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .buffer_info(&objects_buffer_info)
                .build(),
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(4)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .buffer_info(&model_buffer_info)
                .build(),
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(5)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .buffer_info(&draw_count_buffer_info)
                .build(),
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(6)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .buffer_info(&commands_buffer_info)
                .build(),
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(7)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .buffer_info(&draw_data_buffer_info)
                .build(),
        ];
        root_context
            .device
            .update_descriptor_sets(&descriptor_writes, &[]);
        Ok(Self {
            descriptor_set,
            setup: Cache::default(),
        })
    }

    pub fn prepare_render(
        &mut self,
        root_context: &RootContext,
        image: &DepthImage,
        sampler: &Sampler,
    ) {
        let descriptor_set = *self.descriptor_set;
        self.setup.get(root_context.swapchain.image_extent(), || {
            let sampler_info = [DescriptorImageInfo::builder().sampler(**sampler).build()];
            let image_info = [DescriptorImageInfo::builder()
                .image_view(*image.view)
                .image_layout(ImageLayout::GENERAL)
                .build()];
            let descriptor_writes = [
                WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(1)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                    .image_info(&image_info)
                    .build(),
                WriteDescriptorSet::builder()
                    .dst_set(descriptor_set)
                    .dst_binding(2)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::SAMPLER)
                    .image_info(&sampler_info)
                    .build(),
            ];
            root_context
                .device
                .update_descriptor_sets(&descriptor_writes, &[]);
        });
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        DescriptorPoolSetup {
            storage_buffers: 5,
            uniform_buffers: 1,
            sets: 1,
            combined_image_samplers: 0,
            samplers: 1,
            sampled_images: 1,
            storage_images: 0,
        }
    }
}
