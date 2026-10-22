//! Owned, non-interactive child processes. No shell interpolation or unbounded pipe reads.
mod tooling;
use tooling::tool_directories;
pub use tooling::{find_executable, java_environment, wrapped_tool};
#[cfg(unix)]
#[path = "process/unix.rs"]
mod backend;
#[cfg(not(unix))]
#[path = "process/unsupported.rs"]
mod backend;
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
    pub environment: std::collections::BTreeMap<String, String>,
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
        command.envs(&request.environment);
        command
            .args(&request.arguments)
            .current_dir(&request.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        backend::configure(&mut command)?;
        let child = command.spawn().context("could not start task")?;
        let process = Self {
            child,
            finished: false,
        };
        backend::prepare_pipes(&process.child)?;
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
        backend::kill_tree(&mut self.child)
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
