use vulkan::{
    Buffer, BufferUsageFlags, DescriptorBufferInfo, DescriptorImageInfo, DescriptorPool,
    DescriptorPoolSetup, DescriptorSet, DescriptorType, GpuVec, ImageLayout, MemoryLocation,
    Pipeline, PipelineBindPoint, VkResult, WriteDescriptorSet, WHOLE_SIZE,
};

use crate::{
    root::{RootContext, RootFrame},
    scene::SceneFrame,
    world::World,
};

use super::{TerrainContext, Vertex};

pub struct TerrainFrame {
    pub descriptor_set: DescriptorSet,
    pub staging_buffer: Option<Buffer>,
    pub vertices: GpuVec<Vertex>,
    pub last_update_frame: usize,
}

impl TerrainFrame {
    pub fn new(
        root_context: &RootContext,
        context: &TerrainContext,
        scene: &SceneFrame,
        descriptor_pool: &DescriptorPool,
        world: &World,
    ) -> Result<Self, vulkan::Error> {
        let device = &root_context.device;
        let mut descriptor_sets = descriptor_pool.allocate(&[&context.set_layout])?;
        let descriptor_set = descriptor_sets.remove(0);
        let constants_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(**scene.constants.buffer())
            .build()];
        let image_info = [DescriptorImageInfo::builder()
            .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(*context.image_view)
            .build()];
        let sampler_info = [DescriptorImageInfo::builder()
            .sampler(*context.sampler)
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
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                .image_info(&image_info)
                .build(),
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(2)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::SAMPLER)
                .image_info(&sampler_info)
                .build(),
        ];
        device.update_descriptor_sets(&descriptor_writes, &[]);
        Ok(Self {
            descriptor_set,
            staging_buffer: None,
            vertices: device.create_vec(
                world.max_active_regions as usize * 4,
                BufferUsageFlags::VERTEX_BUFFER,
                MemoryLocation::CpuToGpu,
            )?,
            last_update_frame: 0,
        })
    }

    pub fn prepare_render(
        &mut self,
        root_context: &RootContext,
        context: &TerrainContext,
        world: &World,
    ) -> VkResult<()> {
        if self.last_update_frame < context.last_update_frame {
            self.last_update_frame = context.last_update_frame;
            let region_size = world.region_size;
            self.vertices.clear();
            for (pos, index) in context.used.iter() {
                self.vertices.push(Vertex {
                    x: pos.x as f32 * region_size as f32,
                    y: *index as f32,
                    z: pos.z as f32 * region_size as f32,
                });
                self.vertices.push(Vertex {
                    x: (pos.x + 1) as f32 * region_size as f32,
                    y: *index as f32,
                    z: pos.z as f32 * region_size as f32,
                });
                self.vertices.push(Vertex {
                    x: (pos.x + 1) as f32 * region_size as f32,
                    y: *index as f32,
                    z: (pos.z + 1) as f32 * region_size as f32,
                });
                self.vertices.push(Vertex {
                    x: pos.x as f32 * region_size as f32,
                    y: *index as f32,
                    z: (pos.z + 1) as f32 * region_size as f32,
                });
            }
            root_context
                .device
                .flush_mapped_memory_ranges(&[self.vertices.range()])?;
        }
        Ok(())
    }

    pub fn render(
        &mut self,
        root_frame: &RootFrame,
        context: &TerrainContext,
        pipeline: &Pipeline,
    ) {
        let command_buffer = &root_frame.command_buffer;
        command_buffer.bind_pipeline(PipelineBindPoint::GRAPHICS, pipeline);
        command_buffer.bind_descriptor_sets(
            PipelineBindPoint::GRAPHICS,
            &context.pipeline_layout,
            0,
            &[&self.descriptor_set],
            &[],
        );
        command_buffer.bind_vertex_buffers(0, &[(self.vertices.buffer(), 0)]);
        command_buffer.draw(self.vertices.len() as u32, 1, 0, 0);
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        DescriptorPoolSetup {
            combined_image_samplers: 0,
            sets: 1,
            storage_buffers: 0,
            storage_images: 0,
            uniform_buffers: 1,
            sampled_images: 1,
            samplers: 1,
        }
    }
}
