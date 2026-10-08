//! Reject before spawning: child-only cancellation cannot guarantee task-tree cleanup.
use anyhow::{bail, Result};
use std::process::{Child, Command};
pub(super) fn configure(_: &mut Command) -> Result<()> {
    bail!("This platform has no managed process-tree backend yet")
}
pub(super) fn prepare_pipes(_: &Child) -> Result<()> {
    bail!("Managed process pipes are unsupported")
}
pub(super) fn kill_tree(_: &mut Child) -> Result<()> {
    bail!("Managed process cancellation is unsupported")
}
