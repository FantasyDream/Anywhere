//! Validation is separate from approval, installation, action registration and execution.
mod archive;
mod icon;
mod manifest;
mod path;
mod preflight;
pub mod wire;

pub use archive::{VerifiedAsset, VerifiedWebPackage};
pub use manifest::{parse_manifest, validate_manifest};

pub const MAX_MANIFEST_BYTES: usize = 128 * 1024;
pub const MAX_ARCHIVE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 2000;
pub const MAX_EXPANDED_BYTES: u64 = 80 * 1024 * 1024;
pub const MAX_ASSET_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_COMPRESSION_RATIO: u64 = 200;

#[cfg(test)]
mod tests;
