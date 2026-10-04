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
        let home = PathBuf::from(home);
        directories.push(home.join(".cargo/bin"));
        for tool in ["maven", "gradle", "java"] {
            directories.push(home.join(format!(".sdkman/candidates/{tool}/current/bin")));
        }
        directories.push(home.join(".volta/bin"));
        let nvm = std::env::var_os("NVM_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".nvm"));
        directories.push(nvm.join("current/bin"));
        if let Some(nvm) = nvm_default_directory(&nvm) {
            directories.push(nvm);
        }
    }
    directories
        .extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    directories
        .into_iter()
        .filter(|path| path.is_absolute())
        .collect()
}

/// Resolve an NVM default selector or a bounded alias chain without sourcing nvm.sh.
fn nvm_default_directory(root: &std::path::Path) -> Option<PathBuf> {
    let mut alias = "default".to_owned();
    for _ in 0..8 {
        let file = std::fs::File::open(root.join("alias").join(&alias)).ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(257).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 256 {
            return None;
        }
        alias = String::from_utf8(bytes).ok()?.trim().to_owned();
        let version = alias.strip_prefix('v').unwrap_or(&alias);
        let parts: Option<Vec<u32>> = version.split('.').map(|part| part.parse().ok()).collect();
        if let Some(parts) = parts.filter(|parts| !parts.is_empty() && parts.len() <= 3) {
            return installed_node_directory(root, &parts);
        }
        if matches!(alias.as_str(), "node" | "stable") {
            return installed_node_directory(root, &[]);
        }
        if alias.is_empty()
            || alias.contains("..")
            || alias.starts_with('/')
            || !alias
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '*'))
        {
            return None;
        }
    }
    None
}

fn installed_node_directory(root: &std::path::Path, prefix: &[u32]) -> Option<PathBuf> {
    std::fs::read_dir(root.join("versions/node"))
        .ok()?
        .take(128)
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let version = name.to_str()?.strip_prefix('v')?;
            let numbers: Option<Vec<u32>> =
                version.split('.').map(|part| part.parse().ok()).collect();
            let numbers = numbers?;
            let directory = entry.path().join("bin");
            (numbers.len() == 3 && numbers.starts_with(prefix) && directory.is_dir())
                .then_some((numbers, directory))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, path)| path)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvm_default_selection_is_bounded_and_respects_aliases_and_major_versions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("alias/lts")).unwrap();
        for version in ["v22.3.0", "v24.2.0", "v24.14.0"] {
            std::fs::create_dir_all(root.join("versions/node").join(version).join("bin")).unwrap();
        }
        std::fs::write(root.join("alias/default"), "24").unwrap();
        assert_eq!(
            nvm_default_directory(root),
            Some(root.join("versions/node/v24.14.0/bin"))
        );
        std::fs::write(root.join("alias/default"), "lts/*").unwrap();
        std::fs::write(root.join("alias/lts/*"), "v22.3.0").unwrap();
        assert_eq!(
            nvm_default_directory(root),
            Some(root.join("versions/node/v22.3.0/bin"))
        );
        std::fs::write(root.join("alias/default"), "../escape").unwrap();
        assert_eq!(nvm_default_directory(root), None);
        std::fs::write(root.join("alias/default"), "default").unwrap();
        assert_eq!(nvm_default_directory(root), None);
        std::fs::write(root.join("alias/default"), "a".repeat(257)).unwrap();
        assert_eq!(nvm_default_directory(root), None);
    }
}
