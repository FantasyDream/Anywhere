//! Actions derived only from the current captured capabilities. Execution rechecks files.
use super::actions::tool;
use crate::core::{
    action::{
        availability::AvailabilityRule,
        group::groups,
        handler::{ActionHandler, ExecutionOutput},
    },
    context::ResolvedContext,
};
use anyhow::{bail, Context, Result};
use anywhere_platform::{
    process::{find_executable, ProcessRequest},
    Platform,
};
use std::collections::BTreeMap;

pub struct DeveloperActions;
impl crate::core::action::bundle::ContextActionFactory for DeveloperActions {
    fn supports_persistent_binding(&self, id: &str) -> bool {
        if matches!(
            id,
            "developer.node.build"
                | "developer.node.test"
                | "developer.node.dev"
                | "developer.node.start"
                | "developer.node.lint"
        ) {
            return true;
        }
        for manager in ["npm", "pnpm", "yarn"] {
            if let Some(encoded) = id.strip_prefix(&format!("developer.node.{manager}.script.")) {
                return decode_identity(encoded).is_some_and(|script| {
                    super::node::supported_script_name(&script)
                        && super::node::action_id(manager, &script) == id
                });
            }
        }
        id.strip_prefix("developer.cargo.run.")
            .and_then(decode_identity)
            .is_some_and(|target| {
                let parts: Vec<_> = target.split('\0').collect();
                parts.len() == 2
                    && parts.iter().all(|p| {
                        !p.is_empty() && p.len() <= 200 && !p.chars().any(char::is_control)
                    })
            })
    }
    fn id(&self) -> &'static str {
        "developer.actions"
    }
    fn actions(
        &self,
        contexts: &[ResolvedContext],
    ) -> crate::core::action::bundle::ActionExpansion {
        let mut definitions = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for context in contexts {
            let crate::core::context::ContextValue::Project { properties, .. } = &context.value
            else {
                continue;
            };
            if context.type_id == anywhere_protocol::PROJECT_NODE {
                let metadata = match super::metadata::NodeMetadata::read(properties) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        diagnostics.push(format!(
                            "{} / {}: {error}",
                            context.provider_id, context.type_id
                        ));
                        continue;
                    }
                };
                let manager = metadata.manager;
                let name = manager.as_str();
                let scripts = metadata.scripts;
                for script in scripts {
                    let id = super::node::action_id(name, &script);
                    let title = match script.as_str() {
                        "build" => format!("{name} Build"),
                        "test" => format!("{name} Test"),
                        "dev" => format!("{name} Dev"),
                        "start" => format!("{name} Start"),
                        "lint" => format!("{name} Lint"),
                        _ => format!("{name} Run · {script}"),
                    };
                    let handler = Script {
                        manager,
                        name: script.clone(),
                        title: title.clone(),
                    };
                    definitions.entry(id.clone()).or_insert_with(|| {
                        dynamic_definition(
                            (id, title),
                            anywhere_protocol::PROJECT_NODE,
                            (groups::NODE, super::actions::command_icon(&script)),
                            super::node::capability(name, &script),
                            handler,
                            vec![script.clone(), name.into(), "run".into()],
                            script == "build",
                        )
                    });
                }
            } else if context.type_id == anywhere_protocol::PROJECT_RUST {
                let metadata = match super::metadata::CargoMetadata::read(properties) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        diagnostics.push(format!(
                            "{} / {}: {error}",
                            context.provider_id, context.type_id
                        ));
                        continue;
                    }
                };
                if metadata.discovery_failed {
                    diagnostics.push(format!(
                        "{} / {}: Cargo targets could not be read; Check/Test remain available",
                        context.provider_id, context.type_id
                    ));
                }
                for entry in metadata.targets {
                    let key = entry.key();
                    let id = key.replacen("rust.run.", "developer.cargo.run.", 1);
                    let title = format!("Cargo Run · {} / {}", entry.package, entry.binary);
                    let aliases = vec![
                        entry.package.clone(),
                        entry.binary.clone(),
                        "cargo run".into(),
                    ];
                    let handler = CargoRun {
                        package: entry.package,
                        binary: entry.binary,
                        title: title.clone(),
                    };
                    definitions.entry(id.clone()).or_insert_with(|| {
                        dynamic_definition(
                            (id, title),
                            anywhere_protocol::PROJECT_RUST,
                            (groups::RUST, "developer-run"),
                            key,
                            handler,
                            aliases,
                            false,
                        )
                    });
                }
            }
        }
        crate::core::action::bundle::ActionExpansion {
            actions: definitions.into_values().collect(),
            diagnostics,
        }
    }
}

