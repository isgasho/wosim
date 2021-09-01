use network::{server::EndpointError, FromPemError};
use server::ServiceError;
use vulkan::ApiResult;

use std::io;

#[derive(Debug)]
pub enum Error {
    Vulkan(vulkan::Error),
    Io(io::Error),
    Endpoint(EndpointError),
    Service(ServiceError),
    NoSuitableDeviceFound,
    FromPem(FromPemError),
    InvalidDecodeKey(jsonwebtoken::errors::Error),
}

impl From<vulkan::Error> for Error {
    fn from(error: vulkan::Error) -> Self {
        Self::Vulkan(error)
    }
}

impl From<ApiResult> for Error {
    fn from(result: ApiResult) -> Self {
        Self::Vulkan(result.into())
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
