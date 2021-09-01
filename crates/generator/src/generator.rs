use std::{ffi::CString, io, mem::size_of, sync::Arc};

use database::Database;
use generator_spirv::CODE;
use glam::{vec3, UVec4};
use persistent::{Configuration, World};
use protocol::Rotation;
use thiserror::Error;
use tokio::{spawn, sync::mpsc::Sender, task::JoinHandle};
use util::align::align_bytes;
use vulkan::{
    BufferCopy, BufferCreateInfo, BufferUsageFlags, CommandBufferUsageFlags,
    CommandPoolCreateFlags, ComputePipelineCreateInfo, DescriptorBufferInfo, DescriptorPoolSetup,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags, DescriptorType, Device,
    FenceCreateFlags, MemoryLocation, PipelineBindPoint, PipelineCacheCreateFlags,
    PipelineLayoutCreateFlags, PipelineShaderStageCreateInfo, ShaderModuleCreateFlags,
    ShaderStageFlags, SharingMode, SubmitInfo, WriteDescriptorSet,
};

use crate::{CancelError, Control, ControlBarrier, Notification, Template};

pub struct Generator {
    pub control: Control,
    task: Option<JoinHandle<Result<(), GenerateError>>>,
}

impl Generator {
    pub fn new(template: Template, notifier: Sender<Notification>, device: Arc<Device>) -> Self {
        let (control, barrier) = Control::new();
        let task = Some(spawn(generate(template, notifier, barrier, device)));
        Self { control, task }
    }

    pub async fn join(&mut self) -> Result<(), GenerateError> {
        if let Some(task) = self.task.take() {
            task.await.unwrap()
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error(transparent)]
    Cancelled(#[from] CancelError),
    #[error("snapshot failed")]
    Snapshot(#[source] io::Error),
    #[error("create database failed")]
    Create(#[source] io::Error),
    #[error(transparent)]
    VulkanApi(#[from] vulkan::ApiResult),
    #[error(transparent)]
    Vulkan(#[from] vulkan::Error),
}

async fn generate(
    _template: Template,
    _notifier: Sender<Notification>,
    mut barrier: ControlBarrier,
    device: Arc<Device>,
) -> Result<(), GenerateError> {
    let (mut db, mut world) = Database::create("world.db", |db| {
        let configuration = Configuration {
            region_size: 255,
            size: 32,
            full_distance: 3,
            static_distance: 15,
        };
        World::new(db, configuration)
    })
    .map_err(GenerateError::Create)?;
    let shader_module =
        device.create_shader_module(ShaderModuleCreateFlags::empty(), &align_bytes(CODE))?;
    let pipeline_cache = device.create_pipeline_cache(PipelineCacheCreateFlags::empty(), None)?;

    let set_layout = {
        let bindings = [
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .stage_flags(ShaderStageFlags::COMPUTE)
                .build(),
        ];
        device.create_descriptor_set_layout(DescriptorSetLayoutCreateFlags::empty(), &bindings)?
    };
    let pipeline_layout =
        device.create_pipeline_layout(PipelineLayoutCreateFlags::empty(), &[&set_layout], &[])?;
    let pipeline = {
        let main_name = CString::new("height_map").unwrap();
        let stage = PipelineShaderStageCreateInfo::builder()
            .stage(ShaderStageFlags::COMPUTE)
            .module(*shader_module)
            .name(&main_name)
            .build();
        let create_infos = [ComputePipelineCreateInfo::builder()
            .stage(stage)
            .layout(*pipeline_layout)
            .build()];
        let mut pipelines = pipeline_cache.create_compute(&create_infos)?;
        pipelines.remove(0)
    };
    let command_pool = device.create_command_pool(
        CommandPoolCreateFlags::TRANSIENT,
        device.main_queue_family_index(),
    )?;
    let command_buffer = command_pool.allocate_single_primary()?;
    let size = world.configuration.full_size();
    let input = device.create_variable(
        BufferUsageFlags::UNIFORM_BUFFER,
        MemoryLocation::CpuToGpu,
        UVec4::new(0, 0, 0, size as u32),
    )?;
    device.flush_mapped_memory_ranges(&[input.range()])?;
    let output = {
        let create_info = BufferCreateInfo::builder()
            .sharing_mode(SharingMode::EXCLUSIVE)
            .size((size * size) as u64)
            .usage(BufferUsageFlags::STORAGE_BUFFER | BufferUsageFlags::TRANSFER_SRC);
        device.create_buffer(&create_info, MemoryLocation::GpuOnly)
    }?;
    let mut output_readback = device.create_vec::<u8>(
        size * size,
        BufferUsageFlags::TRANSFER_DST,
        MemoryLocation::GpuToCpu,
    )?;
    let descriptor_pool = DescriptorPoolSetup {
        storage_buffers: 1,
        uniform_buffers: 1,
        sets: 1,
        ..Default::default()
    }
    .create_pool(&device)?;
    let mut descriptor_sets = descriptor_pool.allocate(&[&set_layout])?;
    let descriptor_set = descriptor_sets.pop().unwrap();
    {
        let uniform_buffer_info = [DescriptorBufferInfo::builder()
            .buffer(**input.buffer())
            .offset(0)
            .range(size_of::<UVec4>() as u64)
            .build()];
        let storage_buffer_info = [DescriptorBufferInfo::builder()
            .buffer(*output)
            .offset(0)
            .range((size * size) as u64)
            .build()];
        let descriptor_writes = [
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&uniform_buffer_info)
                .build(),
            WriteDescriptorSet::builder()
                .dst_set(*descriptor_set)
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(DescriptorType::STORAGE_BUFFER)
                .buffer_info(&storage_buffer_info)
                .build(),
        ];
        device.update_descriptor_sets(&descriptor_writes, &[]);
    }
    command_buffer.begin(CommandBufferUsageFlags::ONE_TIME_SUBMIT, None)?;
    command_buffer.bind_pipeline(PipelineBindPoint::COMPUTE, &pipeline);
    command_buffer.bind_descriptor_sets(
        PipelineBindPoint::COMPUTE,
        &pipeline_layout,
        0,
        &[&descriptor_set],
        &[],
    );
    command_buffer.dispatch((size as u32 + 15) / 16, 1, (size as u32 + 15) / 16);
    command_buffer.copy_buffer(
        &output,
        output_readback.buffer(),
        &[BufferCopy::builder()
            .src_offset(0)
            .dst_offset(0)
            .size((size * size) as u64)
            .build()],
    );
    command_buffer.end()?;
    let fence = device.create_fence(FenceCreateFlags::empty())?;
    {
        let command_buffers = [*command_buffer];
        let signal_semaphores = [];
        let wait_semaphores = [];
        let wait_dst_stage_mask = [];
        let submits = [SubmitInfo::builder()
            .command_buffers(&command_buffers)
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_dst_stage_mask)
            .signal_semaphores(&signal_semaphores)
            .build()];
        device.submit(&submits, &fence)?;
    }
    fence.wait()?;
    device.invalidate_mapped_memory_ranges(&[output_readback.range()])?;
    unsafe { output_readback.set_len(size * size) };
    {
        let mut heights = world.heights.write();
        heights.append(output_readback.as_slice());
    }
    for x in 0..500 {
        for z in 0..500 {
            world.spawn_npc(
                vec3(100.0 + (x * 15) as f32, 400.0, 100.0 + (z * 15) as f32),
                Rotation {
                    roll: 0.0,
                    pitch: 0.0,
                    yaw: 0.0,
                },
            );
        }
    }
    db.snapshot(&mut world).map_err(GenerateError::Snapshot)?;
    barrier.wait().await?;
    Ok(())
}
