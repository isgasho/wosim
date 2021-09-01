use std::{ffi::CString, mem::size_of};

use vulkan::{
    BlendFactor, BlendOp, ColorComponentFlags, CompareOp, CullModeFlags, DynamicState, Format,
    FrontFace, GraphicsPipelineCreateInfo, LogicOp, Pipeline, PipelineColorBlendAttachmentState,
    PipelineColorBlendStateCreateInfo, PipelineDepthStencilStateCreateInfo,
    PipelineDynamicStateCreateInfo, PipelineInputAssemblyStateCreateInfo,
    PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
    PipelineShaderStageCreateInfo, PipelineTessellationStateCreateInfo,
    PipelineVertexInputStateCreateInfo, PipelineViewportStateCreateInfo, PolygonMode,
    PrimitiveTopology, Rect2D, RenderPass, SampleCountFlags, ShaderStageFlags,
    VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate, Viewport,
    VkResult,
};

use crate::root::RootContext;

use super::{TerrainContext, Vertex};

pub struct TerrainPipelines {
    pub pre_pass: Pipeline,
    pub main: Pipeline,
}

impl TerrainPipelines {
    pub fn new(
        root_context: &RootContext,
        render_pass: &RenderPass,
        context: &TerrainContext,
    ) -> VkResult<Self> {
        let binding_descriptions = [VertexInputBindingDescription::builder()
            .binding(0)
            .stride(size_of::<Vertex>() as u32)
            .input_rate(VertexInputRate::VERTEX)
            .build()];
        let attribute_descriptions = [VertexInputAttributeDescription::builder()
            .binding(0)
            .location(0)
            .format(Format::R32G32B32_SFLOAT)
            .offset(0)
            .build()];
        let vertex_input_state = PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);
        let input_assembly_state = PipelineInputAssemblyStateCreateInfo::builder()
            .topology(PrimitiveTopology::PATCH_LIST)
            .primitive_restart_enable(false);
        let rasterization_state = PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(PolygonMode::FILL)
            .line_width(1f32)
            .cull_mode(CullModeFlags::BACK)
            .front_face(FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false)
            .depth_bias_constant_factor(0f32)
            .depth_bias_clamp(0f32)
            .depth_bias_slope_factor(0f32);
        let multisample_state = PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(SampleCountFlags::TYPE_1)
            .min_sample_shading(1f32)
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false);
        let color_blend_attachments = [PipelineColorBlendAttachmentState::builder()
            .color_write_mask(
                ColorComponentFlags::R
                    | ColorComponentFlags::G
                    | ColorComponentFlags::B
                    | ColorComponentFlags::A,
            )
            .blend_enable(false)
            .src_color_blend_factor(BlendFactor::ONE)
            .dst_color_blend_factor(BlendFactor::ZERO)
            .color_blend_op(BlendOp::ADD)
            .src_alpha_blend_factor(BlendFactor::ONE)
            .dst_alpha_blend_factor(BlendFactor::ZERO)
            .alpha_blend_op(BlendOp::ADD)
            .build()];
        let color_blend_state = PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(LogicOp::COPY)
            .attachments(&color_blend_attachments)
            .blend_constants([0f32, 0f32, 0f32, 0f32]);

        let main_vs = CString::new("terrain_vertex").unwrap();
        let main_fs = CString::new("default_fragment").unwrap();
        let main_tc = CString::new("terrain_tessellation_control").unwrap();
        let main_te = CString::new("terrain_tessellation_evaluation").unwrap();
        let pre_pass_stages = [
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::VERTEX)
                .module(*root_context.shader_module)
                .name(&main_vs)
                .build(),
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::TESSELLATION_CONTROL)
                .module(*root_context.shader_module)
                .name(&main_tc)
                .build(),
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::TESSELLATION_EVALUATION)
                .module(*root_context.shader_module)
                .name(&main_te)
                .build(),
        ];
        let stages = [
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::VERTEX)
                .module(*root_context.shader_module)
                .name(&main_vs)
                .build(),
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::TESSELLATION_CONTROL)
                .module(*root_context.shader_module)
                .name(&main_tc)
                .build(),
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::TESSELLATION_EVALUATION)
                .module(*root_context.shader_module)
                .name(&main_te)
                .build(),
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::FRAGMENT)
                .module(*root_context.shader_module)
                .name(&main_fs)
                .build(),
        ];
        let pre_pass_depth_stencil_state = PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(CompareOp::GREATER)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let depth_stencil_state = PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(true)
            .depth_write_enable(false)
            .depth_compare_op(CompareOp::GREATER_OR_EQUAL)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let tessellation_state =
            PipelineTessellationStateCreateInfo::builder().patch_control_points(4);
        let viewports = [Viewport::default()];
        let scissors = [Rect2D::default()];
        let viewport_state = PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);
        let dynamic_states = [DynamicState::VIEWPORT, DynamicState::SCISSOR];
        let dynamic_state =
            PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_states);
        let create_infos = [
            GraphicsPipelineCreateInfo::builder()
                .stages(&pre_pass_stages)
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly_state)
                .tessellation_state(&tessellation_state)
                .viewport_state(&viewport_state)
                .dynamic_state(&dynamic_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state)
                .color_blend_state(&color_blend_state)
                .depth_stencil_state(&pre_pass_depth_stencil_state)
                .layout(*context.pipeline_layout)
                .render_pass(**render_pass)
                .subpass(0)
                .build(),
            GraphicsPipelineCreateInfo::builder()
                .stages(&stages)
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly_state)
                .tessellation_state(&tessellation_state)
                .viewport_state(&viewport_state)
                .dynamic_state(&dynamic_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state)
                .color_blend_state(&color_blend_state)
                .depth_stencil_state(&depth_stencil_state)
                .layout(*context.pipeline_layout)
                .render_pass(**render_pass)
                .subpass(1)
                .build(),
        ];
        let mut pipelines = root_context.pipeline_cache.create_graphics(&create_infos)?;
        let pre_pass = pipelines.remove(0);
        let main = pipelines.remove(0);
        Ok(Self { pre_pass, main })
    }
}
