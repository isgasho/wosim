use std::{
    ops::{Add, Mul},
    sync::Arc,
};

use ash::{
    prelude::VkResult,
    vk::{DescriptorPoolCreateFlags, DescriptorPoolSize, DescriptorType},
};

use super::{Device, Object};

use ash::vk::{self, DescriptorSetAllocateInfo};

pub type DescriptorSetLayout = Object<vk::DescriptorSetLayout>;
pub type DescriptorSet = Object<vk::DescriptorSet>;
pub type DescriptorPool = Object<vk::DescriptorPool>;

impl DescriptorPool {
    pub fn allocate(&self, set_layouts: &[&DescriptorSetLayout]) -> VkResult<Vec<DescriptorSet>> {
        let set_layouts: Vec<_> = set_layouts
            .iter()
            .map(|set_layout| set_layout.handle)
            .collect();
        let create_info = DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.handle)
            .set_layouts(&set_layouts);
        Ok(
            unsafe { self.device.inner.allocate_descriptor_sets(&create_info) }?
                .into_iter()
                .map(|handle| DescriptorSet {
                    handle,
                    device: self.device.clone(),
                })
                .collect(),
        )
    }
}

#[derive(Default)]
pub struct DescriptorPoolSetup {
    pub samplers: u32,
    pub combined_image_samplers: u32,
    pub sampled_images: u32,
    pub storage_images: u32,
    pub uniform_buffers: u32,
    pub storage_buffers: u32,
    pub sets: u32,
}

impl DescriptorPoolSetup {
    pub fn create_pool(&self, device: &Arc<Device>) -> VkResult<DescriptorPool> {
        let mut pool_sizes = Vec::new();
        if self.samplers > 0 {
            pool_sizes.push(
                DescriptorPoolSize::builder()
                    .ty(DescriptorType::SAMPLER)
                    .descriptor_count(self.samplers)
                    .build(),
            );
        }
        if self.combined_image_samplers > 0 {
            pool_sizes.push(
                DescriptorPoolSize::builder()
                    .ty(DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .descriptor_count(self.combined_image_samplers)
                    .build(),
            );
        }
        if self.sampled_images > 0 {
            pool_sizes.push(
                DescriptorPoolSize::builder()
                    .ty(DescriptorType::SAMPLED_IMAGE)
                    .descriptor_count(self.sampled_images)
                    .build(),
            );
        }
        if self.storage_images > 0 {
            pool_sizes.push(
                DescriptorPoolSize::builder()
                    .ty(DescriptorType::STORAGE_IMAGE)
                    .descriptor_count(self.storage_images)
                    .build(),
            );
        }
        if self.uniform_buffers > 0 {
            pool_sizes.push(
                DescriptorPoolSize::builder()
                    .ty(DescriptorType::UNIFORM_BUFFER)
                    .descriptor_count(self.uniform_buffers)
                    .build(),
            );
        }
        if self.storage_buffers > 0 {
            pool_sizes.push(
                DescriptorPoolSize::builder()
                    .ty(DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(self.storage_buffers)
                    .build(),
            );
        }
        device.create_descriptor_pool(DescriptorPoolCreateFlags::empty(), &pool_sizes, self.sets)
    }
}

impl Add for DescriptorPoolSetup {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            storage_buffers: self.storage_buffers + rhs.storage_buffers,
            combined_image_samplers: self.combined_image_samplers + rhs.combined_image_samplers,
            storage_images: self.storage_images + rhs.storage_images,
            uniform_buffers: self.uniform_buffers + rhs.uniform_buffers,
            sampled_images: self.sampled_images + rhs.sampled_images,
            samplers: self.samplers + rhs.samplers,
            sets: self.sets + rhs.sets,
        }
    }
}

impl Mul<u32> for DescriptorPoolSetup {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self {
            storage_buffers: self.storage_buffers * rhs,
            combined_image_samplers: self.combined_image_samplers * rhs,
            storage_images: self.storage_images * rhs,
            uniform_buffers: self.uniform_buffers * rhs,
            sampled_images: self.sampled_images * rhs,
            samplers: self.samplers * rhs,
            sets: self.sets * rhs,
        }
    }
}
