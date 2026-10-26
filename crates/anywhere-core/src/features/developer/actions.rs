//! Fixed first-party commands. Project scripts are executed by npm, never interpolated here.
use super::provider::{script_property, ProjectKind};
use crate::core::{
    action::{
        availability::AvailabilityRule,
        bundle::{ActionBundle, ActionSpec},
        group::ActionGroup,
        handler::{ActionHandler, ExecutionOutput},
    },
    context::ResolvedContext,
};
use anyhow::{bail, Context, Result};
use anywhere_platform::{
    process::{find_executable, ProcessRequest},
    Platform,
};
use std::{collections::BTreeMap, path::PathBuf};

struct ProjectCommand {
    kind: ProjectKind,
    operation: &'static str,
    title: &'static str,
    localized_titles: &'static [(&'static str, &'static str)],
}
static CARGO_CHECK: ProjectCommand = ProjectCommand {
    kind: ProjectKind::Rust,
    operation: "check",
    title: "Cargo Check",
    localized_titles: &[("zh", "Cargo 检查")],
};
static CARGO_TEST: ProjectCommand = ProjectCommand {
    kind: ProjectKind::Rust,
    operation: "test",
    title: "Cargo Test",
    localized_titles: &[("zh", "Cargo 测试")],
};
static MAVEN_TEST: ProjectCommand = ProjectCommand {
    kind: ProjectKind::Maven,
    operation: "test",
    title: "Maven Test",
    localized_titles: &[("zh", "Maven 测试")],
};
static MAVEN_PACKAGE: ProjectCommand = ProjectCommand {
    kind: ProjectKind::Maven,
    operation: "package",
    title: "Maven Package",
    localized_titles: &[("zh", "Maven 打包")],
};
static GRADLE_BUILD: ProjectCommand = ProjectCommand {
    kind: ProjectKind::Gradle,
    operation: "build",
    title: "Gradle Build",
    localized_titles: &[("zh", "Gradle 构建")],
};
static GRADLE_TEST: ProjectCommand = ProjectCommand {
    kind: ProjectKind::Gradle,
    operation: "test",
    title: "Gradle Test",
    localized_titles: &[("zh", "Gradle 测试")],
};
static SPRING_MAVEN: ProjectCommand = ProjectCommand {
    kind: ProjectKind::SpringBoot,
    operation: "spring-boot:run",
    title: "Spring Boot Run · Maven",
    localized_titles: &[("zh", "Spring Boot 运行 · Maven")],
};
static SPRING_GRADLE: ProjectCommand = ProjectCommand {
    kind: ProjectKind::SpringBoot,
    operation: "bootRun",
    title: "Spring Boot Run · Gradle",
    localized_titles: &[("zh", "Spring Boot 运行 · Gradle")],
};

impl ActionHandler for ProjectCommand {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        if context.type_id != self.kind.type_id() {
            bail!("action requires the matching project context");
        }
        let directory = context
            .value
            .path()
            .context("project directory required")?
            .canonicalize()?;
        if !self
            .kind
            .markers()
            .iter()
            .any(|marker| directory.join(marker).is_file())
        {
            bail!("project manifest no longer exists; reopen the Launcher");
        }
        let (program, arguments) = match self.kind {
            ProjectKind::Rust => (
                tool("cargo")?,
                vec![self.operation.into(), "--color".into(), "never".into()],
            ),
            ProjectKind::Maven => {
                let (program, mut args) = wrapped_tool(&directory, "mvnw", "mvn")?;
                args.extend(
                    ["--batch-mode", "--no-transfer-progress", self.operation].map(String::from),
                );
                (program, args)
            }
            ProjectKind::Gradle => {
                let (program, mut args) = wrapped_tool(&directory, "gradlew", "gradle")?;
                args.extend(["--no-daemon", "--console=plain", self.operation].map(String::from));
                (program, args)
            }
            ProjectKind::SpringBoot => {
                let build_tool = if self.operation == "bootRun" {
                    "gradle"
                } else {
                    "maven"
                };
                if !super::spring::build_tools(&directory)?.contains(&build_tool) {
                    bail!("Spring Boot plugin is no longer declared; reopen the Launcher");
                }
                if build_tool == "gradle" {
                    let (program, mut args) = wrapped_tool(&directory, "gradlew", "gradle")?;
                    args.extend(
                        ["--no-daemon", "--console=plain", self.operation].map(String::from),
                    );
                    (program, args)
                } else {
                    let (program, mut args) = wrapped_tool(&directory, "mvnw", "mvn")?;
                    args.extend(
                        ["--batch-mode", "--no-transfer-progress", self.operation]
                            .map(String::from),
                    );
                    (program, args)
                }
            }
            ProjectKind::Node => bail!("Node commands require a declared script"),
        };
        let mut environment = BTreeMap::new();
        if matches!(
            self.kind,
            ProjectKind::Maven | ProjectKind::Gradle | ProjectKind::SpringBoot
        ) {
            environment.extend(anywhere_platform::process::java_environment());
        }
        environment.insert("NO_COLOR".into(), "1".into());
        Ok(ExecutionOutput::process(
            crate::features::host_services::for_feature(super::descriptor(self.kind), platform)?
                .process_request(ProcessRequest {
                    title: self.title.into(),
                    program,
                    arguments,
                    directory,
                    environment,
                })?,
        ))
    }
}

