use std::{
    ffi::CString,
    sync::{Arc, Mutex},
};

use egui::{ClippedMesh, CtxRef, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Vec2};

use copypasta::{ClipboardContext, ClipboardProvider};

use eyre::eyre;
use winit::{
    event::{
        ElementState, ModifiersState, MouseButton, MouseScrollDelta, VirtualKeyCode, WindowEvent,
    },
    window::{CursorIcon, Window},
};

use util::handle::HandleFlow;
use vulkan::{
    BlendFactor, BlendOp, ColorComponentFlags, CullModeFlags, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags, DescriptorType, Device,
    DynamicState, Filter, Format, GraphicsPipelineCreateInfo, LogicOp, Pipeline, PipelineCache,
    PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo,
    PipelineDepthStencilStateCreateInfo, PipelineDynamicStateCreateInfo,
    PipelineInputAssemblyStateCreateInfo, PipelineLayout, PipelineLayoutCreateFlags,
    PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo,
    PipelineShaderStageCreateInfo, PipelineVertexInputStateCreateInfo,
    PipelineViewportStateCreateInfo, PolygonMode, PrimitiveTopology, PushConstantRange, Rect2D,
    RenderPass, SampleCountFlags, Sampler, SamplerAddressMode, SamplerCreateInfo,
    SamplerMipmapMode, ShaderModule, ShaderStageFlags, VertexInputAttributeDescription,
    VertexInputBindingDescription, VertexInputRate, Viewport, LOD_CLAMP_NONE,
};

use super::Font;

pub struct EguiContext {
    pub(super) inner: CtxRef,
    input: RawInput,
    pub(super) pipeline_layout: PipelineLayout,
    pub(super) sampler: Sampler,
    pub(super) meshes: Arc<Vec<ClippedMesh>>,
    pub(super) set_layout: DescriptorSetLayout,
    pub(super) font: Option<Arc<Font>>,
    cursor_pos: Pos2,
    modifiers: Modifiers,
    clipboard: Arc<Mutex<ClipboardContext>>,
}

