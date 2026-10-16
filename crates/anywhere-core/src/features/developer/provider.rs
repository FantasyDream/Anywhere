//! Lightweight, independent project recognizers. Never execute project tools.
use crate::core::context::{
    provider::{ContextProvider, ProviderDescriptor},
    ContextValue, ExecutionContext, ResolvedContext,
};
use anyhow::Result;
use anywhere_protocol::{
    DIRECTORY, JAVA_SPRING_BOOT, PROJECT_GRADLE, PROJECT_MAVEN, PROJECT_NODE, PROJECT_RUST,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

pub const MAX_PACKAGE_BYTES: usize = super::metadata::MAX_MANIFEST_BYTES;
pub const NODE_SCRIPTS: &[&str] = &["build", "test", "dev", "start", "lint"];

#[derive(Clone, Copy)]
pub enum ProjectKind {
    Rust,
    Maven,
    Gradle,
    Node,
    SpringBoot,
}

impl ProjectKind {
    pub fn type_id(self) -> &'static str {
        match self {
            Self::Rust => PROJECT_RUST,
            Self::Maven => PROJECT_MAVEN,
            Self::Gradle => PROJECT_GRADLE,
            Self::Node => PROJECT_NODE,
            Self::SpringBoot => JAVA_SPRING_BOOT,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust Project",
            Self::Maven => "Maven Project",
            Self::Gradle => "Gradle Project",
            Self::Node => "Node Project",
            Self::SpringBoot => "Spring Boot Project",
        }
    }
    pub fn markers(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["Cargo.toml"],
            Self::Maven => &["pom.xml"],
            Self::Gradle => &["build.gradle", "build.gradle.kts"],
            Self::Node => &["package.json"],
            Self::SpringBoot => &["pom.xml", "build.gradle", "build.gradle.kts"],
        }
    }
}

pub struct ProjectProvider {
    descriptor: ProviderDescriptor,
    kind: ProjectKind,
}

impl ProjectProvider {
    pub fn new(kind: ProjectKind) -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: format!("developer.{}", kind.type_id()),
                accepts: if matches!(kind, ProjectKind::SpringBoot) {
                    vec![PROJECT_MAVEN.into(), PROJECT_GRADLE.into()]
                } else {
                    vec![DIRECTORY.into()]
                },
                produces: vec![kind.type_id().into()],
                priority: 8,
            },
            kind,
        }
    }
}

pub fn providers() -> Vec<Arc<dyn ContextProvider>> {
    [
        ProjectKind::Rust,
        ProjectKind::Maven,
        ProjectKind::Gradle,
        ProjectKind::Node,
        ProjectKind::SpringBoot,
    ]
    .into_iter()
    .map(|kind| Arc::new(ProjectProvider::new(kind)) as Arc<dyn ContextProvider>)
    .collect()
}

#[async_trait::async_trait]
impl ContextProvider for ProjectProvider {
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
        let kind = self.kind;
        let ticket = execution.clone();
        let project = tokio::task::spawn_blocking(move || {
            if !ticket.is_current() {
                return Ok(None);
            }
            recognize(&path, kind)
        })
        .await??;
        if !execution.is_current() {
            return Ok(Vec::new());
        }
        Ok(project
            .into_iter()
            .map(|value| {
                ResolvedContext::new(kind.type_id(), value, input.source, &self.descriptor.id)
            })
            .collect())
    }
}

/// At most 32 ancestors; each project type finds its own nearest marker.
pub fn project_root_for(path: &Path, kind: ProjectKind) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    if !canonical.is_dir() {
        return None;
    }
    canonical
        .ancestors()
        .take(32)
        .find(|root| kind.markers().iter().any(|name| root.join(name).is_file()))
        .map(Path::to_path_buf)
}

fn recognize(path: &Path, kind: ProjectKind) -> Result<Option<ContextValue>> {
    let Some(directory) = project_root_for(path, kind) else {
        return Ok(None);
    };
    let mut properties = BTreeMap::new();
    if matches!(kind, ProjectKind::Node) {
        // Only safe presence flags cross the Context boundary, never script bodies.
        let project = super::node::inspect(&directory)?;
        super::metadata::NodeMetadata::new(
            &project.manager,
            project.scripts.into_iter().collect(),
        )?
        .write(&mut properties)?;
    }
    if matches!(kind, ProjectKind::Rust) {
        // A malformed manifest keeps Check/Test but reports why Run is unavailable.
        let (targets, failed) = match super::rust::targets(&directory) {
            Ok(targets) => (targets, false),
            Err(_) => (Vec::new(), true),
        };
        let metadata = targets
            .into_iter()
            .map(|target| {
                Ok(super::metadata::CargoTargetMetadata {
                    package: target.package,
                    binary: target.binary,
                    manifest: target
                        .manifest
                        .strip_prefix(&directory)?
                        .to_string_lossy()
                        .into_owned(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        super::metadata::CargoMetadata::new(metadata, failed).write(&mut properties)?;
    }
    if matches!(kind, ProjectKind::SpringBoot) {
        let tools = super::spring::build_tools(&directory)?;
        if tools.is_empty() {
            return Ok(None);
        }
        for tool in tools {
            properties.insert(format!("spring.{tool}"), "true".into());
        }
    }
    Ok(Some(ContextValue::Project {
        directory,
        label: kind.label().into(),
        properties: Arc::new(properties),
    }))
}

pub fn script_property(script: &str) -> String {
    format!("node.script.{script}")
}

pub fn declared_node_scripts(directory: &Path) -> Result<BTreeSet<String>> {
    Ok(super::node::inspect(directory)?.scripts)
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
        assert_eq!(
            project_root_for(&member.join("src/nested"), ProjectKind::Rust),
            Some(member)
        );
        assert_eq!(project_root_for(&root, ProjectKind::Rust), Some(root));
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(project_root_for(empty.path(), ProjectKind::Rust), None);
    }
    #[test]
    fn node_metadata_is_bounded_and_contains_only_declared_common_script_names() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("package.json");
        std::fs::write(&manifest, r#"{"scripts":{"build":"echo private-token", "test":" ", "dev":false, "start":"node server.js", "arbitrary":"echo ignored"}}"#).unwrap();
        let project = recognize(dir.path(), ProjectKind::Node).unwrap().unwrap();
        assert_eq!(project.property("node.script.build"), Some("true"));
        assert_eq!(project.property("node.script.test"), None);
        assert_eq!(project.property("node.script.dev"), None);
        assert_eq!(project.property("node.script.arbitrary"), Some("true"));
        let context = ResolvedContext::new(
            PROJECT_NODE,
            project,
            anywhere_protocol::ContextSource::Clipboard,
            "test",
        );
        assert!(!format!("{:?}", context.descriptor()).contains("private-token"));
        std::fs::write(&manifest, vec![b' '; MAX_PACKAGE_BYTES + 1]).unwrap();
        assert!(declared_node_scripts(dir.path())
            .unwrap_err()
            .to_string()
            .contains("256 KiB"));
        std::fs::write(&manifest, "invalid").unwrap();
        assert!(declared_node_scripts(dir.path()).is_err());
    }
}
