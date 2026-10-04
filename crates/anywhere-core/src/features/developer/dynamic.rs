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
    fn actions(
        &self,
        contexts: &[ResolvedContext],
    ) -> Vec<crate::core::action::descriptor::ActionDefinition> {
        let mut definitions = BTreeMap::new();
        for context in contexts {
            let crate::core::context::ContextValue::Project {
                directory,
                properties,
                ..
            } = &context.value
            else {
                continue;
            };
            if context.type_id == anywhere_protocol::PROJECT_NODE {
                let Some(manager) = properties
                    .get("node.manager")
                    .filter(|manager| matches!(manager.as_str(), "npm" | "pnpm" | "yarn"))
                else {
                    continue;
                };
                let scripts: Vec<String> = properties
                    .get("node.scripts")
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                for script in scripts {
                    let id = super::node::action_id(manager, &script);
                    let title = match script.as_str() {
                        "build" => format!("{manager} Build"),
                        "test" => format!("{manager} Test"),
                        "dev" => format!("{manager} Dev"),
                        "start" => format!("{manager} Start"),
                        "lint" => format!("{manager} Lint"),
                        _ => format!("{manager} Run · {script}"),
                    };
                    let handler = Script {
                        manager: manager.clone(),
                        name: script.clone(),
                        title: title.clone(),
                    };
                    definitions.entry(id.clone()).or_insert_with(|| {
                        dynamic_definition(
                            (id, title),
                            anywhere_protocol::PROJECT_NODE,
                            groups::NODE,
                            super::node::capability(manager, &script),
                            handler,
                            vec![script.clone(), manager.clone(), "run".into()],
                            script == "build",
                        )
                    });
                }
            } else if context.type_id == anywhere_protocol::PROJECT_RUST {
                for (property, value) in properties
                    .iter()
                    .filter(|(key, _)| key.starts_with("rust.run.") && key.ends_with(".target"))
                {
                    let Ok([package, binary, manifest]) =
                        serde_json::from_str::<[String; 3]>(value)
                    else {
                        continue;
                    };
                    let target = super::rust::RunTarget {
                        package,
                        binary,
                        manifest: directory.join(manifest),
                    };
                    let key = property.trim_end_matches(".target").to_owned();
                    let id = key.replacen("rust.run.", "developer.cargo.run.", 1);
                    let title = format!("Cargo Run · {} / {}", target.package, target.binary);
                    let aliases = vec![
                        target.package.clone(),
                        target.binary.clone(),
                        "cargo run".into(),
                    ];
                    let handler = CargoRun {
                        target,
                        title: title.clone(),
                    };
                    definitions.entry(id.clone()).or_insert_with(|| {
                        dynamic_definition(
                            (id, title),
                            anywhere_protocol::PROJECT_RUST,
                            groups::RUST,
                            key,
                            handler,
                            aliases,
                            false,
                        )
                    });
                }
            }
        }
        definitions.into_values().collect()
    }
}

fn dynamic_definition(
    (id, title): (String, String),
    accepts: &str,
    group: &str,
    capability: String,
    handler: impl ActionHandler + 'static,
    aliases: Vec<String>,
    home: bool,
) -> crate::core::action::descriptor::ActionDefinition {
    use crate::core::action::{
        descriptor::{ActionDefinition, ActionExecutor},
        handler::ActionHandlerRef,
    };
    ActionDefinition {
        id,
        title,
        subtitle: None,
        shortcut: None,
        accepts: vec![anywhere_protocol::ContextMatcher {
            type_id: accepts.into(),
        }],
        executor: ActionExecutor::Native(ActionHandlerRef::owned(handler)),
        availability: AvailabilityRule::RequireContextProperty { key: capability },
        base_priority: if home { 30 } else { 20 },
        icon: "code".into(),
        group_id: group.into(),
        aliases,
        keywords: vec!["project".into()],
        owning_app: None,
        menu_path: Vec::new(),
        home_eligible: home,
        primary_action: false,
        search_title: None,
    }
}

struct Script {
    manager: String,
    name: String,
    title: String,
}
impl ActionHandler for Script {
    fn execute(&self, context: &ResolvedContext, _: &dyn Platform) -> Result<ExecutionOutput> {
        let directory = context
            .value
            .path()
            .context("project directory required")?
            .canonicalize()?;
        let project = super::node::inspect(&directory)?;
        if project.manager != self.manager || !project.scripts.contains(&self.name) {
            bail!("script or package manager changed; reopen the Launcher");
        }
        let (program, mut arguments) = if let Some(program) = find_executable(&self.manager) {
            (program, vec![])
        } else if self.manager != "npm" {
            (tool("corepack")?, vec![self.manager.clone()])
        } else {
            (tool("npm")?, vec![])
        };
        arguments.extend(["run".into(), self.name.clone()]);
        Ok(ExecutionOutput::process(ProcessRequest {
            title: self.title.clone(),
            program,
            arguments,
            directory,
            environment: BTreeMap::from([
                ("NO_COLOR".into(), "1".into()),
                ("COREPACK_ENABLE_AUTO_PIN".into(), "0".into()),
                ("COREPACK_ENABLE_DOWNLOAD_PROMPT".into(), "0".into()),
            ]),
        }))
    }
}
struct CargoRun {
    target: super::rust::RunTarget,
    title: String,
}
impl ActionHandler for CargoRun {
    fn execute(&self, context: &ResolvedContext, _: &dyn Platform) -> Result<ExecutionOutput> {
        let directory = context
            .value
            .path()
            .context("project directory required")?
            .canonicalize()?;
        let target = super::rust::targets(&directory)?
            .into_iter()
            .find(|target| {
                target.package == self.target.package && target.binary == self.target.binary
            })
            .context("Cargo target changed; reopen the Launcher")?;
        Ok(ExecutionOutput::process(ProcessRequest {
            title: self.title.clone(),
            program: tool("cargo")?,
            arguments: vec![
                "run".into(),
                "--color".into(),
                "never".into(),
                "--manifest-path".into(),
                target.manifest.to_string_lossy().into_owned(),
                "--package".into(),
                self.target.package.clone(),
                "--bin".into(),
                self.target.binary.clone(),
            ],
            directory,
            environment: BTreeMap::from([("NO_COLOR".into(), "1".into())]),
        }))
    }
}
