//! Repository detection: filesystem metadata only, never a Git process.
use crate::core::context::provider::{ContextProvider, ProviderDescriptor};
use crate::core::context::{ContextValue, ExecutionContext, ResolvedContext};
use anyhow::{Context, Result};
use anywhere_protocol::*;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Metadata-only repository detection. Never runs Git.
pub struct GitRepositoryProvider {
    descriptor: ProviderDescriptor,
}
impl Default for GitRepositoryProvider {
    fn default() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.git_repository".into(),
                accepts: vec![DIRECTORY.into()],
                produces: vec![GIT_REPOSITORY.into()],
                priority: 10,
            },
        }
    }
}
#[async_trait]
impl ContextProvider for GitRepositoryProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let Some(path) = input.value.path().map(|path| path.to_path_buf()) else {
            return Ok(vec![]);
        };
        // The repository walk stats every ancestor, so it belongs on a blocking boundary
        // for the same reason input recognition does.
        let root = tokio::task::spawn_blocking(move || repository_root(&path))
            .await
            .context("repository detection task failed")?;
        // The walk may have outlived the revision that asked for it.
        if !execution.is_current() {
            return Ok(vec![]);
        }
        let Some(root) = root else {
            return Ok(vec![]);
        };
        Ok(vec![ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository(root),
            input.source,
            &self.descriptor.id,
        )])
    }
}

/// Walk up to the nearest repository root. A `.git` entry may be a directory or,
/// for linked worktrees and submodules, a file pointing at the real git dir.
pub fn repository_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let marker = current.join(".git");
        if marker.is_dir() || marker.is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::repository_root;

    #[test]
    fn repository_detection_handles_directories_files_and_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(repository_root(&nested), Some(root.clone()));
        assert_eq!(repository_root(&root), Some(root.clone()));
        // A worktree/submodule marker is a file, not a directory.
        let worktree = dir.path().join("worktree");
        std::fs::create_dir(&worktree).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: ../elsewhere\n").unwrap();
        assert_eq!(repository_root(&worktree), Some(worktree));
        // Outside any repository there is no root.
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(repository_root(&plain.path().canonicalize().unwrap()), None);
    }
}
