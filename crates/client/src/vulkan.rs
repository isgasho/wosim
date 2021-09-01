use std::{
    cmp::{Ordering, Reverse},
    sync::Arc,
};

use crate::renderer::RenderConfiguration;
use eyre::eyre;
use vulkan::{
    cmp_device_types, contains_extension, ApiLevel, ColorSpaceKHR, Device, DeviceConfiguration,
    Extent2D, Format, FormatFeatureFlags, ImageTiling, KhrPortabilitySubsetFn,
    KhrShaderFloat16Int8Fn, KhrTimelineSemaphoreFn, PhysicalDevice, PhysicalDeviceFeatures,
    PhysicalDeviceHandle, PresentModeKHR, QueueFlags, Surface, SurfaceFormatKHR, Swapchain,
    SwapchainConfiguration, VkResult, FALSE, TRUE,
};
use winit::window::Window;

pub struct DeviceCandidate {
    physical_device: PhysicalDevice,
    device_configuration: DeviceConfiguration,
    render_configuration: RenderConfiguration,
}

impl DeviceCandidate {
    pub fn new(physical_device: PhysicalDevice, surface: &Surface) -> VkResult<Option<Self>> {
        if choose_surface_format(surface, physical_device.handle())?.is_none()
            || choose_present_mode(surface, physical_device.handle(), false)?.is_none()
        {
            return Ok(None);
        };
        let properties = physical_device.properties();
        let extensions = physical_device.extensions();
        let features = physical_device.features();
        let mut enabled_features = PhysicalDeviceFeatures::default();
        if !contains_extension(extensions, Swapchain::extension_name()) {
            return Ok(None);
        }
        let mut extension_names = vec![Swapchain::extension_name()];
        #[cfg(not(target_os = "macos"))]
        if physical_device.api_level() >= ApiLevel::Vulkan12
            && features.vulkan_12.vulkan_memory_model == TRUE
            && features.vulkan_12.vulkan_memory_model_device_scope == TRUE
        {
            enabled_features.vulkan_12.vulkan_memory_model = TRUE;
            enabled_features.vulkan_12.vulkan_memory_model_device_scope = TRUE;
        } else {
            return Ok(None);
        }
        match physical_device.api_level() {
            ApiLevel::Vulkan11 => {
                if features.shader_draw_parameters.shader_draw_parameters == TRUE
                    && features.float16_int8.shader_int8 == TRUE
                    && features.timeline_semaphore.timeline_semaphore == TRUE
                {
                    enabled_features
                        .shader_draw_parameters
                        .shader_draw_parameters = TRUE;
                    enabled_features.float16_int8.shader_int8 = TRUE;
                    enabled_features.timeline_semaphore.timeline_semaphore = TRUE;
                    extension_names.push(KhrShaderFloat16Int8Fn::name());
                    extension_names.push(KhrTimelineSemaphoreFn::name());
                } else {
                    return Ok(None);
                }
            }
            ApiLevel::Vulkan12 => {
                if features.vulkan_11.shader_draw_parameters == TRUE
                    && features.vulkan_12.shader_int8 == TRUE
                    && features.vulkan_12.timeline_semaphore == TRUE
                {
                    enabled_features.vulkan_11.shader_draw_parameters = TRUE;
                    enabled_features.vulkan_12.shader_int8 = TRUE;
                    enabled_features.vulkan_12.timeline_semaphore = TRUE;
                } else {
                    return Ok(None);
                }
            }
        }
        let use_draw_count = if features.vulkan_12.draw_indirect_count == TRUE {
            enabled_features.vulkan_12.draw_indirect_count = TRUE;
            TRUE
        } else {
            FALSE
        };
        if features.vulkan_10.features.tessellation_shader == FALSE
            || features.vulkan_10.features.multi_draw_indirect == FALSE
        {
            return Ok(None);
        }
        enabled_features.vulkan_10.features.tessellation_shader = TRUE;
        enabled_features.vulkan_10.features.multi_draw_indirect = TRUE;
        if contains_extension(extensions, KhrPortabilitySubsetFn::name()) {
            if ![1, 2, 4, 5, 10, 20].contains(
                &properties
                    .portability_subset
                    .min_vertex_input_binding_stride_alignment,
            ) {
                return Ok(None);
            }
            if features.portability_subset.image_view_format_swizzle == FALSE {
                return Ok(None);
            }
            enabled_features
                .portability_subset
                .image_view_format_swizzle = TRUE;
            extension_names.push(KhrPortabilitySubsetFn::name());
        }
        let families = physical_device.queue_families();
        let main_queue_family_index = match families
            .iter()
            .enumerate()
            .map(|(index, properties)| (index as u32, properties))
            .find(|(index, properties)| {
                match physical_device.surface_support(surface, *index) {
                    Ok(support) => {
                        if !support {
                            return false;
                        }
                    }
                    Err(_) => return false,
                }
                if !properties.queue_flags.contains(QueueFlags::GRAPHICS) {
                    return false;
                }
                properties.queue_flags.contains(QueueFlags::COMPUTE)
            })
            .map(|(index, _)| index as u32)
        {
            Some(index) => index,
            None => return Ok(None),
        };
        let transfer_queue_family_index = families
            .iter()
            .enumerate()
            .map(|(index, properties)| (index as u32, properties))
            .find(|(_, properties)| {
                properties.queue_flags.contains(QueueFlags::TRANSFER)
                    && !properties.queue_flags.contains(QueueFlags::GRAPHICS)
                    && !properties.queue_flags.contains(QueueFlags::COMPUTE)
            })
            .map(|(index, _)| index as u32);
        let device_configuration = DeviceConfiguration {
            extension_names,
            features: enabled_features,
            main_queue_family_index,
            transfer_queue_family_index,
        };
        let depth_format = if let Some(format) = find_supported_format(
            &physical_device,
            &[
                Format::D24_UNORM_S8_UINT,
                Format::D32_SFLOAT,
                Format::D32_SFLOAT_S8_UINT,
                Format::D16_UNORM,
            ],
            ImageTiling::OPTIMAL,
            FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
        ) {
            format
        } else {
            return Ok(None);
        };
        let depth_pyramid_format = if let Some(format) = find_supported_format(
            &physical_device,
            &[Format::R32_SFLOAT],
            ImageTiling::OPTIMAL,
            FormatFeatureFlags::SAMPLED_IMAGE
                | FormatFeatureFlags::STORAGE_IMAGE
                | FormatFeatureFlags::TRANSFER_DST,
        ) {
            format
        } else {
            return Ok(None);
        };
        if properties
            .vulkan_10
            .properties
            .limits
            .timestamp_compute_and_graphics
            == FALSE
        {
            return Ok(None);
        }
        let timestamp_period =
            properties.vulkan_10.properties.limits.timestamp_period as f64 / 1000000.0;
        let render_configuration = RenderConfiguration {
            depth_format,
            depth_pyramid_format,
            timestamp_period,
            use_draw_count,
        };
        Ok(Some(Self {
            physical_device,
            device_configuration,
            render_configuration,
        }))
    }

