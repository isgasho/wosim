use std::{ffi::CString, sync::Arc};

use ash_window::{create_surface, enumerate_required_extensions};
use client_spirv::CODE;
use eyre::{eyre, Context};
use semver::Version;
use util::{align::align_bytes, iterator::MaxOkFilterMap};
use vulkan::{
    AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp,
    DescriptorPool, DescriptorPoolSetup, Device, ImageLayout, Instance, PipelineBindPoint,
    PipelineCache, PipelineCacheCreateFlags, PipelineStageFlags, RenderPass, RenderPassCreateInfo,
    SampleCountFlags, Semaphore, ShaderModule, ShaderModuleCreateFlags, SubpassDependency,
    SubpassDescription, Surface, Swapchain, VkResult, SUBPASS_EXTERNAL,
};
use winit::{
    event_loop::{EventLoop, EventLoopProxy},
    window::{Window, WindowBuilder},
};

use crate::{
    action::Action,
    debug::{DebugContext, DebugWindows},
    egui::Egui,
    renderer::RenderConfiguration,
    subscriber::{init_subscriber, FilterHandle},
    vulkan::{create_swapchain, DeviceCandidate},
};

pub struct RootContext {
    pub frame_count: usize,
    pub semaphore: Semaphore,
    pub pipeline_cache: PipelineCache,
    pub render_configuration: RenderConfiguration,
    pub egui: Egui,
    pub debug: DebugContext,
    pub descriptor_pool: DescriptorPool,
    pub swapchain: Arc<Swapchain>,
    pub shader_module: ShaderModule,
    pub device: Arc<Device>,
    pub surface: Surface,
    pub window: Window,
    pub grab: bool,
    pub vsync: bool,
    pub windows: DebugWindows,
    pub proxy: EventLoopProxy<Action>,
    pub filter_handle: FilterHandle,
}

impl RootContext {
    pub fn new(event_loop: &EventLoop<Action>) -> eyre::Result<Self> {
        let filter_handle = init_subscriber(event_loop.create_proxy())?;
        let window = WindowBuilder::new()
            .with_title(format!("WoSim v{}", env!("CARGO_PKG_VERSION")))
            .build(event_loop)?;
        let proxy = event_loop.create_proxy();
        let version = Version::parse(env!("CARGO_PKG_VERSION"))?;
        let instance = Arc::new(
            Instance::new(
                &CString::new("wosim").unwrap(),
                version,
                enumerate_required_extensions(&window)?,
            )
            .wrap_err("could not create instance")?,
        );
        let surface = instance.create_surface(|entry, instance| unsafe {
            create_surface(entry, instance, &window, None)
        })?;
        let (device, render_configuration) = instance
            .physical_devices()?
            .into_iter()
            .max_ok_filter_map(|physical_device| DeviceCandidate::new(physical_device, &surface))?
            .ok_or_else(|| eyre!("could not find suitable device"))?
            .create()
            .wrap_err("could not create device")?;
        let device = Arc::new(device);
        let shader_module =
            device.create_shader_module(ShaderModuleCreateFlags::empty(), &align_bytes(CODE))?;
        let swapchain = Arc::new(create_swapchain(&device, &surface, &window, false, None)?);
        let pipeline_cache =
            device.create_pipeline_cache(PipelineCacheCreateFlags::empty(), None)?;
        let descriptor_pool = Self::pool_setup().create_pool(&device)?;
        let egui = Egui::new(&device, &descriptor_pool, window.scale_factor() as f32)?;
        let debug = DebugContext::default();
        let semaphore = device.create_timeline_semaphore(0)?;
        let frame_count = 0;
        Ok(Self {
            frame_count,
            semaphore,
            pipeline_cache,
            render_configuration,
            egui,
            debug,
            descriptor_pool,
            swapchain,
            shader_module,
            device,
            surface,
            window,
            grab: false,
            vsync: true,
            windows: DebugWindows::default(),
            proxy,
            filter_handle,
        })
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        Egui::pool_setup()
    }

    pub fn create_render_pass(&self) -> VkResult<RenderPass> {
        let image_format = self.swapchain.image_format();
        let attachments = [
            AttachmentDescription::builder()
                .format(image_format)
                .samples(SampleCountFlags::TYPE_1)
                .load_op(AttachmentLoadOp::CLEAR)
                .store_op(AttachmentStoreOp::STORE)
                .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                .initial_layout(ImageLayout::UNDEFINED)
                .final_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .build(),
            AttachmentDescription::builder()
                .format(self.render_configuration.depth_format)
                .samples(SampleCountFlags::TYPE_1)
                .load_op(AttachmentLoadOp::CLEAR)
                .store_op(AttachmentStoreOp::STORE)
                .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                .initial_layout(ImageLayout::UNDEFINED)
                .final_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                .build(),
            AttachmentDescription::builder()
                .format(self.render_configuration.depth_format)
                .samples(SampleCountFlags::TYPE_1)
                .load_op(AttachmentLoadOp::LOAD)
                .store_op(AttachmentStoreOp::STORE)
                .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                .initial_layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                .final_layout(ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .build(),
            AttachmentDescription::builder()
                .format(image_format)
                .samples(SampleCountFlags::TYPE_1)
                .load_op(AttachmentLoadOp::LOAD)
                .store_op(AttachmentStoreOp::STORE)
                .stencil_load_op(AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(AttachmentStoreOp::DONT_CARE)
                .initial_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .final_layout(ImageLayout::PRESENT_SRC_KHR)
                .build(),
        ];
        let color_attachments = [AttachmentReference::builder()
            .attachment(0)
            .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];
        let post_color_attachments = [AttachmentReference::builder()
            .attachment(3)
            .layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];
        let pre_pass_depth_stencil_attachment = AttachmentReference::builder()
            .attachment(1)
            .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let depth_stencil_attachment = AttachmentReference::builder()
            .attachment(2)
            .layout(ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        let subpasses = [
            SubpassDescription::builder()
                .color_attachments(&[])
                .depth_stencil_attachment(&pre_pass_depth_stencil_attachment)
                .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
                .build(),
            SubpassDescription::builder()
                .color_attachments(&color_attachments)
                .depth_stencil_attachment(&depth_stencil_attachment)
                .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
                .build(),
            SubpassDescription::builder()
                .color_attachments(&post_color_attachments)
                .pipeline_bind_point(PipelineBindPoint::GRAPHICS)
                .build(),
        ];
        let dependencies = [
            SubpassDependency::builder()
                .src_subpass(SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(PipelineStageFlags::TOP_OF_PIPE)
                .dst_stage_mask(PipelineStageFlags::EARLY_FRAGMENT_TESTS)
                .src_access_mask(AccessFlags::empty())
                .dst_access_mask(AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
                .build(),
            SubpassDependency::builder()
                .src_subpass(0)
                .dst_subpass(1)
                .src_stage_mask(PipelineStageFlags::LATE_FRAGMENT_TESTS)
                .dst_stage_mask(PipelineStageFlags::EARLY_FRAGMENT_TESTS)
                .src_access_mask(AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
                .dst_access_mask(AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ)
                .build(),
            SubpassDependency::builder()
                .src_subpass(1)
                .dst_subpass(2)
                .src_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(AccessFlags::COLOR_ATTACHMENT_WRITE)
                .build(),
        ];
        let create_info = RenderPassCreateInfo::builder()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);
        self.device.create_render_pass(&create_info)
    }
}
