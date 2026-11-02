//! Declared scripts and package manager selection. Script bodies never leave this reader.
use super::metadata::read_manifest;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
}

impl PackageManager {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
        }
    }
}
pub const MAX_SCRIPTS: usize = 128;
pub fn supported_script_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('-')
        && !name.chars().any(char::is_control)
}
#[derive(Debug, Clone)]
pub struct NodeProject {
    pub manager: PackageManager,
    pub scripts: BTreeSet<String>,
}

pub fn inspect(root: &Path) -> Result<NodeProject> {
    let manifest = read_package(root)?;
    let manager = package_manager(root, &manifest)?;
    let scripts = declared_scripts(&manifest, manager)?;
    Ok(NodeProject { manager, scripts })
}

fn read_package(root: &Path) -> Result<serde_json::Map<String, serde_json::Value>> {
    serde_json::from_str(&read_manifest(&root.join("package.json"))?)
        .map_err(|_| anyhow::anyhow!("package.json must contain a valid JSON object"))
}

fn declared_scripts(
    object: &serde_json::Map<String, serde_json::Value>,
    manager: PackageManager,
) -> Result<BTreeSet<String>> {
    let scripts: BTreeSet<String> = match object.get("scripts") {
        None => BTreeSet::new(),
        Some(value) => {
            let scripts = value
                .as_object()
                .context("package.json scripts must contain an object")?;
            if scripts.len() > MAX_SCRIPTS {
                bail!("package.json exceeds the 128 scripts limit");
            }
            scripts
                .iter()
                .filter(|(name, body)| {
                    supported_script_name(name)
                        && body.as_str().is_some_and(|body| !body.trim().is_empty())
                })
                .map(|(name, _)| name.clone())
                .collect()
        }
    };
    if manager == PackageManager::Pnpm
        && scripts
            .iter()
            .any(|name| name.starts_with('/') && name.ends_with('/'))
    {
        bail!("pnpm regex script names are not supported; use a literal script name");
    }
    Ok(scripts)
}

fn package_manager(
    root: &Path,
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<PackageManager> {
    if let Some(value) = object.get("packageManager") {
        manager_name(value)
    } else {
        let mut managers = BTreeSet::new();
        // Workspaces may keep the lockfile at an ancestor of the nearest package.json.
        for directory in root.ancestors().take(32) {
            if directory != root && directory.join("package.json").is_file() {
                let parent = read_package(directory)?;
                if let Some(value) = parent.get("packageManager") {
                    managers.insert(manager_name(value)?);
                    break;
                }
            }
            for (marker, manager) in [
                ("pnpm-lock.yaml", PackageManager::Pnpm),
                ("pnpm-workspace.yaml", PackageManager::Pnpm),
                ("yarn.lock", PackageManager::Yarn),
                ("package-lock.json", PackageManager::Npm),
                ("npm-shrinkwrap.json", PackageManager::Npm),
            ] {
                if directory.join(marker).is_file() {
                    managers.insert(manager);
                }
            }
            if !managers.is_empty() {
                break;
            }
        }
        if managers.len() > 1 {
            bail!("conflicting package manager lockfiles; set packageManager explicitly");
        }
        Ok(managers.into_iter().next().unwrap_or(PackageManager::Npm))
    }
}

fn manager_name(value: &serde_json::Value) -> Result<PackageManager> {
    let declaration = value.as_str().context("packageManager must be a string")?;
    let (name, version) = declaration
        .split_once('@')
        .context("packageManager must include its version")?;
    if version.is_empty() {
        bail!("packageManager must include its version");
    }
    match name {
        "npm" => Ok(PackageManager::Npm),
        "pnpm" => Ok(PackageManager::Pnpm),
        "yarn" => Ok(PackageManager::Yarn),
        _ => bail!("unsupported packageManager; expected npm, pnpm or yarn"),
    }
}

pub fn capability(manager: &str, script: &str) -> String {
    format!("node.script.{manager}.{}", super::metadata::encoded(script))
}

pub fn action_id(manager: &str, script: &str) -> String {
    if manager == "npm" && matches!(script, "build" | "test" | "dev" | "start" | "lint") {
        format!("developer.node.{script}")
    } else {
        format!(
            "developer.node.{manager}.script.{}",
            super::metadata::encoded(script)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_declared_manager_then_nearest_lockfile_without_executing_scripts() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("package.json"), r#"{"scripts":{"build:prod":"echo secret", "x y":"echo text", "--bad":"echo bad", "blank":" "}}"#).unwrap();
        std::fs::write(root.join("pnpm-lock.yaml"), "").unwrap();
        let project = inspect(root).unwrap();
        assert_eq!(project.manager, PackageManager::Pnpm);
        assert_eq!(
            project.scripts,
            BTreeSet::from(["build:prod".into(), "x y".into()])
        );
        std::fs::write(root.join("yarn.lock"), "").unwrap();
        assert!(inspect(root).is_err());
        std::fs::write(
            root.join("package.json"),
            r#"{"packageManager":"yarn@4.9.2","scripts":{"test":"echo test"}}"#,
        )
        .unwrap();
        assert_eq!(inspect(root).unwrap().manager, PackageManager::Yarn);
        std::fs::write(
            root.join("package.json"),
            r#"{"packageManager":"bun@1.0.0"}"#,
        )
        .unwrap();
        assert!(inspect(root).is_err());
    }
}
