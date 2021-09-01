use std::{ffi::CStr, ops::Deref, sync::Arc};

use ash::{
    extensions::khr,
    prelude::VkResult,
    vk::{
        self, CompositeAlphaFlagsKHR, Extent2D, Format, Image, ImageAspectFlags,
        ImageSubresourceRange, ImageUsageFlags, ImageViewCreateInfo, ImageViewType, PresentInfoKHR,
        PresentModeKHR, SharingMode, SurfaceFormatKHR, SwapchainCreateInfoKHR, SwapchainKHR,
    },
};

use super::{Device, ImageView, Semaphore, Surface};

pub struct Swapchain {
    image_format: Format,
    image_extent: Extent2D,
    handle: SwapchainKHR,
    inner: khr::Swapchain,
    device: Arc<Device>,
}

impl Swapchain {
    pub(super) fn new(
        device: Arc<Device>,
        configuration: SwapchainConfiguration<'_>,
    ) -> VkResult<Self> {
        let image_format = configuration.surface_format.format;
        let (inner, handle) = if let Some(swapchain) = configuration.previous {
            (swapchain.inner.clone(), swapchain.handle)
        } else {
            (
                khr::Swapchain::new(&device.instance.inner, &device.inner),
                SwapchainKHR::null(),
            )
        };
        let capabilities = configuration
            .surface
            .capabilities(device.physical_device_handle())?;
        let image_extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            Extent2D {
                width: configuration.extent.width.clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                ),
                height: configuration.extent.height.clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                ),
            }
        };
        let min_image_count = if capabilities.max_image_count != 0
            && capabilities.min_image_count + 1 > capabilities.max_image_count
        {
            capabilities.max_image_count
        } else {
            capabilities.min_image_count + 1
        };
        let create_info = SwapchainCreateInfoKHR::builder()
            .surface(configuration.surface.handle)
            .min_image_count(min_image_count)
            .image_format(image_format)
            .image_color_space(configuration.surface_format.color_space)
            .image_extent(image_extent)
            .image_usage(ImageUsageFlags::COLOR_ATTACHMENT)
            .image_array_layers(1)
            .image_sharing_mode(SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(configuration.present_mode)
            .clipped(true)
            .old_swapchain(handle);
        let handle = unsafe { inner.create_swapchain(&create_info, None) }?;
        Ok(Self {
            image_format,
            image_extent,
            handle,
            inner,
            device,
        })
    }

    pub fn acquire_next_image(&self, signal: &Semaphore) -> VkResult<(u32, bool)> {
        unsafe {
            self.inner
                .acquire_next_image(self.handle, u64::MAX, signal.handle, vk::Fence::null())
        }
    }

    pub fn present(&self, image_index: u32, wait: &Semaphore) -> VkResult<bool> {
        let wait_semaphores = [wait.handle];
        let image_indices = [image_index];
        let swapchains = [self.handle];
        let create_info = PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphores)
            .image_indices(&image_indices)
            .swapchains(&swapchains);
        unsafe {
            self.inner
                .queue_present(self.device.main_queue.handle, &create_info)
        }
    }

    pub fn images(&self) -> VkResult<Vec<SwapchainImage>> {
        unsafe { self.inner.get_swapchain_images(self.handle) }?
            .into_iter()
            .map(|handle| SwapchainImage::new(self, handle))
            .collect()
    }

    pub fn image_format(&self) -> Format {
        self.image_format
    }

    pub fn image_extent(&self) -> Extent2D {
        self.image_extent
    }
}

impl Deref for Swapchain {
    type Target = SwapchainKHR;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe { self.inner.destroy_swapchain(self.handle, None) };
    }
}

impl Swapchain {
    pub fn extension_name() -> &'static CStr {
        khr::Swapchain::name()
    }
}

pub struct SwapchainImage {
    view: ImageView,
    handle: Image,
}

impl SwapchainImage {
    pub fn new(swapchain: &Swapchain, handle: Image) -> VkResult<Self> {
        let create_info = ImageViewCreateInfo::builder()
            .image(handle)
            .view_type(ImageViewType::TYPE_2D)
            .format(swapchain.image_format)
            .subresource_range(
                ImageSubresourceRange::builder()
                    .aspect_mask(ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            );
        let view = ImageView {
            handle: unsafe {
                swapchain
                    .device
                    .inner
                    .create_image_view(&create_info, None)?
            },
            device: swapchain.device.clone(),
        };
        Ok(Self { view, handle })
    }

    pub fn view(&self) -> &ImageView {
        &self.view
    }
}

impl Deref for SwapchainImage {
    type Target = Image;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

pub struct SwapchainConfiguration<'a> {
    pub surface: &'a Surface,
    pub previous: Option<&'a Swapchain>,
    pub present_mode: PresentModeKHR,
    pub surface_format: SurfaceFormatKHR,
    pub extent: Extent2D,
}
