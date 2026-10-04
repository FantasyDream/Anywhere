//! Cargo targets from bounded local manifests and conventional source locations.
use super::metadata::{encoded, read_manifest};
use anyhow::{bail, Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTarget {
    pub package: String,
    pub binary: String,
    pub manifest: PathBuf,
}
impl RunTarget {
    pub fn key(&self) -> String {
        format!(
            "rust.run.{}",
            encoded(&format!("{}\0{}", self.package, self.binary))
        )
    }
}

pub fn targets(root: &Path) -> Result<Vec<RunTarget>> {
    let manifest = parse(root)?;
    let mut directories = BTreeSet::from([root.to_path_buf()]);
    if let Some(workspace) = manifest.get("workspace") {
        let excludes: Vec<String> = workspace
            .get("exclude")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
            .map(str::to_owned)
            .collect();
        for member in workspace
            .get("members")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
        {
            let pattern = format!(
                "{}/{}",
                glob::Pattern::escape(&root.to_string_lossy()),
                member
            );
            for (index, entry) in glob::glob(&pattern)?.take(129).enumerate() {
                if index == 128 {
                    bail!("Cargo workspace member expansion exceeds the 128 entries limit");
                }
                let directory = entry?.canonicalize()?;
                // Metadata traversal stays inside this workspace; external members are not inferred.
                if !directory.starts_with(root) {
                    continue;
                }
                let relative = directory.strip_prefix(root)?.to_string_lossy();
                if excludes
                    .iter()
                    .any(|exclude| glob::Pattern::new(exclude).is_ok_and(|p| p.matches(&relative)))
                {
                    continue;
                }
                if directory.join("Cargo.toml").is_file() {
                    directories.insert(directory);
                }
                if directories.len() > 64 {
                    bail!("Cargo workspace exceeds the 64 packages limit");
                }
            }
        }
    }
    let mut result = BTreeMap::new();
    for directory in directories {
        let manifest = if directory == root {
            manifest.clone()
        } else {
            parse(&directory)?
        };
        let Some(package) = manifest.get("package") else {
            continue;
        };
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .context("Cargo package name is required")?;
        let mut bins = BTreeSet::new();
        let mut declared_names = BTreeSet::new();
        let mut declared_paths = BTreeSet::new();
        for bin in manifest
            .get("bin")
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(name) = bin.get("name").and_then(toml::Value::as_str) {
                declared_names.insert(name.to_owned());
                let path = bin
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .map(|p| directory.join(p))
                    .unwrap_or_else(|| {
                        let file = directory.join(format!("src/bin/{name}.rs"));
                        if file.is_file() {
                            file
                        } else if directory.join(format!("src/bin/{name}/main.rs")).is_file() {
                            directory.join(format!("src/bin/{name}/main.rs"))
                        } else {
                            directory.join("src/main.rs")
                        }
                    });
                if let Ok(path) = path.canonicalize() {
                    if path.is_file() {
                        declared_paths.insert(path);
                        bins.insert(name.to_owned());
                    }
                }
            }
        }
        if package.get("autobins").and_then(toml::Value::as_bool) != Some(false) {
            if directory.join("src/main.rs").is_file()
                && !declared_names.contains(name)
                && !directory
                    .join("src/main.rs")
                    .canonicalize()
                    .is_ok_and(|p| declared_paths.contains(&p))
            {
                bins.insert(name.to_owned());
            }
            if let Ok(entries) = std::fs::read_dir(directory.join("src/bin")) {
                for (index, entry) in entries.take(129).enumerate() {
                    if index == 128 {
                        bail!("Cargo bin discovery exceeds the 128 entries limit");
                    }
                    let path = entry?.path();
                    let target =
                        if path.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                            path.file_stem()
                        } else if path.join("main.rs").is_file() {
                            path.file_name()
                        } else {
                            None
                        };
                    if let Some(target) = target.and_then(|name| name.to_str()) {
                        let source = if path.is_file() {
                            path.clone()
                        } else {
                            path.join("main.rs")
                        };
                        if !declared_names.contains(target)
                            && !source
                                .canonicalize()
                                .is_ok_and(|p| declared_paths.contains(&p))
                        {
                            bins.insert(target.to_owned());
                        }
                    }
                }
            }
        }
        for binary in bins {
            if name.is_empty() || binary.is_empty() || name.len() > 200 || binary.len() > 200 {
                bail!("invalid Cargo target name");
            }
            let target = RunTarget {
                package: name.into(),
                binary,
                manifest: directory.join("Cargo.toml"),
            };
            if result.insert(target.key(), target).is_some() {
                bail!("duplicate Cargo package/binary identity");
            }
            if result.len() > 128 {
                bail!("Cargo project exceeds the 128 targets limit");
            }
        }
    }
    Ok(result.into_values().collect())
}
fn parse(root: &Path) -> Result<toml::Value> {
    toml::from_str(&read_manifest(&root.join("Cargo.toml"))?)
        .context("Cargo.toml is not valid TOML")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_bins_claim_their_source_paths_and_missing_sources_are_hidden() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("src/bin")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("src/bin/tool.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname='sample'\nversion='0.1.0'\n[[bin]]\nname='renamed'\npath='src/main.rs'\n[[bin]]\nname='other'\npath='src/bin/tool.rs'\n[[bin]]\nname='missing'\npath='missing.rs'\n").unwrap();
        let found: BTreeSet<_> = targets(&root)
            .unwrap()
            .into_iter()
            .map(|t| t.binary)
            .collect();
        assert_eq!(found, BTreeSet::from(["renamed".into(), "other".into()]));
    }
    #[test]
    fn workspace_targets_include_explicit_and_inferred_bins_and_respect_excludes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=['crates/*']\nexclude=['crates/skip']\n",
        )
        .unwrap();
        for package in ["one", "two", "skip"] {
            let path = root.join("crates").join(package);
            std::fs::create_dir_all(path.join("src/bin/server")).unwrap();
            std::fs::write(path.join("Cargo.toml"), format!("[package]\nname='{package}'\nversion='0.1.0'\n[[bin]]\nname='explicit'\npath='launch.rs'\n")).unwrap();
            for file in [
                "src/main.rs",
                "src/bin/tool.rs",
                "src/bin/server/main.rs",
                "launch.rs",
            ] {
                std::fs::write(path.join(file), "fn main() {}\n").unwrap();
            }
        }
        let found = targets(&root).unwrap();
        assert_eq!(found.len(), 8);
        assert!(found.iter().all(|target| target.package != "skip"));
        assert_eq!(
            found
                .iter()
                .map(RunTarget::key)
                .collect::<BTreeSet<_>>()
                .len(),
            8
        );
    }
}
