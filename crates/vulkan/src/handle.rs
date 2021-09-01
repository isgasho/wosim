use ash::{
    vk::{
        CommandBuffer, CommandPool, DescriptorPool, DescriptorSet, DescriptorSetLayout, Fence,
        Framebuffer, ImageView, Pipeline, PipelineCache, PipelineLayout, QueryPool, RenderPass,
        Sampler, Semaphore, ShaderModule,
    },
    Device,
};

pub trait Handle: Copy {
    /// # Safety
    unsafe fn destroy(self, device: &Device);
}

impl Handle for Fence {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_fence(self, None)
    }
}

impl Handle for CommandPool {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_command_pool(self, None)
    }
}

impl Handle for CommandBuffer {
    unsafe fn destroy(self, _device: &Device) {}
}

impl Handle for Semaphore {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_semaphore(self, None)
    }
}

impl Handle for ImageView {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_image_view(self, None)
    }
}

impl Handle for RenderPass {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_render_pass(self, None)
    }
}

impl Handle for ShaderModule {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_shader_module(self, None)
    }
}

impl Handle for PipelineCache {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_pipeline_cache(self, None)
    }
}

impl Handle for Pipeline {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_pipeline(self, None)
    }
}

impl Handle for PipelineLayout {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_pipeline_layout(self, None)
    }
}

impl Handle for DescriptorSetLayout {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_descriptor_set_layout(self, None)
    }
}

impl Handle for Framebuffer {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_framebuffer(self, None)
    }
}

impl Handle for DescriptorSet {
    unsafe fn destroy(self, _device: &Device) {}
}

impl Handle for DescriptorPool {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_descriptor_pool(self, None);
    }
}

impl Handle for Sampler {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_sampler(self, None);
    }
}

impl Handle for QueryPool {
    unsafe fn destroy(self, device: &Device) {
        device.destroy_query_pool(self, None);
    }
}

pub trait HandleWrapper {
    type Handle;
}

pub trait DerefHandle: Handle {}

impl DerefHandle for CommandBuffer {}
impl DerefHandle for Semaphore {}
impl DerefHandle for ShaderModule {}
impl DerefHandle for PipelineLayout {}
impl DerefHandle for RenderPass {}
impl DerefHandle for DescriptorSet {}
impl DerefHandle for ImageView {}
impl DerefHandle for Sampler {}
