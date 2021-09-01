use std::{ops::Deref, sync::Arc};

use ash::vk::{self, Extent2D, ImageCreateInfo, ImageTiling};
use gpu_allocator::{
    vulkan::{Allocation, AllocationCreateDesc},
    MemoryLocation,
};

use crate::Error;

use super::Device;

pub struct Image {
    pub(super) handle: vk::Image,
    allocation: Option<Allocation>,
    device: Arc<Device>,
}

impl Image {
    pub fn new(
        device: Arc<Device>,
        create_info: &ImageCreateInfo,
        location: MemoryLocation,
    ) -> Result<Self, Error> {
        let handle = unsafe { device.inner.create_image(create_info, None) }?;
        let requirements = unsafe { device.inner.get_image_memory_requirements(handle) };
        let allocation = device
            .allocator
            .lock()
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: "",
                requirements,
                location,
                linear: create_info.tiling == ImageTiling::LINEAR,
            })?;
        unsafe {
            device
                .inner
                .bind_image_memory(handle, allocation.memory(), allocation.offset())
        }?;
        Ok(Self {
            handle,
            allocation: Some(allocation),
            device,
        })
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        self.device
            .allocator
            .lock()
            .unwrap()
            .free(self.allocation.take().unwrap())
            .unwrap();
        unsafe { self.device.inner.destroy_image(self.handle, None) };
    }
}

impl Deref for Image {
    type Target = vk::Image;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

pub fn mip_levels_for_extent(extent: Extent2D) -> u32 {
    32 - extent.width.max(extent.height).leading_zeros()
}
