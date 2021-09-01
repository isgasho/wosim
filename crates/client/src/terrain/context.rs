use std::collections::HashMap;

use protocol::RegionPos;
use vulkan::{
    AccessFlags, Buffer, BufferCreateInfo, BufferImageCopy, BufferUsageFlags, ComponentMapping,
    ComponentSwizzle, DependencyFlags, DescriptorSetLayout, DescriptorSetLayoutBinding,
    DescriptorSetLayoutCreateFlags, DescriptorType, Extent3D, Filter, Format, Image,
    ImageAspectFlags, ImageCreateInfo, ImageLayout, ImageMemoryBarrier, ImageSubresourceLayers,
    ImageSubresourceRange, ImageTiling, ImageTransferInfo, ImageType, ImageUsageFlags, ImageView,
    ImageViewCreateFlags, ImageViewType, MemoryLocation, PipelineLayout, PipelineLayoutCreateFlags,
    PipelineStageFlags, SampleCountFlags, Sampler, SamplerAddressMode, SamplerCreateInfo,
    SamplerMipmapMode, ShaderStageFlags, SharingMode,
};

use crate::{
    root::{RootContext, RootFrame},
    world::World,
};

pub struct TerrainContext {
    pub image: Image,
    pub image_view: ImageView,
    pub sampler: Sampler,
    pub last_update_frame: usize,
    pub pipeline_layout: PipelineLayout,
    pub set_layout: DescriptorSetLayout,
    initialized: bool,
    changes: Vec<Change>,
    add_counter: u64,
    unused: Vec<usize>,
    pub used: HashMap<RegionPos, usize>,
}

pub enum Change {
    Add(RegionPos, Vec<u8>),
    Remove(RegionPos),
}

