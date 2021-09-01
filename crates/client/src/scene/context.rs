use std::sync::Arc;

use client_gpu::{Mesh, Model};
use eyre::Context;
use vulkan::{
    BufferUsageFlags, DescriptorSetLayout, DescriptorSetLayoutBinding,
    DescriptorSetLayoutCreateFlags, DescriptorType, Device, GpuVec, MemoryLocation, PipelineLayout,
    PipelineLayoutCreateFlags, ShaderStageFlags, VkResult,
};

use crate::root::RootContext;

use super::{Camera, MeshData, Vertex};

pub struct SceneContext {
    pub vertices: GpuVec<Vertex>,
    pub vertex_indices: GpuVec<u32>,
    pub models: GpuVec<Model>,
    pub camera: Camera,
    pub pipeline_layout: PipelineLayout,
    pub set_layout: DescriptorSetLayout,
}

impl SceneContext {
    pub fn new(
        root_context: &RootContext,
        vertex_capacity: usize,
        index_capacity: usize,
        model_capacity: usize,
        camera: Camera,
    ) -> eyre::Result<Self> {
        let device = &root_context.device;
        let set_layout = device.create_descriptor_set_layout(
            DescriptorSetLayoutCreateFlags::empty(),
            &[
                DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_count(1)
                    .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                    .stage_flags(ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT)
                    .build(),
                DescriptorSetLayoutBinding::builder()
                    .binding(1)
                    .descriptor_count(1)
                    .descriptor_type(DescriptorType::STORAGE_BUFFER)
                    .stage_flags(ShaderStageFlags::VERTEX)
                    .build(),
            ],
        )?;
        let pipeline_layout = device.create_pipeline_layout(
            PipelineLayoutCreateFlags::empty(),
            &[&set_layout],
            &[],
        )?;
        Ok(Self {
            set_layout,
            pipeline_layout,
            vertices: device
                .create_vec(
                    vertex_capacity,
                    BufferUsageFlags::VERTEX_BUFFER,
                    MemoryLocation::CpuToGpu,
                )
                .wrap_err("could not create vertex gpu vector")?,
            vertex_indices: device
                .create_vec(
                    index_capacity,
                    BufferUsageFlags::INDEX_BUFFER,
                    MemoryLocation::CpuToGpu,
                )
                .wrap_err("could not create index gpu vector")?,
            models: device
                .create_vec(
                    model_capacity,
                    BufferUsageFlags::STORAGE_BUFFER,
                    MemoryLocation::CpuToGpu,
                )
                .wrap_err("could not create model gpu vector")?,
            camera,
        })
    }

    pub fn insert_mesh(&mut self, mesh: MeshData) -> Mesh {
        let vertex_offset = self.vertices.len() as i32;
        let first_index = self.vertex_indices.len() as u32;
        let index_count = mesh.indices.len() as u32;
        self.vertices.append(&mesh.vertices);
        self.vertex_indices.append(&mesh.indices);
        Mesh {
            first_index,
            index_count,
            vertex_offset,
        }
    }

    pub fn insert_model(&mut self, model: Model) -> u32 {
        let model_index = self.models.len() as u32;
        self.models.push(model);
        model_index
    }

    pub fn flush(&self, device: &Arc<Device>) -> VkResult<()> {
        device.flush_mapped_memory_ranges(&[
            self.vertices.range(),
            self.vertex_indices.range(),
            self.models.range(),
        ])
    }
}
