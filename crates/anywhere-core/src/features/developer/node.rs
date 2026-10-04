//! Declared scripts and package manager selection. Script bodies never leave this reader.
use super::metadata::read_manifest;
use anyhow::{bail, Context, Result};
use std::{collections::BTreeSet, path::Path};
pub const MAX_SCRIPTS: usize = 128;
#[derive(Debug, Clone)]
pub struct NodeProject {
    pub manager: String,
    pub scripts: BTreeSet<String>,
}

pub fn inspect(root: &Path) -> Result<NodeProject> {
    let manifest: serde_json::Value =
        serde_json::from_str(&read_manifest(&root.join("package.json"))?)
            .map_err(|_| anyhow::anyhow!("package.json is not valid JSON"))?;
    let object = manifest
        .as_object()
        .context("package.json must contain an object")?;
    let scripts = match object.get("scripts") {
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
                    !name.is_empty()
                        && name.len() <= 200
                        && !name.starts_with('-')
                        && !name.chars().any(char::is_control)
                        && body.as_str().is_some_and(|body| !body.trim().is_empty())
                })
                .map(|(name, _)| name.clone())
                .collect()
        }
    };
    let manager = if let Some(value) = object.get("packageManager") {
        manager_name(value)?
    } else {
        let mut managers = BTreeSet::new();
        // Workspaces may keep the lockfile at an ancestor of the nearest package.json.
        for directory in root.ancestors().take(32) {
            if directory != root && directory.join("package.json").is_file() {
                let parent: serde_json::Value =
                    serde_json::from_str(&read_manifest(&directory.join("package.json"))?)
                        .map_err(|_| anyhow::anyhow!("workspace package.json is not valid JSON"))?;
                if let Some(value) = parent.get("packageManager") {
                    managers.insert(manager_name(value)?);
                    break;
                }
            }
            for (marker, manager) in [
                ("pnpm-lock.yaml", "pnpm"),
                ("pnpm-workspace.yaml", "pnpm"),
                ("yarn.lock", "yarn"),
                ("package-lock.json", "npm"),
                ("npm-shrinkwrap.json", "npm"),
            ] {
                if directory.join(marker).is_file() {
                    managers.insert(manager.to_owned());
                }
            }
            if !managers.is_empty() {
                break;
            }
        }
        if managers.len() > 1 {
            bail!("conflicting package manager lockfiles; set packageManager explicitly");
        }
        managers.into_iter().next().unwrap_or_else(|| "npm".into())
    };
    if manager == "pnpm"
        && scripts
            .iter()
            .any(|name: &String| name.starts_with('/') && name.ends_with('/'))
    {
        bail!("pnpm regex script names are not supported; use a literal script name");
    }
    Ok(NodeProject { manager, scripts })
}

fn manager_name(value: &serde_json::Value) -> Result<String> {
    let declaration = value.as_str().context("packageManager must be a string")?;
    let (name, version) = declaration
        .split_once('@')
        .context("packageManager must include its version")?;
    if !matches!(name, "npm" | "pnpm" | "yarn") || version.is_empty() {
        bail!("unsupported packageManager; expected npm, pnpm or yarn");
    }
    Ok(name.to_owned())
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
        assert_eq!(project.manager, "pnpm");
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
        assert_eq!(inspect(root).unwrap().manager, "yarn");
        std::fs::write(
            root.join("package.json"),
            r#"{"packageManager":"bun@1.0.0"}"#,
        )
        .unwrap();
        assert!(inspect(root).is_err());
    }
}
