use spirv_builder::{Capability, MetadataPrintout, SpirvBuilder, SpirvBuilderError};

#[cfg(target_os = "macos")]
pub const TARGET: &str = "spirv-unknown-spv1.3";

#[cfg(not(target_os = "macos"))]
pub const TARGET: &str = "spirv-unknown-vulkan1.2";

fn main() -> Result<(), SpirvBuilderError> {
    let builder = SpirvBuilder::new("../client-shader", TARGET)
        .print_metadata(MetadataPrintout::Full)
        .capability(Capability::Int8)
        .capability(Capability::ImageQuery)
        .capability(Capability::DrawParameters)
        .capability(Capability::Tessellation)
        .scalar_block_layout(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.capability(Capability::VulkanMemoryModelDeviceScope);
    builder.build()?;
    Ok(())
}
