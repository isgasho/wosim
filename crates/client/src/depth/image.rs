use vulkan::{
    AccessFlags, ClearColorValue, ComponentMapping, DependencyFlags, DescriptorImageInfo,
    DescriptorType, Extent2D, Extent3D, Image, ImageAspectFlags, ImageCreateInfo, ImageLayout,
    ImageMemoryBarrier, ImageSubresourceRange, ImageTiling, ImageType, ImageUsageFlags, ImageView,
    ImageViewCreateFlags, ImageViewType, MemoryLocation, PipelineStageFlags, SampleCountFlags,
    Sampler, WriteDescriptorSet,
};

use crate::root::{RootContext, RootFrame, RootSurface};

use super::DepthContext;

pub struct DepthImage {
    pub views: Vec<ImageView>,
    pub view: ImageView,
    pub image: Image,
}

impl DepthImage {
    pub fn new(
        root_context: &RootContext,
        root_surface: &RootSurface,
        root_frame: &RootFrame,
        context: &DepthContext,
        image_extent: Extent2D,
        mip_levels: u32,
        sampler: &Sampler,
    ) -> Result<Self, vulkan::Error> {
        let device = &root_context.device;
        let format = root_context.render_configuration.depth_pyramid_format;
        let create_info = ImageCreateInfo::builder()
            .extent(Extent3D {
                width: image_extent.width,
                height: image_extent.height,
                depth: 1,
            })
            .tiling(ImageTiling::OPTIMAL)
            .image_type(ImageType::TYPE_2D)
            .usage(
                ImageUsageFlags::SAMPLED | ImageUsageFlags::STORAGE | ImageUsageFlags::TRANSFER_DST,
            )
            .initial_layout(ImageLayout::UNDEFINED)
            .mip_levels(mip_levels)
            .array_layers(1)
            .samples(SampleCountFlags::TYPE_1)
            .format(format);
        let image = device.create_image(&create_info, MemoryLocation::GpuOnly)?;
        let view = device.create_image_view(
            ImageViewCreateFlags::empty(),
            &image,
            ImageViewType::TYPE_2D,
            format,
            ComponentMapping::default(),
            ImageSubresourceRange::builder()
                .aspect_mask(ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(mip_levels)
                .base_array_layer(0)
                .layer_count(1)
                .build(),
        )?;
        let mut views = Vec::with_capacity(mip_levels as usize);
        let mut image_infos = Vec::with_capacity(mip_levels as usize);
        let mut descriptor_writes = Vec::with_capacity(Self::MAX_MIP_LEVELS * 2);
        assert!(mip_levels > 0);
        for i in 0..mip_levels {
            let pyramid_view = device.create_image_view(
                ImageViewCreateFlags::empty(),
                &image,
                ImageViewType::TYPE_2D,
                format,
                ComponentMapping::default(),
                ImageSubresourceRange::builder()
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .base_mip_level(i)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )?;
            let dst_view = *pyramid_view;
            views.push(pyramid_view);
            let src_layout = if i == 0 {
                ImageLayout::SHADER_READ_ONLY_OPTIMAL
            } else {
                ImageLayout::GENERAL
            };
            let src_view = if i == 0 {
                *root_surface.depth_view
            } else {
                *views[(i - 1) as usize]
            };
            image_infos.push([
                DescriptorImageInfo::builder()
                    .image_layout(src_layout)
                    .image_view(src_view)
                    .build(),
                DescriptorImageInfo::builder().sampler(**sampler).build(),
                DescriptorImageInfo::builder()
                    .image_layout(ImageLayout::GENERAL)
                    .image_view(dst_view)
                    .build(),
            ])
        }
        for i in 0..mip_levels {
            descriptor_writes.push(
                WriteDescriptorSet::builder()
                    .dst_set(*context.descriptor_sets[i as usize])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                    .image_info(&image_infos[i as usize][0..1])
                    .build(),
            );
            descriptor_writes.push(
                WriteDescriptorSet::builder()
                    .dst_set(*context.descriptor_sets[i as usize])
                    .dst_binding(1)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::SAMPLER)
                    .image_info(&image_infos[i as usize][1..2])
                    .build(),
            );
            descriptor_writes.push(
                WriteDescriptorSet::builder()
                    .dst_set(*context.descriptor_sets[i as usize])
                    .dst_binding(2)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::STORAGE_IMAGE)
                    .image_info(&image_infos[i as usize][2..3])
                    .build(),
            );
        }
        for i in mip_levels..Self::MAX_MIP_LEVELS as u32 {
            descriptor_writes.push(
                WriteDescriptorSet::builder()
                    .dst_set(*context.descriptor_sets[i as usize])
                    .dst_binding(0)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                    .image_info(&image_infos[mip_levels as usize - 1][0..1])
                    .build(),
            );
            descriptor_writes.push(
                WriteDescriptorSet::builder()
                    .dst_set(*context.descriptor_sets[i as usize])
                    .dst_binding(1)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::SAMPLER)
                    .image_info(&image_infos[mip_levels as usize - 1][1..2])
                    .build(),
            );
            descriptor_writes.push(
                WriteDescriptorSet::builder()
                    .dst_set(*context.descriptor_sets[i as usize])
                    .dst_binding(2)
                    .dst_array_element(0)
                    .descriptor_type(DescriptorType::STORAGE_IMAGE)
                    .image_info(&image_infos[mip_levels as usize - 1][2..3])
                    .build(),
            );
        }
        device.update_descriptor_sets(&descriptor_writes, &[]);
        let subresource_range = ImageSubresourceRange::builder()
            .aspect_mask(ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(views.len() as u32)
            .base_array_layer(0)
            .layer_count(1)
            .build();
        let image_memory_barriers = [ImageMemoryBarrier::builder()
            .image(*image)
            .src_access_mask(AccessFlags::empty())
            .dst_access_mask(AccessFlags::TRANSFER_WRITE)
            .old_layout(ImageLayout::UNDEFINED)
            .new_layout(ImageLayout::GENERAL)
            .subresource_range(subresource_range)
            .build()];
        root_frame.command_buffer.pipeline_barrier(
            PipelineStageFlags::TOP_OF_PIPE,
            PipelineStageFlags::TRANSFER,
            DependencyFlags::empty(),
            &[],
            &[],
            &image_memory_barriers,
        );
        root_frame.command_buffer.clear_color_image(
            &image,
            ImageLayout::GENERAL,
            &ClearColorValue {
                float32: [0.0, 0.0, 0.0, 0.0],
            },
            &[subresource_range],
        );
        Ok(Self { views, view, image })
    }

    pub const MAX_MIP_LEVELS: usize = 13;
}
