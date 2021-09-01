mod context;
mod data;
mod frame;
mod pipeline;

pub use context::*;
pub use data::*;
pub use frame::*;
pub use pipeline::*;
use vulkan::{DescriptorPool, DescriptorPoolSetup, Format, RenderPass};

use crate::{
    cache::Cache,
    frame::{PerFrame, FRAMES_IN_FLIGHT},
    root::{RootContext, RootFrame},
    scene::Scene,
    world::World,
};

pub struct Terrain {
    pub frames: PerFrame<TerrainFrame>,
    pub pipelines: Cache<Format, TerrainPipelines>,
    pub context: TerrainContext,
}

impl Terrain {
    pub fn new(
        root_context: &RootContext,
        scene: &Scene,
        descriptor_pool: &DescriptorPool,
        world: &World,
    ) -> eyre::Result<Self> {
        let context = TerrainContext::new(root_context, world)?;
        let frames = PerFrame::new(|i| {
            TerrainFrame::new(
                root_context,
                &context,
                &scene.frames[i],
                descriptor_pool,
                world,
            )
        })?;
        Ok(Self {
            frames,
            pipelines: Cache::default(),
            context,
        })
    }

    pub fn prepare_render(
        &mut self,
        root_context: &RootContext,
        root_frame: &RootFrame,
        world: &World,
    ) -> Result<(), vulkan::Error> {
        self.context.prepare_render(
            root_context,
            root_frame,
            world,
            &mut self.frames[root_context.frame_count].staging_buffer,
        )?;
        self.frames[root_context.frame_count].prepare_render(root_context, &self.context, world)?;
        Ok(())
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
                TerrainPipelines::new(root_context, render_pass, context)
            })?;
        self.frames[root_context.frame_count].render(
            root_frame,
            context,
            if pre_pass {
                &pipelines.pre_pass
            } else {
                &pipelines.main
            },
        );
        Ok(())
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        TerrainFrame::pool_setup() * FRAMES_IN_FLIGHT as u32
    }
}