impl EguiContext {
    pub fn new(device: &Arc<Device>, scale_factor: f32) -> eyre::Result<Self> {
        let inner = CtxRef::default();
        let mut input = RawInput {
            pixels_per_point: Some(scale_factor),
            ..Default::default()
        };
        input.pixels_per_point = Some(scale_factor);
        let bindings = [
            DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(1)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .build(),
            DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_type(DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .build(),
        ];
        let set_layout = device
            .create_descriptor_set_layout(DescriptorSetLayoutCreateFlags::empty(), &bindings)?;
        let set_layouts = [&set_layout];
        let push_constant_ranges = [
            PushConstantRange::builder()
                .offset(0)
                .size(8)
                .stage_flags(ShaderStageFlags::VERTEX)
                .build(),
            PushConstantRange::builder()
                .offset(8)
                .size(4)
                .stage_flags(ShaderStageFlags::FRAGMENT)
                .build(),
        ];
        let pipeline_layout = device.create_pipeline_layout(
            PipelineLayoutCreateFlags::empty(),
            &set_layouts,
            &push_constant_ranges,
        )?;
        let sampler = device.create_sampler(
            &SamplerCreateInfo::builder()
                .address_mode_u(SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(SamplerAddressMode::CLAMP_TO_EDGE)
                .anisotropy_enable(false)
                .min_filter(Filter::LINEAR)
                .mag_filter(Filter::LINEAR)
                .mipmap_mode(SamplerMipmapMode::LINEAR)
                .min_lod(0.0)
                .max_lod(LOD_CLAMP_NONE),
        )?;
        Ok(Self {
            inner,
            input,
            meshes: Arc::new(Vec::new()),
            pipeline_layout,
            set_layout,
            sampler,
            font: None,
            cursor_pos: Pos2::default(),
            modifiers: Modifiers::default(),
            clipboard: Arc::new(Mutex::new(ClipboardContext::new().map_err(|e| {
                eyre!("could not create clipboard context. caused by {}", e)
            })?)),
        })
    }

    pub fn handle_event(&mut self, event: &::winit::event::Event<()>) -> HandleFlow {
        if let ::winit::event::Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Resized(size) => {
                    let pixels_per_point = self
                        .input
                        .pixels_per_point
                        .unwrap_or_else(|| self.inner.pixels_per_point());
                    self.input.screen_rect = Some(egui::Rect::from_min_size(
                        Default::default(),
                        Vec2 {
                            x: size.width as f32,
                            y: size.height as f32,
                        } / pixels_per_point,
                    ));
                }
                WindowEvent::ReceivedCharacter(c) => {
                    if self.inner.wants_keyboard_input() && !c.is_ascii_control() {
                        self.input.events.push(Event::Text(c.to_string()));
                        return HandleFlow::Handled;
                    }
                }
                WindowEvent::KeyboardInput { input, .. } => {
                    if self.inner.wants_keyboard_input() {
                        if let Some(code) = input.virtual_keycode {
                            if let Some(key) = key(code) {
                                self.input.events.push(Event::Key {
                                    key,
                                    pressed: input.state == ElementState::Pressed,
                                    modifiers: self.modifiers,
                                });
                                return HandleFlow::Handled;
                            }
                        }
                    }
                }
                WindowEvent::ModifiersChanged(state) => {
                    self.modifiers = modifiers(*state);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let scale_factor = self.scale_factor();
                    self.cursor_pos = Pos2 {
                        x: position.x as f32 / scale_factor,
                        y: position.y as f32 / scale_factor,
                    };
                    self.input
                        .events
                        .push(egui::Event::PointerMoved(self.cursor_pos));
                }
                WindowEvent::CursorLeft { .. } => {
                    self.input.events.push(Event::PointerGone);
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    match *delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            self.input.scroll_delta = Vec2 { x, y } * 24.0;
                        }
                        MouseScrollDelta::PixelDelta(delta) => {
                            self.input.scroll_delta = Vec2 {
                                x: delta.x as f32,
                                y: delta.y as f32,
                            };
                        }
                    };
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if let Some(button) = pointer_button(*button) {
                        self.input.events.push(Event::PointerButton {
                            pos: self.cursor_pos,
                            button,
                            pressed: *state == ElementState::Pressed,
                            modifiers: self.modifiers,
                        });
                        if self.inner.wants_pointer_input() {
                            return HandleFlow::Handled;
                        }
                    }
                }
                WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    new_inner_size,
                } => {
                    self.input.pixels_per_point = Some(*scale_factor as f32);
                    self.input.screen_rect = Some(egui::Rect::from_min_size(
                        Default::default(),
                        Vec2 {
                            x: new_inner_size.width as f32,
                            y: new_inner_size.height as f32,
                        } / self.scale_factor(),
                    ));
                }
                _ => {}
            }
        }
        HandleFlow::Unhandled
    }

    pub fn begin(&mut self) -> CtxRef {
        self.inner.begin_frame(self.input.take());
        self.inner.clone()
    }

    pub fn end(&mut self, window: Option<&Window>) -> eyre::Result<()> {
        let (output, shapes) = self.inner.end_frame();
        let meshes = Arc::new(self.inner.tessellate(shapes));
        self.meshes = meshes;
        if let Some(window) = window {
            if let Some(icon) = cursor(output.cursor_icon) {
                window.set_cursor_icon(icon);
                window.set_cursor_visible(true);
            } else {
                window.set_cursor_visible(false);
            }
        }
        if !output.copied_text.is_empty() {
            self.clipboard
                .lock()
                .unwrap()
                .set_contents(output.copied_text)
                .map_err(|e| eyre!("could not set clipboard contents. caused by {}", e))?;
        }
        if let Some(open_url) = output.open_url {
            webbrowser::open(&open_url.url)?;
        }
        Ok(())
    }

    fn scale_factor(&self) -> f32 {
        self.input
            .pixels_per_point
            .unwrap_or_else(|| self.inner.pixels_per_point())
    }

    pub fn create_pipeline(
        &self,
        render_pass: &RenderPass,
        shader_module: &ShaderModule,
        pipeline_cache: &PipelineCache,
    ) -> Result<Pipeline, vulkan::Error> {
        let binding_descriptions = [VertexInputBindingDescription::builder()
            .binding(0)
            .input_rate(VertexInputRate::VERTEX)
            .stride(20)
            .build()];
        let attribute_descriptions = [
            VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(Format::R32G32_SFLOAT)
                .offset(0)
                .build(),
            VertexInputAttributeDescription::builder()
                .binding(0)
                .location(1)
                .format(Format::R32G32_SFLOAT)
                .offset(8)
                .build(),
            VertexInputAttributeDescription::builder()
                .binding(0)
                .location(2)
                .format(Format::R8G8B8A8_UNORM)
                .offset(16)
                .build(),
        ];
        let vertex_input_state = PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);
        let input_assembly_state = PipelineInputAssemblyStateCreateInfo::builder()
            .topology(PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);
        let rasterization_state = PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(PolygonMode::FILL)
            .line_width(1f32)
            .cull_mode(CullModeFlags::NONE)
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
            .blend_enable(true)
            .src_color_blend_factor(BlendFactor::ONE)
            .dst_color_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(BlendOp::ADD)
            .src_alpha_blend_factor(BlendFactor::ZERO)
            .dst_alpha_blend_factor(BlendFactor::ZERO)
            .alpha_blend_op(BlendOp::ADD)
            .build()];
        let color_blend_state = PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .logic_op(LogicOp::COPY)
            .attachments(&color_blend_attachments)
            .blend_constants([0f32, 0f32, 0f32, 0f32]);
        let main_vs = CString::new("egui_vertex").unwrap();
        let main_fs = CString::new("egui_fragment").unwrap();
        let stages = [
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::VERTEX)
                .module(**shader_module)
                .name(&main_vs)
                .build(),
            PipelineShaderStageCreateInfo::builder()
                .stage(ShaderStageFlags::FRAGMENT)
                .module(**shader_module)
                .name(&main_fs)
                .build(),
        ];
        let viewports = [Viewport::default()];
        let scissors = [Rect2D::default()];
        let viewport_state = PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);
        let dynamic_states = [DynamicState::VIEWPORT, DynamicState::SCISSOR];
        let dynamic_state =
            PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_states);
        let depth_stencil_state = PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(false)
            .depth_write_enable(false)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        let create_infos = [GraphicsPipelineCreateInfo::builder()
            .stages(&stages)
            .vertex_input_state(&vertex_input_state)
            .input_assembly_state(&input_assembly_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .depth_stencil_state(&depth_stencil_state)
            .layout(*self.pipeline_layout)
            .viewport_state(&viewport_state)
            .dynamic_state(&dynamic_state)
            .render_pass(**render_pass)
            .subpass(2)
            .build()];
        let mut pipelines = pipeline_cache.create_graphics(&create_infos)?;
        Ok(pipelines.remove(0))
    }
}