pub(super) fn tool(name: &str) -> Result<PathBuf> {
    find_executable(name).with_context(|| {
        format!("{name} was not found. Install it or add it to PATH, then try again.")
    })
}

use anywhere_platform::process::wrapped_tool;

fn spec(id: &'static str, command: &'static ProjectCommand, home: bool) -> ActionSpec {
    let availability = if matches!(command.kind, ProjectKind::SpringBoot) {
        AvailabilityRule::RequireContextProperty {
            key: format!(
                "spring.{}",
                if command.operation == "bootRun" {
                    "gradle"
                } else {
                    "maven"
                }
            ),
        }
    } else if matches!(command.kind, ProjectKind::Node) {
        AvailabilityRule::RequireContextProperty {
            key: script_property(command.operation),
        }
    } else {
        AvailabilityRule::Enabled
    };
    let (aliases, keywords): (&'static [&'static str], &'static [&'static str]) = (
        match command.operation {
            "check" => &["check", "compile"],
            "test" => &["test"],
            "build" => &["build"],
            "package" => &["package"],
            "dev" => &["dev"],
            "start" => &["start"],
            "lint" => &["lint"],
            _ => &[],
        },
        match command.kind {
            ProjectKind::Rust => &["cargo", "rust", "project"],
            ProjectKind::Maven => &["mvn", "maven", "project"],
            ProjectKind::Gradle => &["gradle", "project"],
            ProjectKind::Node => &["npm", "node", "script", "project"],
            ProjectKind::SpringBoot => &["spring", "boot", "run", "java"],
        },
    );
    ActionSpec {
        localized_titles: command.localized_titles,
        id,
        title: command.title,
        accepts: command.kind.type_id(),
        handler: command,
        availability,
        base_priority: if home { 30 } else { 20 },
        icon: command_icon(command.operation),
        aliases,
        keywords,
        primary_action: false,
    }
}

/// Presentation metadata follows the actual operation, not translated titles.
pub(super) fn command_icon(operation: &str) -> &'static str {
    match operation {
        "check" | "lint" => "developer-check",
        "test" => "developer-test",
        "build" => "developer-build",
        "package" => "developer-package",
        "dev" | "start" | "run" | "bootRun" | "spring-boot:run" => "developer-run",
        _ => "developer-script",
    }
}

fn bundle(group: ActionGroup, actions: Vec<ActionSpec>) -> ActionBundle {
    ActionBundle { group, actions }
}

pub fn rust_bundle() -> ActionBundle {
    bundle(
        crate::features::groups::rust(),
        vec![
            spec("developer.cargo.check", &CARGO_CHECK, true),
            spec("developer.cargo.test", &CARGO_TEST, false),
        ],
    )
}
pub fn maven_bundle() -> ActionBundle {
    bundle(
        crate::features::groups::maven(),
        vec![
            spec("developer.maven.test", &MAVEN_TEST, true),
            spec("developer.maven.package", &MAVEN_PACKAGE, false),
        ],
    )
}
pub fn gradle_bundle() -> ActionBundle {
    bundle(
        crate::features::groups::gradle(),
        vec![
            spec("developer.gradle.build", &GRADLE_BUILD, true),
            spec("developer.gradle.test", &GRADLE_TEST, false),
        ],
    )
}
pub fn node_bundle() -> ActionBundle {
    bundle(crate::features::groups::node(), Vec::new())
}
pub fn spring_bundle() -> ActionBundle {
    bundle(
        crate::features::groups::spring_boot(),
        vec![
            spec("developer.spring.maven.run", &SPRING_MAVEN, false),
            spec("developer.spring.gradle.run", &SPRING_GRADLE, false),
        ],
    )
}

pub use super::dynamic::DeveloperActions;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn a_non_executable_wrapper_with_spaces_is_a_literal_argument_and_has_priority() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir
            .path()
            .canonicalize()
            .unwrap()
            .join("project ' with spaces");
        std::fs::create_dir_all(root.join("module")).unwrap();
        std::fs::write(root.join("mvnw"), "#!/bin/sh\nprintf wrapper").unwrap();
        let (program, args) = wrapped_tool(&root.join("module"), "mvnw", "missing-tool").unwrap();
        assert_eq!(program, PathBuf::from("/bin/sh"));
        assert_eq!(args, [root.join("mvnw").to_string_lossy()]);
    }
}
