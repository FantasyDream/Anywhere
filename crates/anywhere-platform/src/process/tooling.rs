//! Tool lookup and wrapper interpretation. Core only supplies a tool name and argv.
use anyhow::{Context, Result};
#[cfg(unix)]
use std::io::Read;
use std::path::{Path, PathBuf};

/// Resolve a tool without invoking a shell or sourcing login scripts.
pub fn find_executable(name: &str) -> Option<PathBuf> {
    if name.contains(['/', '\\']) {
        return None;
    }
    tool_directories()
        .into_iter()
        .flat_map(|path| {
            executable_names(name)
                .into_iter()
                .map(move |name| path.join(name))
        })
        .find(|path| is_executable(path))
}

fn executable_names(name: &str) -> Vec<String> {
    if cfg!(windows) && !name.contains('.') {
        // Batch files require a Windows command adapter; do not silently execute a shell.
        vec![format!("{name}.exe"), format!("{name}.com")]
    } else {
        vec![name.to_owned()]
    }
}

pub(super) fn tool_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    // GUI launches have a smaller PATH than interactive shells.
    #[cfg(unix)]
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
    #[cfg(unix)]
    directories
        .extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    #[cfg(windows)]
    if let Some(home) = std::env::var_os("USERPROFILE") {
        directories.push(PathBuf::from(home).join(".cargo/bin"));
    }
    directories
        .into_iter()
        .filter(|path| path.is_absolute())
        .collect()
}

/// Resolve an NVM default selector or a bounded alias chain without sourcing nvm.sh.
#[cfg(unix)]
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

#[cfg(unix)]
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

/// Optional GUI launch environment; existing user configuration always wins.
pub fn java_environment() -> std::collections::BTreeMap<String, String> {
    #[cfg(unix)]
    let mut environment = std::collections::BTreeMap::new();
    #[cfg(not(unix))]
    let environment = std::collections::BTreeMap::new();
    #[cfg(unix)]
    if std::env::var_os("JAVA_HOME").is_none() {
        if let Some(home) = std::env::var_os("HOME") {
            let java = PathBuf::from(home).join(".sdkman/candidates/java/current");
            if java.join("bin/java").is_file() {
                environment.insert("JAVA_HOME".into(), java.to_string_lossy().into_owned());
            }
        }
    }
    environment
}

/// Wrappers may live at an ancestor of a module. Never fall back when an existing wrapper fails.
pub fn wrapped_tool(
    directory: &Path,
    wrapper: &str,
    fallback: &str,
) -> Result<(PathBuf, Vec<String>)> {
    if let Some(path) = directory
        .ancestors()
        .take(32)
        .map(|root| root.join(wrapper))
        .find(|path| path.is_file())
    {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if path.metadata()?.permissions().mode() & 0o111 != 0 {
                return Ok((path, Vec::new()));
            }
            // Run the wrapper file with sh; do not feed any shell command string.
            return Ok((
                PathBuf::from("/bin/sh"),
                vec![path.to_string_lossy().into_owned()],
            ));
        }
        #[cfg(not(unix))]
        anyhow::bail!("project wrappers are currently supported on Unix only");
    }
    Ok((
        find_executable(fallback).with_context(|| format!("{fallback} was not found in PATH"))?,
        Vec::new(),
    ))
}

#[cfg(all(test, unix))]
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
