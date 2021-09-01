use std::{mem::size_of, sync::Arc};

use egui::{epaint::Vertex, ClippedMesh};
use nalgebra::Vector2;
use vulkan::{
    AccessFlags, Buffer, BufferCreateInfo, BufferImageCopy, BufferUsageFlags, ComponentMapping,
    ComponentSwizzle, DescriptorImageInfo, DescriptorPool, DescriptorPoolSetup, DescriptorSet,
    DescriptorType, Device, Extent2D, Extent3D, Format, GpuVec, ImageAspectFlags, ImageCreateInfo,
    ImageLayout, ImageSubresourceLayers, ImageSubresourceRange, ImageTiling, ImageTransferInfo,
    ImageType, ImageUsageFlags, ImageViewCreateFlags, ImageViewType, IndexType, MemoryLocation,
    Offset2D, Pipeline, PipelineBindPoint, PipelineStageFlags, Rect2D, SampleCountFlags,
    ShaderStageFlags, SharingMode, WriteDescriptorSet,
};

use crate::root::{RootFrame, RootSurface};

use super::{EguiContext, Font};

pub struct EguiFrame {
    indices: GpuVec<u32>,
    vertices: GpuVec<Vertex>,
    descriptor_set: DescriptorSet,
    _staging_buffer: Option<Buffer>,
    font: Option<Arc<Font>>,
}

impl EguiFrame {
    pub fn new(
        device: &Arc<Device>,
        context: &EguiContext,
        descriptor_pool: &DescriptorPool,
    ) -> Result<Self, vulkan::Error> {
        let mut descriptor_sets = descriptor_pool.allocate(&[&context.set_layout])?;
        let descriptor_set = descriptor_sets.remove(0);
        Ok(Self {
            indices: device.create_vec(
                1024,
                BufferUsageFlags::INDEX_BUFFER,
                MemoryLocation::CpuToGpu,
            )?,
            vertices: device.create_vec(
                1024,
                BufferUsageFlags::VERTEX_BUFFER,
                MemoryLocation::CpuToGpu,
            )?,
            descriptor_set,
            _staging_buffer: None,
            font: None,
        })
    }

