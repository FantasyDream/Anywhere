//! Path and directory contexts, and the actions that go with them.
pub mod actions_file;
pub mod actions_folder;
pub mod preview;
pub mod provider;
pub mod scan;
pub mod search;
pub mod workspace;

mod result_store;

// An isolated comparison, never compiled into the application or its public API.
#[cfg(all(test, unix))]
mod result_store_probe;

mod file_types;
