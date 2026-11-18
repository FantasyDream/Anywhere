//! Storage. The domain depends on the repository interface, not on SQLite.
pub(crate) mod application_history;
pub mod clipboard;
pub(crate) mod config_file;
pub mod features;
pub mod module_storage;
pub mod usage;

pub mod web_packages;
