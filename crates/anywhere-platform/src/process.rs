//! Owned, non-interactive child processes. No shell interpolation or unbounded pipe reads.
mod tooling;
use tooling::tool_directories;
pub use tooling::{find_executable, java_environment, wrapped_tool};
#[cfg(unix)]
#[path = "process/unix.rs"]
mod backend;
#[cfg(windows)]
#[path = "process/windows.rs"]
mod backend;
#[cfg(not(any(unix, windows)))]
#[path = "process/unsupported.rs"]
mod backend;
use anyhow::{bail, Context, Result};
use std::{
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
    state: backend::State,
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
        let state = match backend::attach(&child) {
            Ok(state) => state,
            Err(error) => {
                let mut child = child;
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        Ok(Self {
            child,
            state,
            finished: false,
        })
    }

    pub fn read_stdout(&mut self, buffer: &mut [u8]) -> Result<usize> {
        backend::read_pipe(
            self.child.stdout.as_mut().context("stdout missing")?,
            buffer,
        )
    }

    pub fn read_stderr(&mut self, buffer: &mut [u8]) -> Result<usize> {
        backend::read_pipe(
            self.child.stderr.as_mut().context("stderr missing")?,
            buffer,
        )
    }

    pub fn poll(&mut self) -> Result<Option<ExitStatus>> {
        if self.finished {
            return Ok(self.child.try_wait()?);
        }
        if !backend::has_exited(&self.child)? {
            return Ok(None);
        }
        self.stop().map(Some)
    }

    pub fn stop(&mut self) -> Result<ExitStatus> {
        if !self.finished {
            self.kill_group()?;
        }
        let status = self.child.wait()?;
        self.finished = true;
        Ok(status)
    }

    fn kill_group(&mut self) -> Result<()> {
        backend::kill_tree(&mut self.child, &mut self.state)
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
