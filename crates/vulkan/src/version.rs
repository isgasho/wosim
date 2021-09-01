use ash::vk::{api_version_major, api_version_minor, api_version_patch, make_api_version};
use semver::Version;
pub trait VersionExt {
    fn to_u32(&self) -> u32;

    fn from_u32(version: u32) -> Self;
}

impl VersionExt for Version {
    fn to_u32(&self) -> u32 {
        make_api_version(0, self.major as u32, self.minor as u32, self.patch as u32)
    }

    fn from_u32(version: u32) -> Self {
        Self::new(
            api_version_major(version) as u64,
            api_version_minor(version) as u64,
            api_version_patch(version) as u64,
        )
    }
}
