use std::cmp::Ordering;

use vulkan::{
    cmp_device_types, contains_extension, Device, DeviceConfiguration, KhrPortabilitySubsetFn,
    PhysicalDevice, PhysicalDeviceFeatures, QueueFamilyProperties, QueueFlags, VkResult,
};

pub struct DeviceCandidate {
    physical_device: PhysicalDevice,
    device_configuration: DeviceConfiguration,
}

impl DeviceCandidate {
    pub fn new(physical_device: PhysicalDevice) -> VkResult<Option<Self>> {
        let mut extension_names = Vec::new();
        if contains_extension(physical_device.extensions(), KhrPortabilitySubsetFn::name()) {
            extension_names.push(KhrPortabilitySubsetFn::name());
        }
        let families = physical_device.queue_families();
        let main_queue_family_index = match families
            .iter()
            .enumerate()
            .map(|(index, properties)| (index as u32, properties))
            .filter(|(_, properties)| properties.queue_flags.contains(QueueFlags::COMPUTE))
            .max_by_key(|(_, properties)| queue_family_priority(*properties))
            .map(|(index, _)| index as u32)
        {
            Some(index) => index,
            None => return Ok(None),
        };
        let transfer_queue_family_index = families
            .iter()
            .enumerate()
            .map(|(index, properties)| (index as u32, properties))
            .find(|(_, properties)| {
                properties.queue_flags.contains(QueueFlags::TRANSFER)
                    && !properties.queue_flags.contains(QueueFlags::GRAPHICS)
                    && !properties.queue_flags.contains(QueueFlags::COMPUTE)
            })
            .map(|(index, _)| index as u32);
        let device_configuration = DeviceConfiguration {
            extension_names,
            features: PhysicalDeviceFeatures::default(),
            main_queue_family_index,
            transfer_queue_family_index,
        };
        Ok(Some(Self {
            physical_device,
            device_configuration,
        }))
    }

    pub fn create(self) -> Result<Device, vulkan::Error> {
        self.physical_device.create(self.device_configuration)
    }
}

impl PartialEq for DeviceCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for DeviceCandidate {}

impl PartialOrd for DeviceCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeviceCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_device_types(
            self.physical_device
                .properties()
                .vulkan_10
                .properties
                .device_type,
            other
                .physical_device
                .properties()
                .vulkan_10
                .properties
                .device_type,
        )
    }
}

fn queue_family_priority(properties: &QueueFamilyProperties) -> u32 {
    if properties.queue_flags.contains(QueueFlags::GRAPHICS) {
        0
    } else {
        1
    }
}