fn decode_identity(encoded: &str) -> Option<String> {
    if encoded.is_empty() || encoded.len() > 2048 {
        return None;
    }
    let value = String::from_utf8(hex::decode(encoded).ok()?).ok()?;
    (super::metadata::encoded(&value) == encoded).then_some(value)
}

fn dynamic_definition(
    (id, title): (String, String),
    accepts: &str,
    (group, icon): (&str, &str),
    capability: String,
    handler: impl ActionHandler + 'static,
    aliases: Vec<String>,
    preferred: bool,
) -> crate::core::action::descriptor::ActionDefinition {
    use crate::core::action::{
        descriptor::{ActionDefinition, ActionExecutor},
        handler::ActionHandlerRef,
    };
    ActionDefinition {
        localized_titles: Default::default(),
        id,
        title,
        subtitle: None,
        shortcut: None,
        accepts: vec![anywhere_protocol::ContextMatcher {
            type_id: accepts.into(),
        }],
        executor: ActionExecutor::Native(ActionHandlerRef::owned(handler)),
        availability: AvailabilityRule::RequireContextProperty { key: capability },
        base_priority: if preferred { 30 } else { 20 },
        icon: icon.into(),
        group_id: group.into(),
        aliases,
        keywords: vec!["project".into()],
        owning_app: None,
        menu_path: Vec::new(),
        primary_action: false,
        search_title: None,
    }
}

