mod context;
mod frame;

pub use context::*;
pub use frame::*;
use vulkan::{DescriptorPool, DescriptorPoolSetup, Sampler};

use crate::{
    depth::DepthImage,
    frame::{PerFrame, FRAMES_IN_FLIGHT},
    root::RootContext,
    scene::Scene,
};

pub struct Cull {
    pub frames: PerFrame<CullFrame>,
    pub context: CullContext,
}

impl Cull {
    pub fn new(
        root_context: &RootContext,
        scene: &Scene,
        descriptor_pool: &DescriptorPool,
    ) -> eyre::Result<Self> {
        let context = CullContext::new(root_context)?;
        let frames = PerFrame::new(|i| {
            CullFrame::new(
                root_context,
                &context,
                &scene.context,
                &scene.frames[i],
                descriptor_pool,
            )
        })?;
        Ok(Self { frames, context })
    }

    pub fn prepare_render(
        &mut self,
        root_context: &RootContext,
        image: &DepthImage,
        sampler: &Sampler,
    ) {
        self.frames[root_context.frame_count].prepare_render(root_context, image, sampler)
    }

    pub fn pool_setup() -> DescriptorPoolSetup {
        CullFrame::pool_setup() * FRAMES_IN_FLIGHT as u32
    }
}
