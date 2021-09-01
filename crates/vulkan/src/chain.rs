use std::ffi::c_void;

use ash::vk::{
    PhysicalDeviceFeatures2, PhysicalDeviceFloat16Int8FeaturesKHR,
    PhysicalDevicePortabilitySubsetFeaturesKHR, PhysicalDeviceShaderDrawParametersFeatures,
    PhysicalDeviceTimelineSemaphoreFeaturesKHR, PhysicalDeviceVulkan11Features,
    PhysicalDeviceVulkan12Features,
};

pub trait Chainable {
    fn set_next(&mut self, next: *mut c_void);
}

impl Chainable for PhysicalDeviceFeatures2 {
    fn set_next(&mut self, next: *mut c_void) {
        self.p_next = next;
    }
}

impl Chainable for PhysicalDeviceVulkan11Features {
    fn set_next(&mut self, next: *mut c_void) {
        self.p_next = next;
    }
}

impl Chainable for PhysicalDeviceVulkan12Features {
    fn set_next(&mut self, next: *mut c_void) {
        self.p_next = next;
    }
}

impl Chainable for PhysicalDevicePortabilitySubsetFeaturesKHR {
    fn set_next(&mut self, next: *mut c_void) {
        self.p_next = next;
    }
}

impl Chainable for PhysicalDeviceShaderDrawParametersFeatures {
    fn set_next(&mut self, next: *mut c_void) {
        self.p_next = next;
    }
}

impl Chainable for PhysicalDeviceTimelineSemaphoreFeaturesKHR {
    fn set_next(&mut self, next: *mut c_void) {
        self.p_next = next;
    }
}

impl Chainable for PhysicalDeviceFloat16Int8FeaturesKHR {
    fn set_next(&mut self, next: *mut c_void) {
        self.p_next = next;
    }
}

pub struct ChainBuilder<'a>(&'a mut dyn Chainable);

impl<'a> ChainBuilder<'a> {
    pub fn new<T: Chainable>(start: &'a mut T) -> Self {
        Self(start)
    }
    pub fn push<T: Chainable>(&mut self, next: &'a mut T) {
        self.0.set_next(next as *mut T as _);
        self.0 = next;
    }
}