struct Script {
    manager: super::node::PackageManager,
    name: String,
    title: String,
}
impl ActionHandler for Script {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let directory = context
            .value
            .path()
            .context("project directory required")?
            .canonicalize()?;
        let project = super::node::inspect(&directory)?;
        if project.manager != self.manager || !project.scripts.contains(&self.name) {
            bail!("script or package manager changed; reopen the Launcher");
        }
        let manager = self.manager.as_str();
        let (program, mut arguments) = if let Some(program) = find_executable(manager) {
            (program, vec![])
        } else if self.manager != super::node::PackageManager::Npm {
            (tool("corepack")?, vec![manager.into()])
        } else {
            (tool("npm")?, vec![])
        };
        arguments.extend(["run".into(), self.name.clone()]);
        Ok(ExecutionOutput::process(
            crate::features::host_services::for_feature(
                super::descriptor(super::provider::ProjectKind::Node),
                platform,
            )?
            .process_request(ProcessRequest {
                title: self.title.clone(),
                program,
                arguments,
                directory,
                environment: BTreeMap::from([
                    ("NO_COLOR".into(), "1".into()),
                    ("COREPACK_ENABLE_AUTO_PIN".into(), "0".into()),
                    ("COREPACK_ENABLE_DOWNLOAD_PROMPT".into(), "0".into()),
                ]),
            })?,
        ))
    }
}
struct CargoRun {
    package: String,
    binary: String,
    title: String,
}
impl ActionHandler for CargoRun {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let directory = context
            .value
            .path()
            .context("project directory required")?
            .canonicalize()?;
        let target = super::rust::targets(&directory)?
            .into_iter()
            .find(|target| target.package == self.package && target.binary == self.binary)
            .context("Cargo target changed; reopen the Launcher")?;
        let mut arguments = vec![
            "run".into(),
            "--color".into(),
            "never".into(),
            "--manifest-path".into(),
            target.manifest.to_string_lossy().into_owned(),
            "--package".into(),
            self.package.clone(),
            "--bin".into(),
            self.binary.clone(),
        ];
        if !target.required_features.is_empty() {
            arguments.extend(["--features".into(), target.required_features.join(",")]);
        }
        Ok(ExecutionOutput::process(
            crate::features::host_services::for_feature(
                super::descriptor(super::provider::ProjectKind::Rust),
                platform,
            )?
            .process_request(ProcessRequest {
                title: self.title.clone(),
                program: tool("cargo")?,
                arguments,
                directory,
                environment: BTreeMap::from([("NO_COLOR".into(), "1".into())]),
            })?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::metadata::{CargoMetadata, NodeMetadata, CARGO_METADATA, NODE_METADATA};
    use super::super::node::PackageManager;
    use super::*;
    use crate::core::action::bundle::ContextActionFactory;
    use crate::core::context::{ContextSource, ContextValue};
    use std::sync::Arc;

    fn context(kind: &str, properties: BTreeMap<String, String>) -> ResolvedContext {
        ResolvedContext::new(
            kind,
            ContextValue::Project {
                directory: "/project".into(),
                label: "Test".into(),
                properties: Arc::new(properties),
            },
            ContextSource::UserInput,
            "test.provider",
        )
    }
    #[test]
    fn malformed_node_metadata_is_diagnosed_instead_of_becoming_an_empty_project() {
        let mut valid = BTreeMap::new();
        NodeMetadata {
            manager: PackageManager::Npm,
            scripts: vec!["build".into()],
        }
        .write(&mut valid)
        .unwrap();
        let good = valid[NODE_METADATA].clone();
        for bad in [
            "{private-secret".to_owned(),
            "{}".into(),
            good.replace("npm", "bun"),
        ] {
            let mut properties = valid.clone();
            properties.insert(NODE_METADATA.into(), bad);
            let expanded =
                DeveloperActions.actions(&[context(anywhere_protocol::PROJECT_NODE, properties)]);
            assert!(expanded.actions.is_empty());
            assert_eq!(expanded.diagnostics.len(), 1);
            assert!(expanded.diagnostics[0].contains("test.provider"));
            assert!(!expanded.diagnostics[0].contains("private-secret"));
        }
        valid.remove(NODE_METADATA);
        assert_eq!(
            DeveloperActions
                .actions(&[context(anywhere_protocol::PROJECT_NODE, valid)])
                .diagnostics
                .len(),
            1
        );
    }
    #[test]
    fn empty_capabilities_are_valid_and_target_read_failure_is_explicit() {
        let mut properties = BTreeMap::new();
        NodeMetadata {
            manager: PackageManager::Npm,
            scripts: vec![],
        }
        .write(&mut properties)
        .unwrap();
        let empty =
            DeveloperActions.actions(&[context(anywhere_protocol::PROJECT_NODE, properties)]);
        assert!(empty.actions.is_empty() && empty.diagnostics.is_empty());
        for failed in [false, true] {
            let mut properties = BTreeMap::new();
            CargoMetadata {
                targets: vec![],
                discovery_failed: failed,
            }
            .write(&mut properties)
            .unwrap();
            let empty =
                DeveloperActions.actions(&[context(anywhere_protocol::PROJECT_RUST, properties)]);
            assert!(empty.actions.is_empty());
            assert_eq!(empty.diagnostics.len(), usize::from(failed));
        }
    }
    #[test]
    fn cargo_metadata_decode_errors_are_diagnosed() {
        for raw in ["bad", "{}", r#"{"targets":false,"discovery_failed":false}"#] {
            let properties = BTreeMap::from([(CARGO_METADATA.into(), raw.into())]);
            let expanded =
                DeveloperActions.actions(&[context(anywhere_protocol::PROJECT_RUST, properties)]);
            assert!(expanded.actions.is_empty());
            assert_eq!(expanded.diagnostics.len(), 1);
        }
    }
}
