//! A module receives one bound namespace, never the host database or another module's keyspace.
use anyhow::Result;
pub const MAX_KEY_BYTES: usize = 128;
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
pub const MAX_NAMESPACE_BYTES: usize = 256 * 1024;
pub const MAX_NAMESPACE_KEYS: usize = 128;
/// Called once under the storage transaction. Must not perform I/O or re-enter storage.
pub type StorageUpdate<'a> = dyn FnMut(Option<&[u8]>) -> Result<Option<Vec<u8>>> + 'a;
pub trait ExtensionStorage: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    fn remove(&self, key: &str) -> Result<()>;
    /// Read the latest value and atomically replace/remove it, including across host instances.
    fn update(&self, key: &str, change: &mut StorageUpdate<'_>) -> Result<Option<Vec<u8>>>;
}