    pub fn create(self) -> Result<(Device, RenderConfiguration), vulkan::Error> {
        Ok((
            self.physical_device.create(self.device_configuration)?,
            self.render_configuration,
        ))
    }
}

impl PartialEq for DeviceCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for DeviceCandidate {}

impl PartialOrd for DeviceCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeviceCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        cmp_device_types(
            self.physical_device
                .properties()
                .vulkan_10
                .properties
                .device_type,
            other
                .physical_device
                .properties()
                .vulkan_10
                .properties
                .device_type,
        )
    }
}

fn present_mode_priority(present_mode: PresentModeKHR, disable_vsync: bool) -> usize {
    if present_mode == PresentModeKHR::IMMEDIATE {
        if disable_vsync {
            4
        } else {
            0
        }
    } else if present_mode == PresentModeKHR::MAILBOX {
        3
    } else if present_mode == PresentModeKHR::FIFO {
        2
    } else {
        1
    }
}

fn surface_format_priority(surface_format: SurfaceFormatKHR) -> usize {
    if surface_format.format == Format::B8G8R8A8_SRGB
        && surface_format.color_space == ColorSpaceKHR::SRGB_NONLINEAR
    {
        1
    } else {
        0
    }
}

pub fn choose_surface_format(
    surface: &Surface,
    physical_device_handle: PhysicalDeviceHandle,
) -> VkResult<Option<SurfaceFormatKHR>> {
    Ok(surface
        .formats(physical_device_handle)?
        .into_iter()
        .min_by_key(|surface_format| Reverse(surface_format_priority(*surface_format))))
}

pub fn choose_present_mode(
    surface: &Surface,
    physical_device_handle: PhysicalDeviceHandle,
    disable_vsync: bool,
) -> VkResult<Option<PresentModeKHR>> {
    Ok(surface
        .present_modes(physical_device_handle)?
        .into_iter()
        .min_by_key(|present_mode| Reverse(present_mode_priority(*present_mode, disable_vsync))))
}

fn find_supported_format(
    physical_device: &PhysicalDevice,
    formats: &[Format],
    tiling: ImageTiling,
    required_features: FormatFeatureFlags,
) -> Option<Format> {
    for format in formats {
        let properties = physical_device.format_properties(*format);
        let available_features = if tiling == ImageTiling::LINEAR {
            properties.linear_tiling_features
        } else if tiling == ImageTiling::OPTIMAL {
            properties.optimal_tiling_features
        } else {
            FormatFeatureFlags::empty()
        };
        if available_features.contains(required_features) {
            return Some(*format);
        }
    }
    None
}

pub fn create_swapchain(
    device: &Arc<Device>,
    surface: &Surface,
    window: &Window,
    disable_vsync: bool,
    previous: Option<&Swapchain>,
) -> eyre::Result<Swapchain> {
    let extent = window.inner_size();
    let extent = Extent2D {
        width: extent.width,
        height: extent.height,
    };
    let surface_format = choose_surface_format(surface, device.physical_device_handle())?
        .ok_or_else(|| eyre!("could not find suitable surface format"))?;
    let present_mode =
        choose_present_mode(surface, device.physical_device_handle(), disable_vsync)?
            .ok_or_else(|| eyre!("could not find suitable present mode"))?;
    let configuration = SwapchainConfiguration {
        surface,
        previous,
        present_mode,
        surface_format,
        extent,
    };
    Ok(device.create_swapchain(configuration)?)
}