impl TerrainContext {
    pub fn new(root_context: &RootContext, world: &World) -> eyre::Result<Self> {
        let device = &root_context.device;
        let bindings = [
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::TESSELLATION_EVALUATION | ShaderStageFlags::FRAGMENT)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                .stage_flags(ShaderStageFlags::TESSELLATION_EVALUATION)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(2)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::SAMPLER)
                .stage_flags(ShaderStageFlags::TESSELLATION_EVALUATION)
                .build(),
        ];
        let set_layout = device
            .create_descriptor_set_layout(DescriptorSetLayoutCreateFlags::empty(), &bindings)?;
        let pipeline_layout = device.create_pipeline_layout(
            PipelineLayoutCreateFlags::empty(),
            &[&set_layout],
            &[],
        )?;
        let extent = Extent3D {
            width: world.region_size + 1,
            height: world.region_size + 1,
            depth: 1,
        };
        let mip_levels = (world.region_size + 1).next_power_of_two().log2() + 1;
        let sampler = device.create_sampler(
            &SamplerCreateInfo::builder()
                .address_mode_u(SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(SamplerAddressMode::CLAMP_TO_EDGE)
                .anisotropy_enable(false)
                .min_filter(Filter::LINEAR)
                .mag_filter(Filter::LINEAR)
                .mipmap_mode(SamplerMipmapMode::LINEAR)
                .min_lod(0.0)
                .max_lod((mip_levels - 1) as f32),
        )?;
        let create_info = ImageCreateInfo::builder()
            .format(Format::R8_UNORM)
            .initial_layout(ImageLayout::UNDEFINED)
            .samples(SampleCountFlags::TYPE_1)
            .tiling(ImageTiling::OPTIMAL)
            .usage(
                ImageUsageFlags::SAMPLED
                    | ImageUsageFlags::TRANSFER_DST
                    | ImageUsageFlags::TRANSFER_SRC,
            )
            .sharing_mode(SharingMode::EXCLUSIVE)
            .image_type(ImageType::TYPE_2D)
            .mip_levels(mip_levels)
            .array_layers(world.max_active_regions)
            .extent(extent);
        let image = device.create_image(&create_info, MemoryLocation::GpuOnly)?;
        let image_view = device.create_image_view(
            ImageViewCreateFlags::empty(),
            &image,
            ImageViewType::TYPE_2D_ARRAY,
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
                .layer_count(world.max_active_regions)
                .level_count(mip_levels)
                .build(),
        )?;
        Ok(Self {
            sampler,
            image,
            image_view,
            pipeline_layout,
            set_layout,
            used: HashMap::new(),
            unused: (0..world.max_active_regions as usize).rev().collect(),
            changes: Vec::new(),
            add_counter: 0,
            initialized: false,
            last_update_frame: 0,
        })
    }

    pub fn add(&mut self, pos: RegionPos, heights: Vec<u8>) {
        self.changes.push(Change::Add(pos, heights));
        self.add_counter += 1;
    }

    pub fn remove(&mut self, pos: RegionPos) {
        self.changes.push(Change::Remove(pos));
    }

    pub fn prepare_render(
        &mut self,
        root_context: &RootContext,
        root_frame: &RootFrame,
        world: &World,
        staging_buffer: &mut Option<Buffer>,
    ) -> Result<(), vulkan::Error> {
        let command_buffer = &root_frame.command_buffer;
        let device = &root_context.device;
        let region_size = world.region_size;
        if !self.initialized {
            command_buffer.pipeline_barrier(
                PipelineStageFlags::BOTTOM_OF_PIPE,
                PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
                DependencyFlags::empty(),
                &[],
                &[],
                &[ImageMemoryBarrier::builder()
                    .image(*self.image)
                    .src_access_mask(AccessFlags::empty())
                    .dst_access_mask(AccessFlags::SHADER_READ)
                    .old_layout(ImageLayout::UNDEFINED)
                    .new_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .subresource_range(
                        ImageSubresourceRange::builder()
                            .base_array_layer(0)
                            .base_mip_level(0)
                            .layer_count(world.max_active_regions)
                            .level_count((world.region_size + 1).next_power_of_two().log2() + 1)
                            .aspect_mask(ImageAspectFlags::COLOR)
                            .build(),
                    )
                    .build()],
            );
            self.initialized = true;
        }

        let rebuild = !self.changes.is_empty();
        *staging_buffer = if self.add_counter > 0 {
            let create_info = BufferCreateInfo::builder()
                .usage(BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(SharingMode::EXCLUSIVE)
                .size((region_size + 1) as u64 * (region_size + 1) as u64 * self.add_counter);
            let buffer = device.create_buffer(&create_info, MemoryLocation::CpuToGpu)?;
            let mut data = buffer.mapped_ptr().unwrap().cast::<u8>().as_ptr();
            for change in self.changes.iter() {
                if let Change::Add(_, heights) = change {
                    unsafe {
                        data.copy_from_nonoverlapping(
                            heights.as_ptr(),
                            (region_size + 1) as usize * (region_size + 1) as usize,
                        );
                    }
                    data = unsafe {
                        data.add((region_size + 1) as usize * (region_size + 1) as usize)
                    };
                }
            }
            self.add_counter = 0;
            Some(buffer)
        } else {
            None
        };
        let mut buffer_offset = 0;
        let extent = Extent3D {
            width: region_size + 1,
            height: region_size + 1,
            depth: 1,
        };
        for change in self.changes.drain(..) {
            match change {
                Change::Add(pos, _) => {
                    let mip_levels = (world.region_size + 1).next_power_of_two().log2() + 1;
                    let index = self.unused.pop().unwrap();
                    self.used.insert(pos, index);
                    command_buffer.transfer_buffer_to_mipmap_image(
                        staging_buffer.as_ref().unwrap(),
                        &self.image,
                        ImageTransferInfo {
                            src_stage_mask: PipelineStageFlags::HOST,
                            initial_access_mask: AccessFlags::empty(),
                            dst_stage_mask: PipelineStageFlags::TESSELLATION_EVALUATION_SHADER,
                            final_access_mask: AccessFlags::SHADER_READ,
                            initial_layout: ImageLayout::UNDEFINED,
                            final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        },
                        BufferImageCopy::builder()
                            .image_subresource(
                                ImageSubresourceLayers::builder()
                                    .aspect_mask(ImageAspectFlags::COLOR)
                                    .base_array_layer(index as u32)
                                    .layer_count(1)
                                    .mip_level(0)
                                    .build(),
                            )
                            .buffer_offset(buffer_offset)
                            .buffer_row_length(region_size + 1)
                            .buffer_image_height(region_size + 1)
                            .image_extent(extent)
                            .build(),
                        mip_levels,
                        Filter::LINEAR,
                    );
                    buffer_offset += (region_size + 1) as u64 * (region_size + 1) as u64;
                }
                Change::Remove(pos) => {
                    let index = self.used.remove(&pos).unwrap();
                    self.unused.push(index);
                }
            }
        }
        if rebuild {
            self.last_update_frame = root_context.frame_count;
        }
        Ok(())
    }
}
