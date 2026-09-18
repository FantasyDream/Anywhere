//! What can be done with a context.
//!
//! An action is declared by the feature that owns it, as part of an [`ActionBundle`]: a
//! group plus the actions in it. The registry only registers and looks up; matching,
//! availability and execution each have their own module. Side effects live on the
//! feature's [`handler::ActionHandler`]. Visibility is a closed
//! [`availability::AvailabilityRule`] on the spec, so custom actions can declare hide
//! rules without running code on the match path.
//!
//! [`ActionBundle`]: bundle::ActionBundle
pub mod availability;
pub mod bundle;
pub mod descriptor;
pub mod execute;
pub mod group;
pub mod handler;
pub mod matcher;
