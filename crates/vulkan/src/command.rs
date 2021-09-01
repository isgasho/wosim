use ash::{
    prelude::VkResult,
    vk::{
        self, AccessFlags, BufferCopy, BufferImageCopy, BufferMemoryBarrier, ClearColorValue,
        ClearValue, CommandBufferAllocateInfo, CommandBufferBeginInfo,
        CommandBufferInheritanceInfo, CommandBufferLevel, CommandBufferUsageFlags,
        CommandPoolResetFlags, DependencyFlags, Extent3D, Filter, ImageBlit, ImageLayout,
        ImageMemoryBarrier, ImageSubresourceRange, IndexType, MemoryBarrier, Offset3D,
        PipelineBindPoint, PipelineStageFlags, Rect2D, RenderPassBeginInfo, ShaderStageFlags,
        SubpassContents, Viewport,
    },
};
use bytemuck::{bytes_of, Pod};

use super::{
    Buffer, DescriptorSet, Framebuffer, Image, Object, Pipeline, PipelineLayout, QueryPool,
    RenderPass,
};

pub type CommandPool = Object<vk::CommandPool>;
pub type CommandBuffer = Object<vk::CommandBuffer>;

impl CommandPool {
    pub fn allocate(&self, level: CommandBufferLevel, count: u32) -> VkResult<Vec<CommandBuffer>> {
        let create_info = CommandBufferAllocateInfo::builder()
            .command_pool(self.handle)
            .level(level)
            .command_buffer_count(count);
        Ok(
            unsafe { self.device.inner.allocate_command_buffers(&create_info) }?
                .into_iter()
                .map(|handle| CommandBuffer {
                    handle,
                    device: self.device.clone(),
                })
                .collect(),
        )
    }

    pub fn allocate_single_primary(&self) -> VkResult<CommandBuffer> {
        Ok(self.allocate(CommandBufferLevel::PRIMARY, 1)?.remove(0))
    }

    pub fn reset(&self, flags: CommandPoolResetFlags) -> VkResult<()> {
        unsafe { self.device.inner.reset_command_pool(self.handle, flags) }
    }
}

impl CommandBuffer {
    pub fn begin(
        &self,
        flags: CommandBufferUsageFlags,
        inheritance: Option<&CommandBufferInheritanceInfo>,
    ) -> VkResult<()> {
        let begin_info = if let Some(inheritance) = inheritance {
            CommandBufferBeginInfo::builder().inheritance_info(inheritance)
        } else {
            CommandBufferBeginInfo::builder()
        }
        .flags(flags);
        unsafe {
            self.device
                .inner
                .begin_command_buffer(self.handle, &begin_info)
        }
    }

    pub fn begin_render_pass(
        &self,
        render_pass: &RenderPass,
        framebuffer: &Framebuffer,
        render_area: Rect2D,
        clear_values: &[ClearValue],
        contents: SubpassContents,
    ) {
        let create_info = RenderPassBeginInfo::builder()
            .render_pass(render_pass.handle)
            .framebuffer(framebuffer.handle)
            .render_area(render_area)
            .clear_values(clear_values);
        unsafe {
            self.device
                .inner
                .cmd_begin_render_pass(self.handle, &create_info, contents)
        }
    }

    pub fn clear_color_image(
        &self,
        image: &Image,
        image_layout: ImageLayout,
        clear_color_value: &ClearColorValue,
        ranges: &[ImageSubresourceRange],
    ) {
        unsafe {
            self.device.inner.cmd_clear_color_image(
                self.handle,
                image.handle,
                image_layout,
                clear_color_value,
                ranges,
            )
        }
    }

    pub fn fill_buffer(&self, buffer: &Buffer, offset: u64, size: u64, data: u32) {
        unsafe {
            self.device
                .inner
                .cmd_fill_buffer(self.handle, buffer.handle, offset, size, data)
        }
    }

    pub fn copy_buffer(&self, src_buffer: &Buffer, dst_buffer: &Buffer, regions: &[BufferCopy]) {
        unsafe {
            self.device.inner.cmd_copy_buffer(
                self.handle,
                src_buffer.handle,
                dst_buffer.handle,
                regions,
            )
        }
    }

    pub fn copy_buffer_to_image(
        &self,
        src_buffer: &Buffer,
        dst_image: &Image,
        dst_image_layout: ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        unsafe {
            self.device.inner.cmd_copy_buffer_to_image(
                self.handle,
                src_buffer.handle,
                dst_image.handle,
                dst_image_layout,
                regions,
            )
        }
    }

