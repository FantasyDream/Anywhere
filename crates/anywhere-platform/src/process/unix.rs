//! Unix process-tree ownership and nonblocking pipes. No product logic.
use anyhow::{Context, Result};
use std::{
    io::Read,
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command},
};

pub(super) struct State;

pub(super) fn configure(command: &mut Command) -> Result<()> {
    command.process_group(0);
    Ok(())
}
pub(super) fn attach(child: &Child) -> Result<State> {
    nonblocking(child.stdout.as_ref().context("stdout missing")?.as_raw_fd())?;
    nonblocking(child.stderr.as_ref().context("stderr missing")?.as_raw_fd())?;
    Ok(State)
}
pub(super) fn has_exited(child: &Child) -> Result<bool> {
    loop {
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: waitid writes to a valid siginfo_t and WNOWAIT preserves child ownership.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            // SAFETY: the buffer was zero-initialized before waitid optionally filled it.
            return Ok(unsafe { info.assume_init() }.si_signo == libc::SIGCHLD);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}
pub(super) fn kill_tree(child: &mut Child, _state: &mut State) -> Result<()> {
    has_exited(child)?;
    // SAFETY: the child remains unreaped, reserving the process group we created at spawn.
    let result = unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        #[cfg(target_os = "macos")]
        if error.raw_os_error() == Some(libc::EPERM) && has_exited(child)? {
            return Ok(());
        }
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error.into());
        }
    }
    Ok(())
}
pub(super) fn read_pipe<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<usize> {
    match reader.read(buffer) {
        Ok(size) => Ok(size),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(0)
        }
        Err(error) => Err(error.into()),
    }
}
fn nonblocking(fd: std::os::fd::RawFd) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
