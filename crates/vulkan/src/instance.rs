use std::{
    ffi::{CStr, CString},
    sync::Arc,
};

use ash::{
    prelude::VkResult,
    vk::{make_api_version, ApplicationInfo, InstanceCreateInfo, SurfaceKHR},
    Entry,
};
use semver::Version;

use super::{Error, PhysicalDevice, Surface, VersionExt};

pub struct Instance {
    pub(super) inner: ash::Instance,
    pub(super) entry: Entry,
}

impl Instance {
    pub fn new(
        application_name: &CStr,
        application_version: Version,
        extension_names: Vec<&CStr>,
    ) -> Result<Self, Error> {
        let entry = unsafe { Entry::new() }?;
        let layer_names = if cfg!(debug_assertions) {
            vec![CString::new("VK_LAYER_KHRONOS_validation").unwrap()]
        } else {
            vec![]
        };
        let application_info = ApplicationInfo::builder()
            .api_version(make_api_version(0, 1, 2, 0))
            .application_name(application_name)
            .application_version(application_version.to_u32());
        let extension_names_ptr: Vec<_> = extension_names.iter().map(|c| c.as_ptr()).collect();
        let layer_names_ptr: Vec<_> = layer_names.iter().map(|c| c.as_ptr()).collect();
        let create_info = InstanceCreateInfo::builder()
            .enabled_layer_names(&layer_names_ptr)
            .enabled_extension_names(&extension_names_ptr)
            .application_info(&application_info)
            .build();
        let inner = unsafe { entry.create_instance(&create_info, None) }?;
        Ok(Self { inner, entry })
    }

    pub fn create_surface<F: Fn(&Entry, &ash::Instance) -> VkResult<SurfaceKHR>>(
        self: &Arc<Self>,
        create_handle: F,
    ) -> VkResult<Surface> {
        let handle = create_handle(&self.entry, &self.inner)?;
        Ok(Surface::new(self.clone(), handle))
    }

    pub fn physical_devices(self: &Arc<Self>) -> VkResult<Vec<PhysicalDevice>> {
        Ok(unsafe { self.inner.enumerate_physical_devices() }?
            .into_iter()
            .filter_map(|handle| PhysicalDevice::new(self.clone(), handle).ok().flatten())
            .collect())
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { self.inner.destroy_instance(None) }
    }
}
