use std::{cmp::Ordering, sync::Arc};

use ash::{
    prelude::VkResult,
    vk::{
        self, ExtensionProperties, Format, FormatProperties, KhrPortabilitySubsetFn,
        KhrShaderFloat16Int8Fn, KhrTimelineSemaphoreFn, PhysicalDeviceFeatures2,
        PhysicalDeviceFloat16Int8FeaturesKHR, PhysicalDevicePortabilitySubsetFeaturesKHR,
        PhysicalDevicePortabilitySubsetPropertiesKHR, PhysicalDeviceProperties2,
        PhysicalDeviceShaderDrawParametersFeatures, PhysicalDeviceTimelineSemaphoreFeatures,
        PhysicalDeviceType, PhysicalDeviceVulkan11Features, PhysicalDeviceVulkan12Features,
        QueueFamilyProperties,
    },
};
use semver::{Comparator, Op, Prerelease, Version};
use vk::{DeviceCreateInfo, DeviceQueueCreateInfo};

use crate::{chain::ChainBuilder, contains_extension, ApiLevel, VersionExt};

use super::{Device, DeviceConfiguration, Error, Instance, Surface};

#[derive(Clone)]
pub struct PhysicalDevice {
    pub(super) instance: Arc<Instance>,
    pub(super) handle: vk::PhysicalDevice,
    pub(super) properties: PhysicalDeviceProperties,
    pub(super) features: PhysicalDeviceFeatures,
    pub(super) api_level: ApiLevel,
    pub(super) extensions: Vec<ExtensionProperties>,
    pub(super) queue_families: Vec<QueueFamilyProperties>,
}

impl PhysicalDevice {
    pub(super) fn new(
        instance: Arc<Instance>,
        handle: vk::PhysicalDevice,
    ) -> VkResult<Option<Self>> {
        let extensions = unsafe { instance.inner.enumerate_device_extension_properties(handle) }?;
        let queue_families = unsafe {
            instance
                .inner
                .get_physical_device_queue_family_properties(handle)
        };
        let mut properties = PhysicalDeviceProperties::default();
        properties.vulkan_10.properties =
            unsafe { instance.inner.get_physical_device_properties(handle) };
        let api_version = Version::from_u32(properties.vulkan_10.properties.api_version);
        let api_level = if (Comparator {
            op: Op::GreaterEq,
            major: 2,
            minor: None,
            patch: None,
            pre: Prerelease::EMPTY,
        })
        .matches(&api_version)
        {
            return Ok(None);
        } else if (Comparator {
            op: Op::GreaterEq,
            major: 1,
            minor: Some(2),
            patch: None,
            pre: Prerelease::EMPTY,
        })
        .matches(&api_version)
        {
            ApiLevel::Vulkan12
        } else if (Comparator {
            op: Op::GreaterEq,
            major: 1,
            minor: Some(1),
            patch: None,
            pre: Prerelease::EMPTY,
        })
        .matches(&api_version)
        {
            ApiLevel::Vulkan11
        } else {
            return Ok(None);
        };
        unsafe {
            instance
                .inner
                .get_physical_device_properties2(handle, properties.chain())
        };
        let mut features = PhysicalDeviceFeatures::default();
        unsafe {
            instance
                .inner
                .get_physical_device_features2(handle, features.chain(api_level, &extensions))
        };
        Ok(Some(Self {
            instance,
            handle,
            extensions,
            queue_families,
            properties,
            features,
            api_level,
        }))
    }

    pub fn features(&self) -> &PhysicalDeviceFeatures {
        &self.features
    }

    pub fn extensions(&self) -> &[ExtensionProperties] {
        &self.extensions
    }

    pub fn queue_families(&self) -> &[QueueFamilyProperties] {
        &self.queue_families
    }

    pub fn handle(&self) -> vk::PhysicalDevice {
        self.handle
    }

    pub fn surface_support(&self, surface: &Surface, queue_family_index: u32) -> VkResult<bool> {
        unsafe {
            surface.inner.get_physical_device_surface_support(
                self.handle,
                queue_family_index,
                surface.handle,
            )
        }
    }

    pub fn properties(&self) -> &PhysicalDeviceProperties {
        &self.properties
    }

    pub fn api_level(&self) -> ApiLevel {
        self.api_level
    }