fn key(code: VirtualKeyCode) -> Option<Key> {
    Some(match code {
        VirtualKeyCode::Key1 => Key::Num1,
        VirtualKeyCode::Key2 => Key::Num2,
        VirtualKeyCode::Key3 => Key::Num3,
        VirtualKeyCode::Key4 => Key::Num4,
        VirtualKeyCode::Key5 => Key::Num5,
        VirtualKeyCode::Key6 => Key::Num6,
        VirtualKeyCode::Key7 => Key::Num7,
        VirtualKeyCode::Key8 => Key::Num8,
        VirtualKeyCode::Key9 => Key::Num9,
        VirtualKeyCode::Key0 => Key::Num0,
        VirtualKeyCode::A => Key::A,
        VirtualKeyCode::B => Key::B,
        VirtualKeyCode::C => Key::C,
        VirtualKeyCode::D => Key::D,
        VirtualKeyCode::E => Key::E,
        VirtualKeyCode::F => Key::F,
        VirtualKeyCode::G => Key::G,
        VirtualKeyCode::H => Key::H,
        VirtualKeyCode::I => Key::I,
        VirtualKeyCode::J => Key::J,
        VirtualKeyCode::K => Key::K,
        VirtualKeyCode::L => Key::L,
        VirtualKeyCode::M => Key::M,
        VirtualKeyCode::N => Key::N,
        VirtualKeyCode::O => Key::O,
        VirtualKeyCode::P => Key::P,
        VirtualKeyCode::Q => Key::Q,
        VirtualKeyCode::R => Key::R,
        VirtualKeyCode::S => Key::S,
        VirtualKeyCode::T => Key::T,
        VirtualKeyCode::U => Key::U,
        VirtualKeyCode::V => Key::V,
        VirtualKeyCode::W => Key::W,
        VirtualKeyCode::X => Key::X,
        VirtualKeyCode::Y => Key::Y,
        VirtualKeyCode::Z => Key::Z,
        VirtualKeyCode::Escape => Key::Escape,
        VirtualKeyCode::Insert => Key::Insert,
        VirtualKeyCode::Home => Key::Home,
        VirtualKeyCode::Delete => Key::Delete,
        VirtualKeyCode::End => Key::End,
        VirtualKeyCode::PageDown => Key::PageDown,
        VirtualKeyCode::PageUp => Key::PageUp,
        VirtualKeyCode::Left => Key::ArrowLeft,
        VirtualKeyCode::Up => Key::ArrowUp,
        VirtualKeyCode::Right => Key::ArrowRight,
        VirtualKeyCode::Down => Key::ArrowDown,
        VirtualKeyCode::Back => Key::Backspace,
        VirtualKeyCode::Return => Key::Enter,
        VirtualKeyCode::Space => Key::Space,
        VirtualKeyCode::Numpad0 => Key::Num0,
        VirtualKeyCode::Numpad1 => Key::Num1,
        VirtualKeyCode::Numpad2 => Key::Num2,
        VirtualKeyCode::Numpad3 => Key::Num3,
        VirtualKeyCode::Numpad4 => Key::Num4,
        VirtualKeyCode::Numpad5 => Key::Num5,
        VirtualKeyCode::Numpad6 => Key::Num6,
        VirtualKeyCode::Numpad7 => Key::Num7,
        VirtualKeyCode::Numpad8 => Key::Num8,
        VirtualKeyCode::Numpad9 => Key::Num9,
        VirtualKeyCode::NumpadEnter => Key::Enter,
        VirtualKeyCode::Tab => Key::Tab,
        _ => return None,
    })
}

