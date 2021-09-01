use std::sync::Arc;

use ash::{
    extensions::khr,
    prelude::VkResult,
    vk::{PhysicalDevice, PresentModeKHR, SurfaceCapabilitiesKHR, SurfaceFormatKHR, SurfaceKHR},
};

use super::Instance;

pub struct Surface {
    pub(super) inner: khr::Surface,
    pub(super) handle: SurfaceKHR,
    _instance: Arc<Instance>,
}

impl Surface {
    pub(super) fn new(instance: Arc<Instance>, handle: SurfaceKHR) -> Self {
        let inner = khr::Surface::new(&instance.entry, &instance.inner);
        Self {
            inner,
            handle,
            _instance: instance,
        }
    }

    pub fn formats(&self, physical_device: PhysicalDevice) -> VkResult<Vec<SurfaceFormatKHR>> {
        unsafe {
            self.inner
                .get_physical_device_surface_formats(physical_device, self.handle)
        }
    }

    pub fn present_modes(&self, physical_device: PhysicalDevice) -> VkResult<Vec<PresentModeKHR>> {
        unsafe {
            self.inner
                .get_physical_device_surface_present_modes(physical_device, self.handle)
        }
    }

    pub fn capabilities(
        &self,
        physical_device: PhysicalDevice,
    ) -> VkResult<SurfaceCapabilitiesKHR> {
        unsafe {
            self.inner
                .get_physical_device_surface_capabilities(physical_device, self.handle)
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.inner.destroy_surface(self.handle, None) }
    }
}
