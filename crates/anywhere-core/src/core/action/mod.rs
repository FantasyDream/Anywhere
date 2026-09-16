//! What can be done with a context.
//!
//! An action is declared by the feature that owns it, as part of an [`ActionBundle`]: a
//! group plus the actions in it. The registry only registers and looks up; matching,
//! availability and execution each have their own module.
//!
//! [`ActionBundle`]: bundle::ActionBundle
pub mod availability;
pub mod bundle;
pub mod descriptor;
pub mod executor;
pub mod group;
pub mod matcher;
