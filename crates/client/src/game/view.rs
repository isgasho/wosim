use std::sync::Arc;

use eyre::Context;
use vulkan::{
    mip_levels_for_extent, CommandBuffer, Device, Extent2D, ImageView, PipelineCache, RenderPass,
};

use crate::{
    depth::{DepthContext, DepthView},
    renderer::RenderConfiguration,
    scene::{SceneContext, SceneView},
    terrain::{TerrainContext, TerrainView},
};

pub struct GameView {
    pub depth: DepthView,
    pub terrain: TerrainView,
    pub scene: SceneView,
}

impl GameView {
    pub fn new(
        device: &Arc<Device>,
        image_extent: Extent2D,
        pipeline_cache: &PipelineCache,
        depth_view: &ImageView,
        scene: &SceneContext,
        terrain: &TerrainContext,
        depth: &DepthContext,
        render_pass: &RenderPass,
        render_configuration: &RenderConfiguration,
        command_buffer: &CommandBuffer,
    ) -> eyre::Result<Self> {
        let depth_pyramid_mip_levels = mip_levels_for_extent(image_extent);
        let scene = SceneView::new(scene, render_pass, pipeline_cache, 0, image_extent)
            .wrap_err("could not create scene view")?;
        let terrain = TerrainView::new(terrain, render_pass, pipeline_cache, 0, image_extent)?;
        let depth = DepthView::new(
            device,
            depth,
            render_configuration,
            image_extent,
            depth_pyramid_mip_levels,
            depth_view,
            command_buffer,
        )
        .wrap_err("could not create depth view")?;
        Ok(Self {
            depth,
            terrain,
            scene,
        })
    }
}
