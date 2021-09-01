mod context;
mod data;
mod frame;
mod pipeline;

pub use context::*;
pub use data::*;
pub use frame::*;
pub use pipeline::*;
use vulkan::{DescriptorPool, DescriptorPoolSetup, Format, RenderPass, VkResult, TRUE};

use crate::{
    cache::Cache,
    frame::{PerFrame, FRAMES_IN_FLIGHT},
    root::{RootContext, RootFrame},
    world::World,
};

pub struct Scene {
    pub frames: PerFrame<SceneFrame>,
    pub pipelines: Cache<Format, ScenePipelines>,
    pub context: SceneContext,
}

impl Scene {
    pub fn new(
        root_context: &RootContext,
        descriptor_pool: &DescriptorPool,
        camera: Camera,
    ) -> eyre::Result<Self> {
        let context = SceneContext::new(root_context, 100, 3 * 100, 100, camera)?;
        let frames = PerFrame::new(|_| {
            SceneFrame::new(root_context, &context, 2usize.pow(18), descriptor_pool)
        })?;
        Ok(Self {
            frames,
            pipelines: Cache::default(),
            context,
        })
    }

    pub fn update(&mut self, root_context: &RootContext, world: &mut World) -> VkResult<u32> {
        self.frames[root_context.frame_count].update(root_context, &self.context, world)
    }

    pub fn render(
        &mut self,
        root_context: &RootContext,
        render_pass: &RenderPass,
        root_frame: &RootFrame,
        pre_pass: bool,
    ) -> Result<(), vulkan::Error> {
        let context = &mut self.context;
        let pipelines = self
            .pipelines
            .try_get(root_context.swapchain.image_format(), || {
                ScenePipelines::new(root_context, render_pass, context)
            })?;
        self.frames[root_context.frame_count].render(
            root_frame,
            context,
            if pre_pass {
                &pipelines.pre_pass
            } else {
                &pipelines.main
            },
            root_context.render_configuration.use_draw_count == TRUE,
        );
        Ok(())
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        SceneFrame::pool_setup() * FRAMES_IN_FLIGHT as u32
    }
}