    pub fn prepare(
        &mut self,
        context: &mut EguiContext,
        root_frame: &mut RootFrame,
        device: &Arc<Device>,
    ) -> Result<(), vulkan::Error> {
        self._staging_buffer = None;
        if context.meshes.is_empty() {
            return Ok(());
        }
        let texture = context.inner.fonts().texture();
        if if let Some(font) = context.font.as_ref() {
            font.version != texture.version
        } else {
            true
        } {
            let extent = Extent3D {
                width: texture.width as u32,
                height: texture.height as u32,
                depth: 1,
            };
            let create_info = BufferCreateInfo::builder()
                .usage(BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(SharingMode::EXCLUSIVE)
                .size(texture.pixels.len() as u64);
            let buffer = device.create_buffer(&create_info, MemoryLocation::CpuToGpu)?;
            unsafe {
                buffer
                    .mapped_ptr()
                    .unwrap()
                    .cast::<u8>()
                    .as_ptr()
                    .copy_from_nonoverlapping(texture.pixels.as_ptr(), texture.pixels.len())
            }
            device.flush_mapped_memory_ranges(&[buffer.range()])?;
            let create_info = ImageCreateInfo::builder()
                .format(Format::R8_UNORM)
                .initial_layout(ImageLayout::UNDEFINED)
                .samples(SampleCountFlags::TYPE_1)
                .tiling(ImageTiling::OPTIMAL)
                .usage(ImageUsageFlags::SAMPLED | ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(SharingMode::EXCLUSIVE)
                .image_type(ImageType::TYPE_2D)
                .mip_levels(1)
                .array_layers(1)
                .extent(extent);
            let image = device.create_image(&create_info, MemoryLocation::GpuOnly)?;
            root_frame.command_buffer.transfer_buffer_to_image(
                &buffer,
                &image,
                ImageTransferInfo {
                    initial_access_mask: AccessFlags::empty(),
                    src_stage_mask: PipelineStageFlags::HOST,
                    dst_stage_mask: PipelineStageFlags::FRAGMENT_SHADER,
                    final_access_mask: AccessFlags::SHADER_READ,
                    initial_layout: ImageLayout::UNDEFINED,
                    final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                },
                ImageSubresourceRange::builder()
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .base_array_layer(0)
                    .layer_count(1)
                    .base_mip_level(0)
                    .level_count(1)
                    .build(),
                &[BufferImageCopy::builder()
                    .image_subresource(
                        ImageSubresourceLayers::builder()
                            .aspect_mask(ImageAspectFlags::COLOR)
                            .base_array_layer(0)
                            .layer_count(1)
                            .mip_level(0)
                            .build(),
                    )
                    .image_extent(extent)
                    .build()],
            );
            let view = device.create_image_view(
                ImageViewCreateFlags::empty(),
                &image,
                ImageViewType::TYPE_2D,
                Format::R8_UNORM,
                ComponentMapping {
                    r: ComponentSwizzle::R,
                    g: ComponentSwizzle::R,
                    b: ComponentSwizzle::R,
                    a: ComponentSwizzle::R,
                },
                ImageSubresourceRange::builder()
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .base_array_layer(0)
                    .base_mip_level(0)
                    .layer_count(1)
                    .level_count(1)
                    .build(),
            )?;
            self._staging_buffer = Some(buffer);
            context.font = Some(Arc::new(Font {
                view,
                _image: image,
                version: texture.version,
            }));
        }
        if self.font.as_ref().map(|f| f.version) != context.font.as_ref().map(|f| f.version) {
            let sampler_info = [DescriptorImageInfo::builder()
                .sampler(*context.sampler)
                .build()];
            let image_info = [DescriptorImageInfo::builder()
                .image_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(*context.font.as_ref().unwrap().view)
                .build()];
            device.update_descriptor_sets(
                &[
                    WriteDescriptorSet::builder()
                        .dst_set(*self.descriptor_set)
                        .dst_binding(0)
                        .dst_array_element(0)
                        .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                        .image_info(&image_info)
                        .build(),
                    WriteDescriptorSet::builder()
                        .dst_set(*self.descriptor_set)
                        .dst_binding(1)
                        .dst_array_element(0)
                        .descriptor_type(DescriptorType::SAMPLER)
                        .image_info(&sampler_info)
                        .build(),
                ],
                &[],
            );
            self.font = context.font.clone();
        }
        Ok(())
    }

    pub fn render(
        &mut self,
        root_frame: &mut RootFrame,
        root_surface: &RootSurface,
        context: &mut EguiContext,
        device: &Arc<Device>,
        pipeline: &Pipeline,
    ) -> Result<(), vulkan::Error> {
        if context.meshes.is_empty() {
            return Ok(());
        }
        let mut index_count = 0;
        let mut vertex_count = 0;
        for ClippedMesh(_, mesh) in context.meshes.iter() {
            index_count += mesh.indices.len();
            vertex_count += mesh.vertices.len();
        }
        self.indices.clear();
        self.indices.reserve(index_count)?;
        self.vertices.clear();
        self.vertices.reserve(vertex_count)?;
        let command_buffer = &root_frame.command_buffer;
        command_buffer.bind_pipeline(PipelineBindPoint::GRAPHICS, pipeline);
        command_buffer.bind_vertex_buffers(0, &[(self.vertices.buffer(), 0)]);
        command_buffer.bind_index_buffer(self.indices.buffer(), 0, IndexType::UINT32);
        command_buffer.bind_descriptor_sets(
            PipelineBindPoint::GRAPHICS,
            &context.pipeline_layout,
            0,
            &[&self.descriptor_set],
            &[],
        );
        let extent = root_surface.swapchain.image_extent();
        command_buffer.push_constants(
            &context.pipeline_layout,
            ShaderStageFlags::VERTEX,
            0,
            &[extent.width as f32, extent.height as f32],
        );
        for ClippedMesh(clip, mesh) in context.meshes.iter() {
            let scissors = [Rect2D {
                offset: Offset2D {
                    x: clip.min.x as i32,
                    y: clip.min.y as i32,
                },
                extent: Extent2D {
                    width: (clip.max.x - clip.min.x) as u32,
                    height: (clip.max.y - clip.min.y) as u32,
                },
            }];
            command_buffer.set_scissor(0, &scissors);
            let texture_id = match mesh.texture_id {
                egui::TextureId::Egui => 0,
                egui::TextureId::User(id) => id as u32 + 1,
            };
            command_buffer.push_constants(
                &context.pipeline_layout,
                ShaderStageFlags::FRAGMENT,
                size_of::<Vector2<f32>>() as u32,
                &texture_id,
            );
            command_buffer.draw_indexed(
                mesh.indices.len() as u32,
                1,
                self.indices.len() as u32,
                self.vertices.len() as i32,
                0,
            );
            self.indices.append(&mesh.indices);
            self.vertices.append(&mesh.vertices);
        }
        device.flush_mapped_memory_ranges(&[self.indices.range(), self.vertices.range()])?;
        Ok(())
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        DescriptorPoolSetup {
            sets: 1,
            samplers: 1,
            sampled_images: 1,
            ..Default::default()
        }
    }
}
