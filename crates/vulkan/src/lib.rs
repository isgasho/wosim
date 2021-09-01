mod api_level;
mod buffer;
mod chain;
mod command;
mod descriptor;
mod device;
mod error;
mod handle;
mod image;
mod instance;
mod object;
mod physical_device;
mod surface;
mod swapchain;
mod version;

use std::{ffi::CStr, os::raw::c_char};

use ash::vk;

pub use api_level::*;
pub use buffer::*;
pub use command::*;
pub use descriptor::*;
pub use device::*;
pub use error::*;
pub use handle::*;
pub use image::*;
pub use instance::*;
pub use object::*;
pub use physical_device::*;
pub use surface::*;
pub use swapchain::*;
pub use version::*;

pub use ash::{
    prelude::VkResult,
    vk::{
        AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentReference,
        AttachmentStoreOp, BlendFactor, BlendOp, Bool32, BufferCopy, BufferCreateInfo,
        BufferImageCopy, BufferMemoryBarrier, BufferUsageFlags, ClearColorValue,
        ClearDepthStencilValue, ClearValue, ColorComponentFlags, ColorSpaceKHR, CommandBufferLevel,
        CommandBufferUsageFlags, CommandPoolCreateFlags, CommandPoolResetFlags, CompareOp,
        ComponentMapping, ComponentSwizzle, ComputePipelineCreateInfo, CopyDescriptorSet,
        CullModeFlags, DependencyFlags, DescriptorBufferInfo, DescriptorImageInfo,
        DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags, DescriptorType,
        DrawIndexedIndirectCommand, DynamicState, ExtensionProperties, Extent2D, Extent3D,
        FenceCreateFlags, Filter, Format, FormatFeatureFlags, FramebufferCreateFlags, FrontFace,
        GraphicsPipelineCreateInfo, ImageAspectFlags, ImageCreateInfo, ImageLayout,
        ImageMemoryBarrier, ImageSubresourceLayers, ImageSubresourceRange, ImageTiling, ImageType,
        ImageUsageFlags, ImageViewCreateFlags, ImageViewCreateInfo, ImageViewType, IndexType,
        KhrPortabilitySubsetFn, KhrShaderDrawParametersFn, KhrShaderFloat16Int8Fn,
        KhrTimelineSemaphoreFn, LogicOp, MemoryBarrier, MemoryPropertyFlags, Offset2D,
        PipelineBindPoint, PipelineCacheCreateFlags, PipelineColorBlendAttachmentState,
        PipelineColorBlendStateCreateInfo, PipelineDepthStencilStateCreateInfo,
        PipelineDynamicStateCreateInfo, PipelineInputAssemblyStateCreateInfo,
        PipelineLayoutCreateFlags, PipelineMultisampleStateCreateInfo,
        PipelineRasterizationStateCreateInfo, PipelineShaderStageCreateInfo, PipelineStageFlags,
        PipelineTessellationStateCreateInfo, PipelineVertexInputStateCreateInfo,
        PipelineViewportStateCreateInfo, PolygonMode, PresentModeKHR, PrimitiveTopology,
        PushConstantRange, QueryPipelineStatisticFlags, QueryResultFlags, QueryType,
        QueueFamilyProperties, QueueFlags, Rect2D, RenderPassCreateInfo, SampleCountFlags,
        SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode, SamplerReductionMode,
        SamplerReductionModeCreateInfo, ShaderModuleCreateFlags, ShaderStageFlags, SharingMode,
        SpecializationInfo, SpecializationMapEntry, SubmitInfo, SubpassContents, SubpassDependency,
        SubpassDescription, SurfaceFormatKHR, SwapchainKHR, TimelineSemaphoreSubmitInfo,
        VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate, Viewport,
        WriteDescriptorSet, FALSE, LOD_CLAMP_NONE, SUBPASS_EXTERNAL, TRUE, WHOLE_SIZE,
    },
};

pub type PhysicalDeviceHandle = vk::PhysicalDevice;

pub use gpu_allocator::MemoryLocation;

pub use bytemuck::{bytes_of, Pod, Zeroable};

pub type ApiResult = vk::Result;

pub fn contains_extension(extensions: &[ExtensionProperties], extension_name: &CStr) -> bool {
    for extension in extensions {
        if extension_name == unsafe { to_cstr(&extension.extension_name) } {
            return true;
        }
    }
    false
}

unsafe fn to_cstr(data: &[c_char]) -> &CStr {
    CStr::from_ptr(data.as_ptr())
}
