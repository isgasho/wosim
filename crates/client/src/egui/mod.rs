mod context;
mod font;
mod frame;

use std::sync::Arc;

use font::*;

pub use context::*;
pub use frame::*;
use vulkan::{
    DescriptorPool, DescriptorPoolSetup, Device, Format, Pipeline, PipelineCache, RenderPass,
    ShaderModule,
};

use crate::{
    cache::Cache,
    frame::{PerFrame, FRAMES_IN_FLIGHT},
    root::{RootFrame, RootSurface},
};

pub struct Egui {
    pub frames: PerFrame<EguiFrame>,
    pub pipeline: Cache<Format, Pipeline>,
    pub context: EguiContext,
}

impl Egui {
    pub fn new(
        device: &Arc<Device>,
        descriptor_pool: &DescriptorPool,
        scale_factor: f32,
    ) -> eyre::Result<Self> {
        let context = EguiContext::new(device, scale_factor)?;
        let frames = PerFrame::new(|_| EguiFrame::new(device, &context, descriptor_pool))?;
        Ok(Self {
            frames,
            pipeline: Cache::default(),
            context,
        })
    }

    pub fn prepare_render(
        &mut self,
        root_frame: &mut RootFrame,
        device: &Arc<Device>,
        frame_count: usize,
    ) -> Result<(), vulkan::Error> {
        self.frames[frame_count].prepare(&mut self.context, root_frame, device)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        render_pass: &RenderPass,
        root_surface: &RootSurface,
        root_frame: &mut RootFrame,
        device: &Arc<Device>,
        shader_module: &ShaderModule,
        pipeline_cache: &PipelineCache,
        frame_count: usize,
    ) -> Result<(), vulkan::Error> {
        let context = &mut self.context;
        let pipeline = self
            .pipeline
            .try_get(root_surface.swapchain.image_format(), || {
                context.create_pipeline(render_pass, shader_module, pipeline_cache)
            })?;
        self.frames[frame_count].render(root_frame, root_surface, context, device, pipeline)
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        EguiFrame::pool_setup() * FRAMES_IN_FLIGHT as u32
    }
}
