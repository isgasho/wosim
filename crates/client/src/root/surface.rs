use std::sync::Arc;

use vulkan::{
    ApiResult, ComponentMapping, Extent3D, Framebuffer, FramebufferCreateFlags, Image,
    ImageAspectFlags, ImageCreateInfo, ImageLayout, ImageSubresourceRange, ImageTiling, ImageType,
    ImageUsageFlags, ImageView, ImageViewCreateFlags, ImageViewType, MemoryLocation, RenderPass,
    SampleCountFlags, Swapchain, SwapchainImage,
};

use super::RootContext;

pub struct RootSurface {
    pub framebuffers: Vec<Framebuffer>,
    pub depth_view: ImageView,
    pub depth_image: Image,
    pub images: Vec<SwapchainImage>,
    pub swapchain: Arc<Swapchain>,
}

impl RootSurface {
    pub fn new(context: &RootContext, render_pass: &RenderPass) -> eyre::Result<Self> {
        let images = context.swapchain.images()?;
        let image_extent = context.swapchain.image_extent();
        let create_info = ImageCreateInfo::builder()
            .extent(Extent3D {
                width: image_extent.width,
                height: image_extent.height,
                depth: 1,
            })
            .tiling(ImageTiling::OPTIMAL)
            .image_type(ImageType::TYPE_2D)
            .usage(ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | ImageUsageFlags::SAMPLED)
            .initial_layout(ImageLayout::UNDEFINED)
            .mip_levels(1)
            .array_layers(1)
            .samples(SampleCountFlags::TYPE_1)
            .format(context.render_configuration.depth_format);
        let depth_image = context
            .device
            .create_image(&create_info, MemoryLocation::GpuOnly)?;
        let depth_view = context.device.create_image_view(
            ImageViewCreateFlags::empty(),
            &depth_image,
            ImageViewType::TYPE_2D,
            context.render_configuration.depth_format,
            ComponentMapping::default(),
            ImageSubresourceRange::builder()
                .aspect_mask(ImageAspectFlags::DEPTH)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1)
                .build(),
        )?;
        let framebuffers: Result<_, ApiResult> = images
            .iter()
            .map(|image| {
                render_pass.create_framebuffer(
                    FramebufferCreateFlags::empty(),
                    &[image.view(), &depth_view, &depth_view, image.view()],
                    image_extent.width,
                    image_extent.height,
                    1,
                )
            })
            .collect();
        let framebuffers = framebuffers?;
        Ok(Self {
            framebuffers,
            depth_image,
            depth_view,
            images,
            swapchain: context.swapchain.clone(),
        })
    }
}