fn modifiers(state: ModifiersState) -> Modifiers {
    Modifiers {
        alt: state.alt(),
        ctrl: state.ctrl(),
        #[cfg(target_os = "macos")]
        mac_cmd: state.logo(),
        #[cfg(not(target_os = "macos"))]
        mac_cmd: false,
        #[cfg(target_os = "macos")]
        command: state.logo(),
        #[cfg(not(target_os = "macos"))]
        command: state.ctrl(),
        shift: state.shift(),
    }
}

fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    Some(match button {
        MouseButton::Left => PointerButton::Primary,
        MouseButton::Right => PointerButton::Secondary,
        MouseButton::Middle => PointerButton::Middle,
        _ => return None,
    })
}

fn cursor(icon: egui::CursorIcon) -> Option<CursorIcon> {
    Some(match icon {
        egui::CursorIcon::Default => CursorIcon::Default,
        egui::CursorIcon::None => return None,
        egui::CursorIcon::ContextMenu => CursorIcon::ContextMenu,
        egui::CursorIcon::Help => CursorIcon::Help,
        egui::CursorIcon::PointingHand => CursorIcon::Hand,
        egui::CursorIcon::Progress => CursorIcon::Progress,
        egui::CursorIcon::Wait => CursorIcon::Wait,
        egui::CursorIcon::Cell => CursorIcon::Cell,
        egui::CursorIcon::Crosshair => CursorIcon::Crosshair,
        egui::CursorIcon::Text => CursorIcon::Text,
        egui::CursorIcon::VerticalText => CursorIcon::VerticalText,
        egui::CursorIcon::Alias => CursorIcon::Alias,
        egui::CursorIcon::Copy => CursorIcon::Copy,
        egui::CursorIcon::Move => CursorIcon::Move,
        egui::CursorIcon::NoDrop => CursorIcon::NoDrop,
        egui::CursorIcon::NotAllowed => CursorIcon::NotAllowed,
        egui::CursorIcon::Grab => CursorIcon::Grab,
        egui::CursorIcon::Grabbing => CursorIcon::Grabbing,
        egui::CursorIcon::AllScroll => CursorIcon::AllScroll,
        egui::CursorIcon::ResizeHorizontal => CursorIcon::ColResize,
        egui::CursorIcon::ResizeNeSw => CursorIcon::NeswResize,
        egui::CursorIcon::ResizeNwSe => CursorIcon::NwseResize,
        egui::CursorIcon::ResizeVertical => CursorIcon::RowResize,
        egui::CursorIcon::ZoomIn => CursorIcon::ZoomIn,
        egui::CursorIcon::ZoomOut => CursorIcon::ZoomOut,
    })
}
