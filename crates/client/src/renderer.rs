use eyre::Error;
use vulkan::{Bool32, Format};

pub struct RenderResult {
    pub suboptimal: bool,
    pub timestamps: Option<RenderTimestamps>,
}

pub enum RenderError {
    Error(Error),
    OutOfDate,
}

impl From<vulkan::ApiResult> for RenderError {
    fn from(result: vulkan::ApiResult) -> Self {
        match result {
            vulkan::ApiResult::ERROR_OUT_OF_DATE_KHR => Self::OutOfDate,
            result => Self::Error(result.into()),
        }
    }
}

impl From<Error> for RenderError {
    fn from(error: Error) -> Self {
        Self::Error(error)
    }
}

pub struct RenderTimestamps {
    pub begin: f64,
    pub end: f64,
}

pub struct RenderConfiguration {
    pub depth_format: Format,
    pub depth_pyramid_format: Format,
    pub timestamp_period: f64,
    pub use_draw_count: Bool32,
}
