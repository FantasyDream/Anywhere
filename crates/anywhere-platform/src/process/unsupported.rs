//! Reject before spawning: child-only cancellation cannot guarantee task-tree cleanup.
use anyhow::{bail, Result};
use std::{
    io::Read,
    process::{Child, Command},
};
pub(super) struct State;
pub(super) fn configure(_: &mut Command) -> Result<()> {
    bail!("This platform has no managed process-tree backend yet")
}
pub(super) fn attach(_: &Child) -> Result<State> {
    bail!("Managed process pipes are unsupported")
}
pub(super) fn has_exited(_: &Child) -> Result<bool> {
    bail!("Managed process observation is unsupported")
}
pub(super) fn kill_tree(_: &mut Child, _: &mut State) -> Result<()> {
    bail!("Managed process cancellation is unsupported")
}
pub(super) fn read_pipe<R: Read>(_: &mut R, _: &mut [u8]) -> Result<usize> {
    bail!("Managed process pipes are unsupported")
}
