use crate::core::{
    action::{
        availability::AvailabilityRule,
        bundle::{ActionBundle, ActionSpec},
        handler::{ActionHandler, ExecutionOutput},
    },
    context::ResolvedContext,
};
use anyhow::{Context, Result};
use anywhere_platform::{
    process::{find_executable, ProcessRequest},
    Platform,
};
use anywhere_protocol::PROJECT_RUST;

struct Cargo {
    operation: &'static str,
    title: &'static str,
}
const CHECK: Cargo = Cargo {
    operation: "check",
    title: "Cargo Check",
};
const TEST: Cargo = Cargo {
    operation: "test",
    title: "Cargo Test",
};

impl ActionHandler for Cargo {
    fn execute(&self, context: &ResolvedContext, _: &dyn Platform) -> Result<ExecutionOutput> {
        let directory = context
            .value
            .path()
            .context("Rust project required")?
            .canonicalize()?;
        if !directory.join("Cargo.toml").is_file() {
            anyhow::bail!("Cargo.toml no longer exists");
        }
        let program = find_executable("cargo")
            .context("Cargo was not found. Install Rust or add Cargo to PATH, then try again.")?;
        Ok(ExecutionOutput::process(ProcessRequest {
            title: self.title.into(),
            program,
            arguments: vec![self.operation.into(), "--color".into(), "never".into()],
            directory,
        }))
    }
}

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: crate::core::action::group::groups::rust(),
        actions: vec![
            ActionSpec {
                id: "developer.cargo.check",
                title: "Cargo Check",
                accepts: PROJECT_RUST,
                handler: &CHECK,
                availability: AvailabilityRule::Enabled,
                base_priority: 30,
                icon: "code",
                aliases: &["check", "compile"],
                keywords: &["cargo", "rust", "project"],
                home_eligible: true,
                primary_action: false,
            },
            ActionSpec {
                id: "developer.cargo.test",
                title: "Cargo Test",
                accepts: PROJECT_RUST,
                handler: &TEST,
                availability: AvailabilityRule::Enabled,
                base_priority: 20,
                icon: "code",
                aliases: &["test"],
                keywords: &["cargo", "rust", "project"],
                home_eligible: false,
                primary_action: false,
            },
        ],
    }
}
