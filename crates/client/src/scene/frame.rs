use std::mem::size_of;

use client_gpu::{Constants, DrawData, Object};
use gpu_util::glam::{mat4, vec4, Mat4};
use vulkan::{
    Buffer, BufferCreateInfo, BufferUsageFlags, CommandBuffer, DescriptorBufferInfo,
    DescriptorPool, DescriptorPoolSetup, DescriptorSet, DescriptorType, DrawIndexedIndirectCommand,
    GpuVariable, GpuVec, IndexType, MemoryLocation, Pipeline, PipelineBindPoint, VkResult,
    WriteDescriptorSet, WHOLE_SIZE,
};

use crate::{
    root::{RootContext, RootFrame},
    world::World,
};

use super::SceneContext;

pub struct SceneFrame {
    pub descriptor_set: DescriptorSet,
    pub objects: GpuVec<Object>,
    pub constants: GpuVariable<Constants>,
    pub draw_count_read_back: GpuVariable<u32>,
    pub draw_count: Buffer,
    pub draw_data: Buffer,
    pub commands: Buffer,
    pub previous_view: Mat4,
}

impl SceneFrame {
    pub fn new(
        root_context: &RootContext,
        context: &SceneContext,
        object_capacity: usize,
        descriptor_pool: &DescriptorPool,
    ) -> Result<Self, vulkan::Error> {
        let device = &root_context.device;
        let mut descriptor_sets = descriptor_pool.allocate(&[&context.set_layout])?;
        let descriptor_set = descriptor_sets.remove(0);
        let constants = device.create_variable(
            BufferUsageFlags::UNIFORM_BUFFER,
            MemoryLocation::CpuToGpu,
            Constants::default(),
        )?;
        let create_info = BufferCreateInfo::builder()
            .size((size_of::<DrawIndexedIndirectCommand>() * object_capacity) as u64)
            .usage(BufferUsageFlags::STORAGE_BUFFER | BufferUsageFlags::INDIRECT_BUFFER);
        let commands = device.create_buffer(&create_info, MemoryLocation::GpuOnly)?;
        let create_info = BufferCreateInfo::builder()
            .size((size_of::<DrawData>() * object_capacity) as u64)
            .usage(BufferUsageFlags::STORAGE_BUFFER);
        let draw_data = device.create_buffer(&create_info, MemoryLocation::GpuOnly)?;
        let create_info = BufferCreateInfo::builder()
            .size(size_of::<u32>() as u64)
            .usage(
                BufferUsageFlags::STORAGE_BUFFER
                    | BufferUsageFlags::TRANSFER_SRC
                    | BufferUsageFlags::TRANSFER_DST
                    | BufferUsageFlags::INDIRECT_BUFFER,
            );
        let draw_count = device.create_buffer(&create_info, MemoryLocation::GpuOnly)?;
        let constants_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(**constants.buffer())
            .build()];
        let draw_data_buffer_info = [DescriptorBufferInfo::builder()
            .offset(0)
            .range(WHOLE_SIZE)
            .buffer(*draw_data)
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
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .buffer_info(&draw_data_buffer_info)
                .build(),
        ];
        device.update_descriptor_sets(&descriptor_writes, &[]);
        Ok(Self {
            descriptor_set,
            objects: device.create_vec(
                object_capacity,
                BufferUsageFlags::STORAGE_BUFFER,
                MemoryLocation::CpuToGpu,
            )?,
            draw_count_read_back: device.create_variable(
                BufferUsageFlags::TRANSFER_DST,
                MemoryLocation::GpuToCpu,
                0,
            )?,
            commands,
            constants,
            draw_count,
            draw_data,
            previous_view: Mat4::IDENTITY,
        })
    }

    pub fn update(
        &mut self,
        root_context: &RootContext,
        context: &SceneContext,
        world: &mut World,
    ) -> VkResult<u32> {
        let extent = root_context.swapchain.image_extent();
        let aspect = (extent.width as f32) / (extent.height as f32);
        let h = (context.camera.fovy / 2.0).tan();
        let w = h * aspect;
        let projection = mat4(
            vec4(1.0 / w, 0.0, 0.0, 0.0),
            vec4(0.0, -1.0 / h, 0.0, 0.0),
            vec4(
                0.0,
                0.0,
                context.camera.znear / (context.camera.zfar - context.camera.znear),
                -1.0,
            ),
            vec4(
                0.0,
                0.0,
                context.camera.znear * context.camera.zfar
                    / (context.camera.zfar - context.camera.znear),
                0.0,
            ),
        );
        let view = Mat4::from_quat(context.camera.rotation().inverse())
            * Mat4::from_translation(-context.camera.translation);
        let previous_view = self.previous_view;
        self.previous_view = view;
        let view_projection = projection * view;
        self.objects.clear();
        let tick_time = (world.tick_time.elapsed().as_secs_f32()
            - world.client_delta.as_secs_f32())
            / world.tick_delta.as_secs_f32()
            + world.tick as f32;
        for index in world.npcs.range() {
            let transform = world.npcs.transform[index].get(tick_time).0;
            let handle = world.npcs.handle[index];
            world
                .physics
                .bodies
                .get_mut(handle)
                .unwrap()
                .set_next_kinematic_position(transform);
            world.npcs.object[index].transform = transform.into();
        }
        for index in world.pcs.range() {
            let transform = world.pcs.transform[index].get(tick_time).0;
            let handle = world.pcs.handle[index];
            world
                .physics
                .bodies
                .get_mut(handle)
                .unwrap()
                .set_next_kinematic_position(transform);
            world.pcs.object[index].transform = transform.into();
        }
        world.physics.step();
        self.objects.append(&world.npcs.object);
        self.objects.append(&world.pcs.object);
        *self.constants.value_mut() = Constants {
            object_count: self.objects.len() as u32,
            view,
            previous_view,
            projection,
            view_projection,
            view_pos: context.camera.translation.into(),
            zfar: context.camera.zfar,
            znear: context.camera.znear,
            w,
            h,
            use_draw_count: root_context.render_configuration.use_draw_count,
        };
        root_context
            .device
            .flush_mapped_memory_ranges(&[self.objects.range(), self.constants.range()])?;
        root_context
            .device
            .invalidate_mapped_memory_ranges(&[self.draw_count_read_back.range()])?;
        Ok(*self.draw_count_read_back.value())
    }

    pub fn render(
        &self,
        root_frame: &RootFrame,
        context: &SceneContext,
        pipeline: &Pipeline,
        use_draw_count: bool,
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
        command_buffer.bind_vertex_buffers(0, &[(context.vertices.buffer(), 0)]);
        command_buffer.bind_index_buffer(context.vertex_indices.buffer(), 0, IndexType::UINT32);
        self.draw(command_buffer, use_draw_count);
    }

    fn draw(&self, command_buffer: &CommandBuffer, use_draw_count: bool) {
        if use_draw_count {
            command_buffer.draw_indexed_indirect_count(
                &self.commands,
                0,
                &self.draw_count,
                0,
                self.objects.len() as u32,
                size_of::<DrawIndexedIndirectCommand>() as u32,
            );
        } else {
            command_buffer.draw_indexed_indirect(
                &self.commands,
                0,
                self.objects.len() as u32,
                size_of::<DrawIndexedIndirectCommand>() as u32,
            );
        }
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        DescriptorPoolSetup {
            combined_image_samplers: 0,
            sets: 1,
            storage_buffers: 1,
            storage_images: 0,
            uniform_buffers: 1,
            sampled_images: 0,
            samplers: 0,
        }
    }
}
