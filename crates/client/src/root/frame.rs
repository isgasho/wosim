use eyre::Context;
use vulkan::{
    ClearColorValue, ClearDepthStencilValue, ClearValue, CommandBuffer, CommandBufferLevel,
    CommandBufferUsageFlags, CommandPool, CommandPoolCreateFlags, CommandPoolResetFlags, Fence,
    FenceCreateFlags, Offset2D, PipelineStageFlags, QueryPipelineStatisticFlags, QueryPool,
    QueryResultFlags, QueryType, Rect2D, RenderPass, Semaphore, SubmitInfo, SubpassContents,
    TimelineSemaphoreSubmitInfo, Viewport,
};

use crate::renderer::{RenderError, RenderResult, RenderTimestamps};

use super::{RootContext, RootState, RootSurface};

pub struct RootFrame {
    pub command_buffer: CommandBuffer,
    pub main_queue_fence: Fence,
    pub image_ready: Semaphore,
    pub render_finished: Semaphore,
    pub timestamp_pool: QueryPool,
    pub command_pool: CommandPool,
}

impl RootFrame {
    pub fn new(context: &RootContext) -> eyre::Result<Self> {
        let command_pool = context.device.create_command_pool(
            CommandPoolCreateFlags::TRANSIENT,
            context.device.main_queue_family_index(),
        )?;
        let timestamp_pool = context.device.create_query_pool(
            QueryType::TIMESTAMP,
            2,
            QueryPipelineStatisticFlags::empty(),
        )?;
        let mut command_buffers = command_pool.allocate(CommandBufferLevel::PRIMARY, 1)?;
        let command_buffer = command_buffers.remove(0);
        let main_queue_fence = context.device.create_fence(FenceCreateFlags::SIGNALED)?;
        let image_ready = context.device.create_semaphore()?;
        let render_finished = context.device.create_semaphore()?;
        Ok(Self {
            command_buffer,
            main_queue_fence,
            image_ready,
            render_finished,
            timestamp_pool,
            command_pool,
        })
    }

    pub fn render(
        &mut self,
        context: &mut RootContext,
        state: &mut RootState,
        render_pass: &RenderPass,
        surface: &RootSurface,
    ) -> Result<RenderResult, RenderError> {
        self.main_queue_fence.wait()?;
        self.main_queue_fence.reset()?;
        let timestamps: Option<Vec<u64>> =
            self.timestamp_pool
                .results(0, 2, QueryResultFlags::TYPE_64)?;
        self.command_pool.reset(CommandPoolResetFlags::empty())?;
        self.command_buffer
            .begin(CommandBufferUsageFlags::ONE_TIME_SUBMIT, None)?;
        self.command_buffer
            .reset_query_pool(&self.timestamp_pool, 0, 2);
        self.command_buffer.write_timestamp(
            PipelineStageFlags::TOP_OF_PIPE,
            &self.timestamp_pool,
            0,
        );
        context
            .egui
            .prepare_render(self, &context.device, context.frame_count)
            .wrap_err("could not prepare egui rendering")?;
        state.prepare_render(context, surface, self)?;
        let (image_index, suboptimal) = surface.swapchain.acquire_next_image(&self.image_ready)?;
        let clear_values = [
            ClearValue {
                color: ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            },
            ClearValue {
                depth_stencil: ClearDepthStencilValue {
                    depth: 0f32,
                    stencil: 0,
                },
            },
            ClearValue::default(),
        ];
        self.command_buffer.begin_render_pass(
            render_pass,
            &surface.framebuffers[image_index as usize],
            Rect2D {
                offset: Offset2D { x: 0, y: 0 },
                extent: surface.swapchain.image_extent(),
            },
            &clear_values,
            SubpassContents::INLINE,
        );
        let extent = context.swapchain.image_extent();
        self.command_buffer.set_viewport(
            0,
            &[Viewport {
                x: 0f32,
                y: 0f32,
                width: extent.width as f32,
                height: extent.height as f32,
                min_depth: 0f32,
                max_depth: 1f32,
            }],
        );
        self.command_buffer.set_scissor(
            0,
            &[Rect2D {
                offset: Offset2D { x: 0, y: 0 },
                extent,
            }],
        );
        state
            .render(context, render_pass, self, true)
            .wrap_err("could not render pre-pass")?;
        self.command_buffer.next_subpass(SubpassContents::INLINE);
        state
            .render(context, render_pass, self, false)
            .wrap_err("could not render main pass")?;
        self.command_buffer.next_subpass(SubpassContents::INLINE);
        context
            .egui
            .render(
                render_pass,
                surface,
                self,
                &context.device,
                &context.shader_module,
                &context.pipeline_cache,
                context.frame_count,
            )
            .wrap_err("could not render egui")?;
        self.command_buffer.end_render_pass();
        self.command_buffer.write_timestamp(
            PipelineStageFlags::BOTTOM_OF_PIPE,
            &self.timestamp_pool,
            1,
        );
        self.command_buffer.end()?;
        let command_buffers = [*self.command_buffer];
        let signal_semaphores = [*self.render_finished, *context.semaphore];
        let wait_semaphores = [*self.image_ready, *context.semaphore];
        let wait_semaphore_values = [0, context.frame_count as u64];
        let signal_semaphore_values = [0, context.frame_count as u64 + 1];
        let wait_dst_stage_mask = [
            PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            PipelineStageFlags::COMPUTE_SHADER,
        ];
        let mut timeline = TimelineSemaphoreSubmitInfo::builder()
            .wait_semaphore_values(&wait_semaphore_values)
            .signal_semaphore_values(&signal_semaphore_values);
        let submits = [SubmitInfo::builder()
            .command_buffers(&command_buffers)
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_dst_stage_mask)
            .signal_semaphores(&signal_semaphores)
            .push_next(&mut timeline)
            .build()];
        context.device.submit(&submits, &self.main_queue_fence)?;
        let suboptimal = surface
            .swapchain
            .present(image_index, &self.render_finished)?
            || suboptimal;
        let timestamps = timestamps.map(|timestamps| RenderTimestamps {
            begin: timestamps[0] as f64 * context.render_configuration.timestamp_period,
            end: timestamps[1] as f64 * context.render_configuration.timestamp_period,
        });
        Ok(RenderResult {
            suboptimal,
            timestamps,
        })
    }
}
