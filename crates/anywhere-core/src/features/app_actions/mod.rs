//! Current-app menu actions reuse the normal registry, availability and handler contracts.
pub mod catalog;
pub mod convert;
pub mod handler;
pub use catalog::AppActionCatalog;
pub use convert::descriptors;
