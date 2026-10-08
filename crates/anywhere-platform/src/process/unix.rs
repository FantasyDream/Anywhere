//! Unix process-tree ownership and nonblocking pipes. No product logic.
use anyhow::{Context, Result};
use std::{
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command},
};

pub(super) fn configure(command: &mut Command) -> Result<()> {
    command.process_group(0);
    Ok(())
}
pub(super) fn prepare_pipes(child: &Child) -> Result<()> {
    nonblocking(child.stdout.as_ref().context("stdout missing")?.as_raw_fd())?;
    nonblocking(child.stderr.as_ref().context("stderr missing")?.as_raw_fd())
}
pub(super) fn kill_tree(child: &mut Child) -> Result<()> {
    let result = unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error.into());
        }
    }
    Ok(())
}
fn nonblocking(fd: std::os::fd::RawFd) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
