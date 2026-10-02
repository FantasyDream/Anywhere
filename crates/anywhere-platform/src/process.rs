//! Owned, non-interactive child processes. No shell interpolation or unbounded pipe reads.
use anyhow::{bail, Context, Result};
use std::{
    io::Read,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
};

#[derive(Debug, Clone)]
pub struct ProcessRequest {
    pub title: String,
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub directory: PathBuf,
}

/// The worker owns the child from spawn through reaping. Cancellation never uses a stale PID.
pub struct OwnedProcess {
    child: Child,
    finished: bool,
}

impl OwnedProcess {
    pub fn spawn(request: &ProcessRequest) -> Result<Self> {
        if !request.directory.is_dir() {
            bail!("working directory no longer exists");
        }
        if !request.program.is_absolute() || !request.program.is_file() {
            bail!("executable is unavailable");
        }
        let mut command = Command::new(&request.program);
        let tool_directory = request
            .program
            .parent()
            .context("executable directory missing")?;
        let search_path = std::iter::once(tool_directory.to_path_buf()).chain(tool_directories());
        command.env("PATH", std::env::join_paths(search_path)?);
        command
            .args(&request.arguments)
            .current_dir(&request.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(not(unix))]
        bail!("managed processes are currently supported on Unix only");
        let child = command.spawn().context("could not start task")?;
        let process = Self {
            child,
            finished: false,
        };
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            set_nonblocking(
                process
                    .child
                    .stdout
                    .as_ref()
                    .context("stdout missing")?
                    .as_raw_fd(),
            )?;
            set_nonblocking(
                process
                    .child
                    .stderr
                    .as_ref()
                    .context("stderr missing")?
                    .as_raw_fd(),
            )?;
        }
        Ok(process)
    }

    pub fn read_stdout(&mut self, buffer: &mut [u8]) -> Result<usize> {
        read_available(
            self.child.stdout.as_mut().context("stdout missing")?,
            buffer,
        )
    }

    pub fn read_stderr(&mut self, buffer: &mut [u8]) -> Result<usize> {
        read_available(
            self.child.stderr.as_mut().context("stderr missing")?,
            buffer,
        )
    }

    pub fn poll(&mut self) -> Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            // A non-interactive build must not orphan descendants keeping our pipes open.
            self.kill_group()?;
            self.finished = true;
        }
        Ok(status)
    }

    pub fn stop(&mut self) -> Result<ExitStatus> {
        self.kill_group()?;
        let status = self.child.wait()?;
        self.finished = true;
        Ok(status)
    }

    fn kill_group(&mut self) -> Result<()> {
        #[cfg(unix)]
        {
            // The child is the leader of a group we created. This includes build subprocesses.
            let result = unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
            if result != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error.into());
                }
            }
        }
        #[cfg(not(unix))]
        self.child.kill()?;
        Ok(())
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.kill_group();
            let _ = self.child.wait();
        }
    }
}

fn read_available(reader: &mut impl Read, buffer: &mut [u8]) -> Result<usize> {
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

#[cfg(unix)]
fn set_nonblocking(fd: std::os::fd::RawFd) -> Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// Resolve a tool without invoking a shell or sourcing login scripts.
pub fn find_executable(name: &str) -> Option<PathBuf> {
    if name.contains(['/', '\\']) {
        return None;
    }
    tool_directories()
        .into_iter()
        .map(|path| path.join(name))
        .find(|path| is_executable(path))
}

fn tool_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    // GUI launches have a smaller PATH than interactive shells.
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".cargo/bin"));
    }
    directories
        .extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    directories
        .into_iter()
        .filter(|path| path.is_absolute())
        .collect()
}

fn is_executable(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    path.is_file()
}