    pub fn format_properties(&self, format: Format) -> FormatProperties {
        unsafe {
            self.instance
                .inner
                .get_physical_device_format_properties(self.handle, format)
        }
    }

    pub fn create(self, mut configuration: DeviceConfiguration) -> Result<Device, Error> {
        let queue_priorities = [1.0];
        let mut queue_create_infos = vec![DeviceQueueCreateInfo::builder()
            .queue_family_index(configuration.main_queue_family_index)
            .queue_priorities(&queue_priorities)
            .build()];
        if let Some(transfer_family_index) = configuration.transfer_queue_family_index {
            queue_create_infos.push(
                DeviceQueueCreateInfo::builder()
                    .queue_family_index(transfer_family_index)
                    .queue_priorities(&queue_priorities)
                    .build(),
            )
        }
        let extension_names_ptr: Vec<_> = configuration
            .extension_names
            .iter()
            .map(|c| c.as_ptr())
            .collect();
        let create_info = DeviceCreateInfo::builder()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&extension_names_ptr)
            .push_next(
                configuration
                    .features
                    .chain(self.api_level, &self.extensions),
            );
        let device = unsafe {
            self.instance
                .inner
                .create_device(self.handle, &create_info, None)?
        };
        Ok(Device::new(self, device, configuration))
    }
}

#[derive(Clone, Debug, Default)]
pub struct PhysicalDeviceFeatures {
    pub vulkan_10: PhysicalDeviceFeatures2,
    pub portability_subset: PhysicalDevicePortabilitySubsetFeaturesKHR,
    pub vulkan_11: PhysicalDeviceVulkan11Features,
    pub shader_draw_parameters: PhysicalDeviceShaderDrawParametersFeatures,
    pub vulkan_12: PhysicalDeviceVulkan12Features,
    pub timeline_semaphore: PhysicalDeviceTimelineSemaphoreFeatures,
    pub float16_int8: PhysicalDeviceFloat16Int8FeaturesKHR,
}

impl PhysicalDeviceFeatures {
    fn chain(
        &mut self,
        api_level: ApiLevel,
        extensions: &[ExtensionProperties],
    ) -> &mut PhysicalDeviceFeatures2 {
        let mut chain_builder = ChainBuilder::new(&mut self.vulkan_10);
        if contains_extension(extensions, KhrPortabilitySubsetFn::name()) {
            chain_builder.push(&mut self.portability_subset)
        }
        if api_level >= ApiLevel::Vulkan12 {
            chain_builder.push(&mut self.vulkan_11);
            chain_builder.push(&mut self.vulkan_12);
        } else {
            if contains_extension(extensions, vk::KhrShaderDrawParametersFn::name()) {
                chain_builder.push(&mut self.shader_draw_parameters)
            }
            if contains_extension(extensions, KhrTimelineSemaphoreFn::name()) {
                chain_builder.push(&mut self.timeline_semaphore)
            }
            if contains_extension(extensions, KhrShaderFloat16Int8Fn::name()) {
                chain_builder.push(&mut self.float16_int8)
            }
        }
        &mut self.vulkan_10
    }
}

#[derive(Default, Clone, Debug)]
pub struct PhysicalDeviceProperties {
    pub vulkan_10: PhysicalDeviceProperties2,
    pub portability_subset: PhysicalDevicePortabilitySubsetPropertiesKHR,
}

impl PhysicalDeviceProperties {
    fn chain(&mut self) -> &mut PhysicalDeviceProperties2 {
        self.vulkan_10.p_next = &mut self.portability_subset
            as *mut PhysicalDevicePortabilitySubsetPropertiesKHR
            as *mut _;
        &mut self.vulkan_10
    }
}

pub fn cmp_device_types(a: PhysicalDeviceType, b: PhysicalDeviceType) -> Ordering {
    device_type_priority(a).cmp(&device_type_priority(b))
}

fn device_type_priority(device_type: PhysicalDeviceType) -> u32 {
    if device_type == PhysicalDeviceType::DISCRETE_GPU {
        5
    } else if device_type == PhysicalDeviceType::INTEGRATED_GPU {
        4
    } else if device_type == PhysicalDeviceType::CPU {
        3
    } else if device_type == PhysicalDeviceType::VIRTUAL_GPU {
        2
    } else if device_type == PhysicalDeviceType::OTHER {
        1
    } else {
        0
    }
}
