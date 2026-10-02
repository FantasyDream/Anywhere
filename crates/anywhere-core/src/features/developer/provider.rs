use crate::core::context::{
    provider::{ContextProvider, ProviderDescriptor},
    ContextValue, ExecutionContext, ResolvedContext,
};
use anyhow::Result;
use anywhere_protocol::{DIRECTORY, PROJECT_RUST};
use std::path::{Path, PathBuf};

pub struct RustProjectProvider {
    descriptor: ProviderDescriptor,
}

impl Default for RustProjectProvider {
    fn default() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "developer.rust_project".into(),
                accepts: vec![DIRECTORY.into()],
                produces: vec![PROJECT_RUST.into()],
                priority: 8,
            },
        }
    }
}

#[async_trait::async_trait]
impl ContextProvider for RustProjectProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let Some(path) = input.value.path().map(Path::to_path_buf) else {
            return Ok(Vec::new());
        };
        let root = tokio::task::spawn_blocking(move || project_root(&path)).await?;
        if !execution.is_current() {
            return Ok(Vec::new());
        }
        Ok(root
            .into_iter()
            .map(|root| {
                ResolvedContext::new(
                    PROJECT_RUST,
                    ContextValue::Directory(root),
                    input.source,
                    &self.descriptor.id,
                )
            })
            .collect())
    }
}

/// At most 32 ancestor metadata probes. No manifest parsing, shell or Cargo execution.
pub fn project_root(path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    if !canonical.is_dir() {
        return None;
    }
    canonical
        .ancestors()
        .take(32)
        .find(|root| root.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearest_manifest_is_found_without_running_or_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("Cargo.toml"), "not valid TOML").unwrap();
        let member = root.join("member");
        std::fs::create_dir_all(member.join("src/nested")).unwrap();
        std::fs::write(member.join("Cargo.toml"), "").unwrap();
        assert_eq!(project_root(&member.join("src/nested")), Some(member));
        assert_eq!(project_root(&root), Some(root));
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(project_root(empty.path()), None);
    }
}