    pub fn next_subpass(&self, contents: SubpassContents) {
        unsafe { self.device.inner.cmd_next_subpass(self.handle, contents) }
    }

    pub fn dispatch(&self, group_count_x: u32, group_count_y: u32, group_count_z: u32) {
        unsafe {
            self.device
                .inner
                .cmd_dispatch(self.handle, group_count_x, group_count_y, group_count_z)
        }
    }

    pub fn draw(
        &self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.inner.cmd_draw(
                self.handle,
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            )
        };
    }

    pub fn draw_indexed(
        &self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.inner.cmd_draw_indexed(
                self.handle,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            )
        }
    }

    pub fn draw_indexed_indirect(
        &self,
        buffer: &Buffer,
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) {
        unsafe {
            self.device.inner.cmd_draw_indexed_indirect(
                self.handle,
                buffer.handle,
                offset,
                draw_count,
                stride,
            )
        }
    }

    pub fn draw_indexed_indirect_count(
        &self,
        buffer: &Buffer,
        offset: u64,
        count_buffer: &Buffer,
        count_buffer_offset: u64,
        max_draw_count: u32,
        stride: u32,
    ) {
        unsafe {
            self.device.inner.cmd_draw_indexed_indirect_count(
                self.handle,
                buffer.handle,
                offset,
                count_buffer.handle,
                count_buffer_offset,
                max_draw_count,
                stride,
            )
        }
    }

    pub fn bind_index_buffer(&self, buffer: &Buffer, offset: u64, index_type: IndexType) {
        unsafe {
            self.device
                .inner
                .cmd_bind_index_buffer(self.handle, buffer.handle, offset, index_type)
        }
    }

    pub fn bind_vertex_buffers(&self, first_binding: u32, buffers: &[(&Buffer, u64)]) {
        let offsets: Vec<_> = buffers.iter().map(|(_, offset)| *offset).collect();
        let buffers: Vec<_> = buffers.iter().map(|(buffer, _)| buffer.handle).collect();
        unsafe {
            self.device.inner.cmd_bind_vertex_buffers(
                self.handle,
                first_binding,
                &buffers,
                &offsets,
            )
        }
    }

    pub fn push_constants<T: Pod>(
        &self,
        layout: &PipelineLayout,
        stage_flags: ShaderStageFlags,
        offset: u32,
        constant: &T,
    ) {
        unsafe {
            self.device.inner.cmd_push_constants(
                self.handle,
                layout.handle,
                stage_flags,
                offset,
                bytes_of(constant),
            )
        }
    }

    pub fn bind_descriptor_sets(
        &self,
        pipeline_bind_point: PipelineBindPoint,
        layout: &PipelineLayout,
        first_set: u32,
        descriptor_sets: &[&DescriptorSet],
        dynamic_offsets: &[u32],
    ) {
        let descriptor_sets: Vec<_> = descriptor_sets
            .iter()
            .map(|descriptor_set| descriptor_set.handle)
            .collect();
        unsafe {
            self.device.inner.cmd_bind_descriptor_sets(
                self.handle,
                pipeline_bind_point,
                layout.handle,
                first_set,
                &descriptor_sets,
                dynamic_offsets,
            )
        }
    }

    pub fn end_render_pass(&self) {
        unsafe { self.device.inner.cmd_end_render_pass(self.handle) }
    }

    pub fn bind_pipeline(&self, binding_point: PipelineBindPoint, pipeline: &Pipeline) {
        unsafe {
            self.device
                .inner
                .cmd_bind_pipeline(self.handle, binding_point, pipeline.handle)
        }
    }

    pub fn end(&self) -> VkResult<()> {
        unsafe { self.device.inner.end_command_buffer(self.handle) }
    }

    pub fn set_viewport(&self, first_viewport: u32, viewports: &[Viewport]) {
        unsafe {
            self.device
                .inner
                .cmd_set_viewport(self.handle, first_viewport, viewports)
        }
    }

    pub fn set_scissor(&self, first_scissor: u32, scissors: &[Rect2D]) {
        unsafe {
            self.device
                .inner
                .cmd_set_scissor(self.handle, first_scissor, scissors)
        }
    }

    pub fn pipeline_barrier(
        &self,
        src_stage_mask: PipelineStageFlags,
        dst_stage_mask: PipelineStageFlags,
        dependency_flags: DependencyFlags,
        memory_barriers: &[MemoryBarrier],
        buffer_memory_barriers: &[BufferMemoryBarrier],
        image_memory_barriers: &[ImageMemoryBarrier],
    ) {
        unsafe {
            self.device.inner.cmd_pipeline_barrier(
                self.handle,
                src_stage_mask,
                dst_stage_mask,
                dependency_flags,
                memory_barriers,
                buffer_memory_barriers,
                image_memory_barriers,
            )
        }
    }

    pub fn write_timestamp(
        &self,
        pipeline_stage: PipelineStageFlags,
        query_pool: &QueryPool,
        query: u32,
    ) {
        unsafe {
            self.device.inner.cmd_write_timestamp(
                self.handle,
                pipeline_stage,
                query_pool.handle,
                query,
            )
        }
    }

    pub fn transfer_buffer_to_image(
        &self,
        src: &Buffer,
        dst: &Image,
        info: ImageTransferInfo,
        subresource_range: ImageSubresourceRange,
        regions: &[BufferImageCopy],
    ) {
        self.pipeline_barrier(
            info.src_stage_mask,
            PipelineStageFlags::TRANSFER,
            DependencyFlags::empty(),
            &[],
            &[],
            &[ImageMemoryBarrier::builder()
                .image(dst.handle)
                .src_access_mask(info.initial_access_mask)
                .dst_access_mask(AccessFlags::TRANSFER_WRITE)
                .old_layout(info.initial_layout)
                .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .subresource_range(subresource_range)
                .build()],
        );
        self.copy_buffer_to_image(src, dst, ImageLayout::TRANSFER_DST_OPTIMAL, regions);
        self.pipeline_barrier(
            PipelineStageFlags::TRANSFER,
            info.dst_stage_mask,
            DependencyFlags::empty(),
            &[],
            &[],
            &[ImageMemoryBarrier::builder()
                .src_access_mask(AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(info.final_access_mask)
                .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(info.final_layout)
                .image(dst.handle)
                .subresource_range(subresource_range)
                .build()],
        );
    }

    pub fn transfer_buffer_to_mipmap_image(
        &self,
        src: &Buffer,
        dst: &Image,
        info: ImageTransferInfo,
        region: BufferImageCopy,
        level_count: u32,
        filter: Filter,
    ) {
        assert!(level_count > 0);
        self.pipeline_barrier(
            info.src_stage_mask,
            PipelineStageFlags::TRANSFER,
            DependencyFlags::empty(),
            &[],
            &[],
            &[ImageMemoryBarrier::builder()
                .image(dst.handle)
                .src_access_mask(info.initial_access_mask)
                .dst_access_mask(AccessFlags::TRANSFER_WRITE)
                .old_layout(info.initial_layout)
                .new_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                .subresource_range(
                    ImageSubresourceRange::builder()
                        .aspect_mask(region.image_subresource.aspect_mask)
                        .base_array_layer(region.image_subresource.base_array_layer)
                        .layer_count(region.image_subresource.layer_count)
                        .base_mip_level(region.image_subresource.mip_level)
                        .level_count(level_count)
                        .build(),
                )
                .build()],
        );
        self.copy_buffer_to_image(src, dst, ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);
        let mut src_extent = region.image_extent;
        let mut src_offset = region.image_offset;
        let mut src_layers = region.image_subresource;
        while src_layers.mip_level + 1 < region.image_subresource.mip_level + level_count {
            let dst_extent = half_extent(src_extent);
            let dst_offset = half_offset(src_offset);
            let mut dst_layers = src_layers;
            dst_layers.mip_level += 1;
            self.pipeline_barrier(
                PipelineStageFlags::TRANSFER,
                PipelineStageFlags::TRANSFER,
                DependencyFlags::empty(),
                &[],
                &[],
                &[ImageMemoryBarrier::builder()
                    .image(dst.handle)
                    .src_access_mask(AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(AccessFlags::TRANSFER_READ)
                    .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .subresource_range(
                        ImageSubresourceRange::builder()
                            .aspect_mask(src_layers.aspect_mask)
                            .base_array_layer(src_layers.base_array_layer)
                            .layer_count(src_layers.layer_count)
                            .base_mip_level(src_layers.mip_level)
                            .level_count(1)
                            .build(),
                    )
                    .build()],
            );
            let regions = [ImageBlit::builder()
                .src_offsets([src_offset, add_extent(src_offset, src_extent)])
                .dst_offsets([dst_offset, add_extent(dst_offset, dst_extent)])
                .src_subresource(src_layers)
                .dst_subresource(dst_layers)
                .build()];
            self.blit_image(
                dst,
                ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst,
                ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
                filter,
            );
            src_extent = dst_extent;
            src_offset = dst_offset;
            src_layers = dst_layers;
        }
        if level_count > 1 {
            self.pipeline_barrier(
                PipelineStageFlags::TRANSFER,
                info.dst_stage_mask,
                DependencyFlags::empty(),
                &[],
                &[],
                &[
                    ImageMemoryBarrier::builder()
                        .src_access_mask(AccessFlags::TRANSFER_READ)
                        .dst_access_mask(info.final_access_mask)
                        .old_layout(ImageLayout::TRANSFER_SRC_OPTIMAL)
                        .new_layout(info.final_layout)
                        .image(dst.handle)
                        .subresource_range(
                            ImageSubresourceRange::builder()
                                .aspect_mask(region.image_subresource.aspect_mask)
                                .base_array_layer(region.image_subresource.base_array_layer)
                                .layer_count(region.image_subresource.layer_count)
                                .base_mip_level(region.image_subresource.mip_level)
                                .level_count(level_count - 1)
                                .build(),
                        )
                        .build(),
                    ImageMemoryBarrier::builder()
                        .src_access_mask(AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(info.final_access_mask)
                        .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                        .new_layout(info.final_layout)
                        .image(dst.handle)
                        .subresource_range(
                            ImageSubresourceRange::builder()
                                .aspect_mask(region.image_subresource.aspect_mask)
                                .base_array_layer(region.image_subresource.base_array_layer)
                                .layer_count(region.image_subresource.layer_count)
                                .base_mip_level(
                                    region.image_subresource.mip_level + level_count - 1,
                                )
                                .level_count(1)
                                .build(),
                        )
                        .build(),
                ],
            );
        } else {
            self.pipeline_barrier(
                PipelineStageFlags::TRANSFER,
                info.dst_stage_mask,
                DependencyFlags::empty(),
                &[],
                &[],
                &[ImageMemoryBarrier::builder()
                    .src_access_mask(AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(info.final_access_mask)
                    .old_layout(ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(info.final_layout)
                    .image(dst.handle)
                    .subresource_range(
                        ImageSubresourceRange::builder()
                            .aspect_mask(region.image_subresource.aspect_mask)
                            .base_array_layer(region.image_subresource.base_array_layer)
                            .layer_count(region.image_subresource.layer_count)
                            .base_mip_level(region.image_subresource.mip_level)
                            .level_count(1)
                            .build(),
                    )
                    .build()],
            );
        }
    }

    pub fn blit_image(
        &self,
        src: &Image,
        src_layout: ImageLayout,
        dst: &Image,
        dst_layout: ImageLayout,
        regions: &[ImageBlit],
        filter: Filter,
    ) {
        unsafe {
            self.device.inner.cmd_blit_image(
                self.handle,
                src.handle,
                src_layout,
                dst.handle,
                dst_layout,
                regions,
                filter,
            )
        }
    }

    pub fn reset_query_pool(&self, pool: &QueryPool, first_query: u32, query_count: u32) {
        unsafe {
            self.device.inner.cmd_reset_query_pool(
                self.handle,
                pool.handle,
                first_query,
                query_count,
            )
        }
    }
}

pub struct ImageTransferInfo {
    pub src_stage_mask: PipelineStageFlags,
    pub dst_stage_mask: PipelineStageFlags,
    pub initial_access_mask: AccessFlags,
    pub final_access_mask: AccessFlags,
    pub initial_layout: ImageLayout,
    pub final_layout: ImageLayout,
}

pub struct ImageBlitInfo {
    pub src_stage_mask: PipelineStageFlags,
    pub dst_stage_mask: PipelineStageFlags,
    pub initial_src_access_mask: AccessFlags,
    pub final_src_access_mask: AccessFlags,
    pub initial_src_layout: ImageLayout,
    pub final_src_layout: ImageLayout,
    pub initial_dst_access_mask: AccessFlags,
    pub final_dst_access_mask: AccessFlags,
    pub initial_dst_layout: ImageLayout,
    pub final_dst_layout: ImageLayout,
    pub filter: Filter,
}

pub fn half_extent(e: Extent3D) -> Extent3D {
    Extent3D {
        width: (e.width / 2).max(1),
        height: (e.height / 2).max(1),
        depth: (e.depth / 2).max(1),
    }
}

pub fn half_offset(o: Offset3D) -> Offset3D {
    Offset3D {
        x: o.x / 2,
        y: o.y / 2,
        z: o.z / 2,
    }
}

pub fn add_extent(o: Offset3D, e: Extent3D) -> Offset3D {
    Offset3D {
        x: o.x + e.width as i32,
        y: o.y + e.height as i32,
        z: o.z + e.depth as i32,
    }
}
